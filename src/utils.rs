use std::{fs, path::Path};
use crate::track::Track;

use rand::Rng;
use ratatui::style::{Color, palette::tailwind::SLATE};

pub fn get_random_index(length: usize) -> usize {
    let mut range = rand::rng();
    range.random_range(0..length)
}

pub fn alternate_colors(i: usize) -> Color {
    if i % 2 == 0 {
        SLATE.c950
    } else {
        SLATE.c900
    }
}

pub fn visit_dirs(dir: &Path) -> Vec<Track> {
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