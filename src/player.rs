use std::fmt::Debug;
use std::io::BufReader;
use std::time::{Duration, Instant};
use std::{cmp, io};
use std::fs::File;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use crate::utils::{alternate_colors, get_random_index};
use ratatui::DefaultTerminal;
use ratatui::prelude::*;
use ratatui::style::palette::tailwind::{self, SLATE};
use ratatui::widgets::{Block, BorderType, Borders, Gauge, HighlightSpacing, List, ListItem, ListState, Padding, Paragraph};

use rodio::{Decoder, OutputStream, Sink, Source};

use crate::control::{Control, ControlButton};
use crate::track::Track;

/* Modes: (1) normal mode, (2) repeat mode, (3) shuffle mode */
/* Navigation: (1) playlist, (2) toolkit, (3) Search */

pub struct Player {
    playlist: Playlist,
    current: Track,
    current_index: usize,
    last_played: usize,
    sink: Sink,
    stream: OutputStream,
    start_time: Instant,
    position: Duration,
    ratio: u64,
    mode: u8,
    navigation: u8,
    state: AppState,
    control: Control,
    searching: String,
    is_paused: bool
}

#[derive(Debug, Default)]
pub struct Playlist {
    pub tracks: Vec<Track>,
    pub state: ListState,
}

#[derive(PartialEq)]
enum AppState {
    Running,
    Started,
    Quitting,
}

impl Player {
    pub fn new(tracks: &Vec<Track>) -> Self {
        
        let mut current = ListState::default();
        if !tracks.is_empty() {
            current.select(Some(0));
        }

        let stream = rodio::OutputStreamBuilder::open_default_stream()
            .expect("open default audio stream");

        let sink = rodio::Sink::connect_new(&stream.mixer());

        Player {
            playlist: Playlist { tracks: tracks.to_vec(), state: ListState::default() },
            current: tracks[0].clone(),
            current_index: 0,
            sink,
            stream,
            mode: 1,
            start_time: Instant::now(),
            position: Duration::from_secs(0),
            state: AppState::Started,
            ratio: 0,
            navigation: 1,
            control: Control { button: ControlButton::Play, selected: true },
            last_played: 0,
            searching: String::from(""),
            is_paused: false
        }
    }

    pub fn run(mut self, terminal: &mut DefaultTerminal, tracks: Vec<Track>) -> io::Result<()> {        
        self.playlist.tracks = tracks;
        
        while self.state != AppState::Quitting {
            terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;
            self.update();
            
            if let Event::Key(key) = event::read()? {
                self.handle_key(key);
            };
        }
        Ok(())
    }

    fn update(&mut self) {
        if self.state != AppState::Running {
            return;
        }

        if self.current.playing && self.state == AppState::Running {
            if self.position.as_secs() <= self.current.duration {
                self.position = Instant::now() - self.start_time;
            } else {
                self.position = Duration::new(self.current.duration, 0);
            }
        }

        self.ratio = self.calculate_ratio();

        
        if self.position.as_secs() >= self.current.duration - 5 {
            self.handle_end();
        }
    }

    pub fn render_explorer(&mut self, area: Rect, buf: &mut Buffer) {
        let general_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Fill(90), /* Explorer */
                Constraint::Length(3) /* Search */
            ]).split(area);
        
        let block = Block::new()
            .title(Line::raw("TRACKS").centered())
            .borders(Borders::ALL)
            .bg(SLATE.c950);

        let songs: Vec<ListItem> = self
            .playlist
            .tracks
            .iter()
            .enumerate()
            .map(|(i, track)| {
                let color = alternate_colors(i);
                ListItem::from(track.name.clone()).bg(color)
            }).collect();

        let list = List::new(songs)
            .block(block)
            .highlight_style(Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD))
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);

        let search = match self.searching.as_str() {
            "" => {
                if self.navigation == 3 {
                    "Type something."
                } else {
                    "Type '/' to search for a track."
                }
            },
            _ => &self.searching
        };

        Paragraph::new(search)
            .style(Style::new().gray())
            .block(
                Block::new()
                    .title("- [ Search ] ")
                    .borders(Borders::ALL)
                    .style(Style::new().light_cyan()).padding(Padding::left(2)))
            .render(general_layout[1], buf);

        StatefulWidget::render(list, area, buf, &mut self.playlist.state);
    }

    pub fn render_information(&mut self, area: Rect, buf: &mut Buffer) {
        let information =  Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Percentage(10), /* Song Title */
                Constraint::Percentage(90), /* Extra */
            ])
            .split(area);

        let extra = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Percentage(10), /* Position */
                Constraint::Percentage(40), /* Mode */
                Constraint::Percentage(40), /* Navigation */
                Constraint::Percentage(10), /* Duration */
            ])
            .split(information[1]);

        Block::new()
            .borders(Borders::TOP)
            .bg(SLATE.c950)
            .render(information[1], buf);

        Paragraph::new(self.position.as_secs().to_string())
            .style(Style::default().fg(Color::Yellow))
            .alignment(HorizontalAlignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Elapsed")
                    .border_type(BorderType::Rounded)
            ).render(extra[0], buf);

        Paragraph::new(self.get_mode())
            .style(Style::default().fg(Color::Yellow))
            .alignment(HorizontalAlignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Mode")
                    .border_type(BorderType::Rounded)
            ).render(extra[1], buf);

        Paragraph::new(self.get_navigation())
            .style(Style::default().fg(Color::Yellow))
            .alignment(HorizontalAlignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Navigation")
                    .border_type(BorderType::Rounded)
            ).render(extra[2], buf);

        Paragraph::new(self.current.duration.to_string())
            .style(Style::default().fg(Color::Yellow))
            .alignment(HorizontalAlignment::Center)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title("Duration")
                    .border_type(BorderType::Rounded)
            ).render(extra[3], buf);
    }

    pub fn render_gauge(&mut self, area: Rect, buf: &mut Buffer) {
        let title = Line::raw(self.current.name.clone()).centered()
            .bg(SLATE.c950);

        let block = Block::new()
            .title(title)
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .bg(SLATE.c950);

        Gauge::default()
            .block(block)
            .gauge_style(tailwind::CYAN.c800)
            .percent(self.ratio.try_into().unwrap())
            .render(area, buf);
    }

    pub fn render_toolkit(&mut self, area: Rect, buf: &mut Buffer) {
        let toolkit = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Percentage(20), /* Repeat */
                Constraint::Percentage(80), /* Control */
                Constraint::Percentage(20), /* shuffle */
            ])
            .split(area);

        let default_style = Style::default().fg(tailwind::CYAN.c200);
        let selected_style = Style::default().fg(tailwind::YELLOW.c400);

        Paragraph::new("↻")
            .centered()
            .style({
                if self.control.button == ControlButton::Repeat {
                    selected_style
                } else {
                    default_style
                }
            })
            .block(
                Block::default()
                    .borders(Borders::ALL)
            )
        .render(toolkit[0], buf);

        Paragraph::new("↳↰")
            .centered()
            .style({
                if self.control.button == ControlButton::Shuffle {  
                    selected_style
                } else {
                    default_style
                }
            })
            .block(
                Block::default()
                    .borders(Borders::ALL)
            )
        .render(toolkit[2], buf);

        let play = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Percentage(35), /* Backward */
                Constraint::Percentage(35), /* play */
                Constraint::Percentage(35), /* Forward */
            ]).split(toolkit[1]);

        let previous = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Percentage(50), /* Backwards */
                Constraint::Percentage(50), /* -10s */
            ]).split(play[0]);

        Paragraph::new("10s <<")
            .centered()
            .style({
                if self.control.button == ControlButton::MinusTen {
                    selected_style
                } else {
                    default_style
                }
            })
            .block(
                Block::default()
                    .borders(Borders::ALL)
            )
        .render(previous[0], buf);

    Paragraph::new("⏮")
            .centered()
            .style({
                if self.control.button == ControlButton::Previous {
                    selected_style
                } else {
                    default_style
                }
            })
            .block(
                Block::default()
                    .borders(Borders::ALL)
            )
        .render(previous[1], buf);
 
    Paragraph::new(if self.current.playing {"||"} else {"▶"})
            .centered()
            .style({
                if self.control.button == ControlButton::Play {
                    selected_style
                } else {
                    default_style
                }
            })
            .block(
                Block::default()
                    .borders(Borders::ALL)
            )
        .render(play[1], buf);

    let next = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Percentage(50), /* +10s */
                Constraint::Percentage(50), /* Next */
            ]).split(play[2]);

    Paragraph::new("⏭")
            .centered()
            .style({
                if self.control.button == ControlButton::Next {
                    selected_style
                } else {
                    default_style
                }
            })
            .block(
                Block::default()
                    .borders(Borders::ALL)
            )
        .render(next[0], buf);

    Paragraph::new(">> 10s")
            .centered()
            .style({
                if self.control.button == ControlButton::PlusTen {
                    selected_style
                } else {
                    default_style
                }
            })
            .block(
                Block::default()
                    .borders(Borders::ALL)
            )
        .render(next[1], buf);
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match self.navigation {
            1 => 
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => self.state = AppState::Quitting,
                    KeyCode::Char('h') | KeyCode::Left => self.select_none(),
                    KeyCode::Char('j') | KeyCode::Down => self.select_next(),
                    KeyCode::Char('k') | KeyCode::Up => self.select_previous(),
                    KeyCode::Char('g') | KeyCode::Home => self.select_first(),
                    KeyCode::Char('G') | KeyCode::End => self.select_last(),
                    KeyCode::Char('/') => self.navigation = 3,
                    KeyCode::Tab => self.navigation = 2,
                    KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
                        self.toggle_status();                 
                    }
                    _ => {}
                }
            2 => match key.code {
                KeyCode::Tab => self.navigation = 1,
                KeyCode::Char('/') => self.navigation = 3,
                KeyCode::Char('q') | KeyCode::Esc => self.state = AppState::Quitting,
                KeyCode::Char('h') | KeyCode::Left => self.select_left(),
                KeyCode::Char('j') | KeyCode::Right => self.select_right(),
                KeyCode::Char('l') | KeyCode::Enter => {
                        self.toggle_control_status();                 
                    }
                _ => {}
            }
            3 => match key.code  {
                KeyCode::Tab => self.navigation = 1,
                KeyCode::Backspace => {
                    if !self.searching.is_empty() {
                        self.searching = self.searching[0..self.searching.len() - 1].to_owned()
                    }},
                _ => self.searching.push_str(&key.code.as_char().unwrap_or_default().to_string()),
            }
            _ => {}
        }
        
    }

    fn select_none(&mut self) {
        self.playlist.state.select(None);
    }

    fn select_next(&mut self) {
        // self.playlist.state.select_next();
        let idx = self.current_index;

        self.last_played = idx;
        self.current_index = (idx + 1) % self.playlist.tracks.len();

        self.playlist.state.select(Some(self.current_index));
    }

    fn select_previous(&mut self) {
        // self.playlist.state.select_previous();
        
        let idx = self.current_index;

        if idx == 0 {
            self.select_last();
            self.last_played = self.current_index;
            self.current_index = self.playlist.tracks.len() - 1;
        } else {
            self.last_played = self.current_index;
            self.current_index = (idx - 1) % self.playlist.tracks.len();

            self.playlist.state.select(Some(self.current_index));
        }
    }

    fn select_first(&mut self) {
        self.playlist.state.select_first();
    }

    fn select_last(&mut self) {
        self.playlist.state.select_last();
    }

    fn toggle_status(&mut self) {
        if let Some(i) = self.playlist.state.selected() {
            self.playlist.tracks[i].playing = match self.playlist.tracks[i].playing {
                true => false,
                false => {
                    self.current = self.playlist.tracks[i].clone();
                    self.last_played = self.current_index;
                    self.current_index = i;
                    self.play_track();
                    true
                },
            };
        }
    }

    fn select_left(&mut self) {
        let buttons = vec![ControlButton::Repeat, ControlButton::MinusTen, ControlButton::Previous, ControlButton::Play, ControlButton::Next, ControlButton::PlusTen, ControlButton::Shuffle];

        let current_control_index = buttons.iter().position(|c| c == &self.control.button).unwrap_or_default();
        
        if current_control_index == 0 {
            self.control = Control { button: ControlButton::Shuffle, selected: true };
        } else {
            self.control.button = buttons[(current_control_index - 1) % buttons.len()];
            self.control.selected = true;
        }
    }

    fn select_right(&mut self) {
        let buttons = vec![ControlButton::Repeat, ControlButton::MinusTen, ControlButton::Previous, ControlButton::Play, ControlButton::Next, ControlButton::PlusTen, ControlButton::Shuffle];

        let current_control_index = buttons.iter().position(|c| c == &self.control.button).unwrap_or_default();
        self.control.button = buttons[(current_control_index + 1) % buttons.len()];
    }

    fn toggle_control_status(&mut self) {
        match self.control.button {
            ControlButton::Repeat => {
                match self.mode {
                    2 => self.mode = 1,
                    _ => self.mode = 2
                }
            },
            ControlButton::Previous => {
                if self.position.as_secs() > 5 {
                    self.position = Duration::new(0, 0);
                    self.play_track();
                } else {
                    self.select_previous();
                    self.current = self.playlist.tracks.get(self.current_index).unwrap().clone();
                    self.play_track();
                }
            },
            ControlButton::Play => {
                match self.current.playing {
                    true => self.pause_track(),
                    false => self.play_track(),
                }
            },
            ControlButton::Next => {
                match self.mode {
                    3 => {
                        self.play_random();
                    },
                    _ => {
                        self.select_next();
                        self.current = self.playlist.tracks.get(self.current_index).unwrap().clone();
                        self.play_track();
                    }
                }
            },
            ControlButton::Shuffle => {
                match self.mode {
                    3 => self.mode = 1,
                    _ => self.mode = 3
                }
            }
            ControlButton::MinusTen => self.skip_ten(false),
            ControlButton::PlusTen => self.skip_ten(true),
        }
    }

    fn handle_end(&mut self) {
        match self.mode {
            2 => self.play_track(),
            3 => self.play_random(),
            _ => {}
        }
    }

    fn play_random(&mut self) {
        let length = self.playlist.tracks.len();
        let to_play = get_random_index(length);

        self.playlist.state.select(Some(to_play));
        self.current = self.playlist.tracks.get(to_play).unwrap().clone();
        
        self.play_track();
    }

    pub fn get_mode(&self) -> String {
        match self.mode {
            1 => "Normal".to_owned(),
            2 => "Repeat".to_owned(),
            3 => "Shuffle".to_owned(),
            _ => "Not Selected".to_owned()
        }
    }

    fn get_navigation(&self) -> String {
        match self.navigation {
            1 => "Playlist".to_owned(),
            2 => "Toolkit".to_owned(),
            3 => "Search".to_owned(),
            _ => "Not Selected".to_owned()
        }
    }

    fn play_track(&mut self) {
        if self.is_paused {
            self.pause_track();

            let source = Decoder::new(BufReader::new(File::open(self.current.path.clone()).unwrap())).unwrap();
            
            let current_position = self.position;

            self.sink.append(source.skip_duration(current_position));
        } else {
            self.stop_track();

            let file = BufReader::new(File::open(self.current.path.clone()).unwrap());
            self.sink = rodio::play(&self.stream.mixer(), file).unwrap();
        }

        self.is_paused = false;
        self.state = AppState::Running;
        self.current.playing = true;
    }

    fn pause_track(&mut self) {
        self.current.playing = false;
        self.state = AppState::Started;
        self.sink = rodio::Sink::connect_new(&self.stream.mixer());
        self.is_paused = true;
    }

    fn stop_track(&mut self) {
        self.current.playing = false;
        self.state = AppState::Started;
        self.position = Duration::new(0, 0);
        self.start_time = Instant::now();
        self.sink = rodio::Sink::connect_new(&self.stream.mixer());
    }

    fn skip_ten(&mut self, direction: bool) {
        self.pause_track();

        let source = Decoder::new(BufReader::new(File::open(self.current.path.clone()).unwrap())).unwrap();

        // Adding +10s
        let mut skip_duration = self.position;

        match direction {
            /* +10s */
            true => {
                skip_duration = cmp::min(skip_duration + Duration::from_secs(10), Duration::new(self.current.duration - 1, 0));

                self.start_time -= Duration::new(10, 0);
            },
            /* -10s */
            false => {
                if skip_duration < Duration::from_secs(10) {
                    self.stop_track();
                    self.play_track();
                } else {
                    skip_duration -= Duration::from_secs(10);
                    self.start_time += Duration::new(10, 0);
                }

            },
        }

        self.current.playing = true;
        self.state = AppState::Running;

        self.sink.append(source.skip_duration(skip_duration));
    }

    fn calculate_ratio(&self) -> u64 {
        cmp::min((self.position.as_secs() * 100) / self.current.duration, 100)
    }
}

