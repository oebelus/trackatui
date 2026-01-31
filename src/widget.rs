use ratatui::{buffer::Buffer, layout::{Constraint, Direction, Layout, Rect}, style::{Style, palette::tailwind::SLATE}, widgets::{Block, Borders, Widget}};

use crate::Player;

impl Widget for &mut Player {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        let general_layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![
                Constraint::Percentage(30), /* File Explorer */
                Constraint::Fill(70), /* Music Player */
            ])
            .split(area);

        let music_player = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Fill(80), /* Information */
                Constraint::Length(3), /* Toolkit */
            ])
            .split(general_layout[1]);

        let information = Layout::default()
            .direction(Direction::Vertical)
            .constraints(vec![
                Constraint::Percentage(60), /* Progression gauge */
                Constraint::Percentage(20), /* Space */
                Constraint::Length(3), /* Informatiom */
            ])
            .split(music_player[0]);

        /* File Explorer */
        Player::render_explorer(self, general_layout[0], buffer);

        Block::new()
            .borders(Borders::ALL)
            .style(Style::new().bg(SLATE.c950))
            .render(music_player[0], buffer);

        /* Information */
        Player::render_information(self, information[2], buffer);

        /* Space */
        Block::new()
            .style(Style::new().bg(SLATE.c950))
            .render(information[1], buffer);

        /* Progression Gauge */
        Player::render_gauge(self, information[0], buffer);

        /* Toolkit */
        Player::render_toolkit(self, music_player[1], buffer);
    }
}