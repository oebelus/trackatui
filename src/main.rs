use std::fmt::Debug;
use std::i64::MAX;
use std::io::BufReader;
use std::time::{Duration, Instant};
use std::{cmp, env, io};
use std::fs::{self, File};
use std::path::Path;

use color_eyre::owo_colors::OwoColorize;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind};
use ratatui::DefaultTerminal;
use ratatui::prelude::*;
use ratatui::style::palette::tailwind::{self, SLATE};
use ratatui::widgets::{Block, BorderType, Borders, Gauge, HighlightSpacing, List, ListItem, ListState, Padding, Paragraph};

use rodio::{OutputStream, Sink};
use symphonia::core::io::MediaSourceStream;
use symphonia::core::probe::Hint;
use symphonia::default::get_probe;
use tui_big_text::{BigText, PixelSize};

pub struct App {
    playlist: Playlist,
    current: Track,
    sink: Sink,
    stream: OutputStream,
    start_time: Instant,
    position: Duration,
    ratio: u64,
    mode: u8,
    navigation: u8,
    state: AppState,
}

/* Modes: (1) normal mode, (2) repeat mode, (3) shuffle mode */
/* Navigation: (1) playlist, (2) toolkit */

#[derive(Debug, Default)]
pub struct Playlist {
    tracks: Vec<Track>,
    state: ListState,
}

#[derive(Debug, Default, Clone)]
pub struct Track {
    name: String,
    path: String,
    playing: bool,
    duration: u64,
}

#[derive(PartialEq)]
enum AppState {
    Running,
    Started,
    Quitting,
}

const SELECTED_STYLE: Style = Style::new().bg(SLATE.c800).add_modifier(Modifier::BOLD);

fn main() -> color_eyre::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        println!("Usage: <script> <directory>");
        return Ok(());
    }

    let tracks = visit_dirs(Path::new(&args[1]));

    let app = App::new(&tracks);

    // println!("{:?}", y);

    ratatui::run(|terminal| app.run(terminal, tracks.to_vec()))?;
    Ok(())
}

impl Track {
    fn new(name: String, path: String) -> Self {
        Self {
            name,
            path: path.clone(),
            playing: false,
            duration: Self::calculate_duration(path).unwrap(),
        }
    }

    fn calculate_duration(path: String) -> Option<u64> {
        let file = File::open(Path::new(&path)).ok()?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());
        let mut hint = Hint::new();
        hint.with_extension("mp3");

        let format = get_probe()
            .format(&hint, mss, &Default::default(), &Default::default())
            .ok()?
            .format;

        if let Some(track) = format.tracks().iter().next() {
            if let Some(time_base) = track.codec_params.time_base {
                if let Some(n_frames) = track.codec_params.n_frames {
                    let duration_secs =
                        n_frames as f64 / time_base.denom as f64 * time_base.numer as f64;
                    return Some(duration_secs as u64);
                }
            }
        }
        None
    }
}

impl App {
    pub fn new(tracks: &Vec<Track>) -> Self {
        
        let mut current = ListState::default();
        if !tracks.is_empty() {
            current.select(Some(0));
        }

        let stream = rodio::OutputStreamBuilder::open_default_stream()
            .expect("open default audio stream");

        let sink = rodio::Sink::connect_new(&stream.mixer());

        App {
            playlist: Playlist { tracks: tracks.to_vec(), state: ListState::default() },
            current: tracks[0].clone(),
            sink,
            stream,
            mode: 1,
            start_time: Instant::now(),
            position: Duration::from_secs(0),
            state: AppState::Started,
            ratio: 0,
            navigation: 1,
        }
    }

    pub fn run(mut self, terminal: &mut DefaultTerminal, tracks: Vec<Track>) -> io::Result<()> {        
        self.playlist.tracks = tracks;
        
        while self.state != AppState::Quitting {
            terminal.draw(|frame| frame.render_widget(&mut self, frame.area()))?;
            if let Event::Key(key) = event::read()? {
                self.handle_key(key);
                self.update();
            };
        }
        Ok(())
    }

    fn update(&mut self) {
        if self.state != AppState::Running {
            return;
        }

        self.position = Duration::from_secs(self.elapsed_duration());
        self.ratio = self.calculate_ratio();
    }

    fn render_explorer(&mut self, area: Rect, buf: &mut Buffer) {
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
            .highlight_style(SELECTED_STYLE)
            .highlight_symbol(">")
            .highlight_spacing(HighlightSpacing::Always);

        StatefulWidget::render(list, area, buf, &mut self.playlist.state)
    }

    fn render_information(&mut self, area: Rect, buf: &mut Buffer) {
        let title = Line::raw(self.current.name.clone()).centered()
            .bg(SLATE.c950);

        let total_duration = Line::raw(self.current.duration.to_string());

        let elapsed = Line::from(self.elapsed_duration().to_string()).left_aligned();

        let block = Block::new()
            .title(title)
            .title_bottom(total_duration)
            .title_top(elapsed)
            .borders(Borders::ALL)
            .bg(SLATE.c950);

        Gauge::default()
            .block(block)
            .gauge_style(tailwind::CYAN.c800)
            .percent(self.ratio.try_into().unwrap())
            .render(area, buf);
    }

    fn render_toolkit(&mut self, area: Rect, buf: &mut Buffer) {
        let toolkit = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Percentage(20), /* Repeat */
                Constraint::Percentage(60), /* play */
                Constraint::Percentage(20), /* shuffle */
            ])
            .split(area);

        Paragraph::new("↻")
            .centered()
            .style(Style::default().fg(tailwind::CYAN.c400).bg(tailwind::SLATE.c950))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
            )
        .render(toolkit[0], buf);

        Paragraph::new("↳↰")
            .centered()
            .style(Style::default().fg(tailwind::CYAN.c400).bg(tailwind::SLATE.c950))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
            )
        .render(toolkit[2], buf);

        let play = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Percentage(33), /* Backward */
                Constraint::Percentage(33), /* play */
                Constraint::Percentage(33), /* Forward */
            ]).split(toolkit[1]);

        Paragraph::new("⏮")
            .centered()
            .style(Style::default().fg(tailwind::CYAN.c400).bg(tailwind::SLATE.c950))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
            )
        .render(play[0], buf);

    Paragraph::new("▶")
            .centered()
            .style(Style::default().fg(tailwind::CYAN.c400).bg(tailwind::SLATE.c950))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
            )
        .render(play[1], buf);

    Paragraph::new("⏭")
            .centered()
            .style(Style::default().fg(tailwind::CYAN.c400).bg(tailwind::SLATE.c950))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
            )
        .render(play[2], buf);
    
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.state = AppState::Quitting,
            KeyCode::Char('h') | KeyCode::Left => self.select_none(),
            KeyCode::Char('j') | KeyCode::Down => self.select_next(),
            KeyCode::Char('k') | KeyCode::Up => self.select_previous(),
            KeyCode::Char('g') | KeyCode::Home => self.select_first(),
            KeyCode::Char('G') | KeyCode::End => self.select_last(),
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => {
                self.toggle_status();                 
            }
            _ => {}
        }
    }

    fn select_none(&mut self) {
        self.playlist.state.select(None);
    }

    fn select_next(&mut self) {
        self.playlist.state.select_next();
    }

    fn select_previous(&mut self) {
        self.playlist.state.select_previous();
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
                    self.play_track();
                    true
                },
            };
        }
    }

    fn play_track(&mut self) {
        self.stop_track();

        self.state = AppState::Running;

        let file = BufReader::new(File::open(self.current.path.clone()).unwrap());
        
        self.sink = rodio::play(&self.stream.mixer(), file).unwrap();
    }

    fn stop_track(&mut self) {
        self.state = AppState::Started;
        self.start_time = Instant::now();
    }

    fn elapsed_duration(&mut self) -> u64 {
        let elapsed = (Instant::now() - self.start_time).as_secs();

        if elapsed > self.current.duration {
            return self.current.duration;
        }
        (Instant::now() - self.start_time).as_secs()
    }

    fn calculate_ratio(&self) -> u64 {
        cmp::max((self.position.as_secs() * 100) / self.current.duration, 100)
    }
}

impl Widget for &mut App {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Percentage(85), /* Player */
                Constraint::Percentage(15), /* Commands */
            ])
            .split(area);

        let general_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Percentage(30), /* File Explorer */
                Constraint::Percentage(70), /* Music Player */
            ])
            .split(layout[0]);

        let music_player = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Percentage(70), /* Information */
                Constraint::Percentage(30), /* Toolkit */
            ])
            .split(general_layout[1]);
        
        /* File Explorer */
        App::render_explorer(self, general_layout[0], buffer);

        /* Information */
        App::render_information(self, music_player[0], buffer);

        /* Toolkit */
        App::render_toolkit(self, music_player[1], buffer);

        // frame.render_widget(
        //     Paragraph::new("Player")
        //         .block(Block::new().borders(Borders::ALL)),
        //     music_player[1]);

        // frame.render_widget(
        //     Paragraph::new("Commands")
        //         .block(Block::new().borders(Borders::ALL)),
        //     layout[1]);
    }
}

fn visit_dirs(dir: &Path) -> Vec<Track> {
    let mut tracks = vec![];
    
    if dir.is_dir() {
            for entry in fs::read_dir(dir).unwrap() {
                let entry = entry.unwrap();
                let path = entry.path();

                if !path.is_dir() {
                    let p = path.to_str().unwrap_or_default();
                    if p.ends_with(".mp3") {
                        tracks.push(Track::new(p.split("\\").last().unwrap_or_default().to_string(), path.to_str().unwrap_or_default().to_owned()));
                    }
                }
            }
    }

    tracks
}


const fn alternate_colors(i: usize) -> Color {
    if i % 2 == 0 {
        SLATE.c950
    } else {
        SLATE.c900
    }
}