//! Describes every track of a list without downloading anything:
//! `cargo run --example check -- path/to/list.txt`. Needs yt-dlp in `PATH`.

use std::process::ExitCode;

use engine::playlist;
use engine::process::YtDlp;
use engine::tracklist;

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: check <track list file>");
        return ExitCode::FAILURE;
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!("{path}: {error}");
            return ExitCode::FAILURE;
        }
    };
    for checked in playlist::check(&YtDlp::new("yt-dlp"), &tracklist::parse(&text)) {
        let name = checked.entry.display_name();
        match &checked.video {
            Ok(video) if video.has_non_music() => println!(
                "{:02} ! {name}: \"{}\" has {:.0}s of non-music in {} part(s)",
                checked.entry.position,
                video.title,
                video.non_music_seconds(),
                video.non_music.len()
            ),
            Ok(video) => println!(
                "{:02}   {name}: \"{}\"",
                checked.entry.position, video.title
            ),
            Err(error) => println!("{:02} x {name}: {error}", checked.entry.position),
        }
    }
    ExitCode::SUCCESS
}
