use std::{fs::File, path::Path};

use symphonia::{core::{io::MediaSourceStream, probe::Hint}, default::get_probe};

#[derive(Debug, Default, Clone)]
pub struct Track {
    pub name: String,
    pub path: String,
    pub playing: bool,
    pub duration: u64,
}

impl Track {
    pub fn new(name: String, path: String) -> Self {
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