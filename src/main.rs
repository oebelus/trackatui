mod track;
mod widget;
mod control;
mod player;
mod utils;

use std::{env, path::Path};

use color_eyre::Result;

use crate::{utils::visit_dirs, player::Player};

fn main() -> Result<()> {
    let args: Vec<String> = env::args().collect();

    let script = "cargo run";

    if args.len() != 2 {
        println!("Usage: {script} <directory>");
        return Ok(());
    }

    let tracks = visit_dirs(Path::new(&args[1]));

    if tracks.is_empty() {
        println!("The folder you provided does not contain any mp3 file.");
        return Ok(());
    }

    let app = Player::new(&tracks);

    ratatui::run(|terminal| app.run(terminal, tracks.to_vec()))?;
    ratatui::restore();
    Ok(())
}