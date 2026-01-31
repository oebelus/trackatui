mod track;
mod widget;
mod control;
mod player;
mod utils;

use std::{env, path::Path};

use crate::{utils::visit_dirs, player::Player};

fn main() -> color_eyre::Result<()> {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        println!("Usage: <script> <directory>");
        return Ok(());
    }

    let tracks = visit_dirs(Path::new(&args[1]));

    let app = Player::new(&tracks);

    ratatui::run(|terminal| app.run(terminal, tracks.to_vec()))?;
    ratatui::restore();
    Ok(())
}