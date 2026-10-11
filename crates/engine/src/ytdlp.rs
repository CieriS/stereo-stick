//! The yt-dlp command lines, built as plain argument lists so that they can
//! be tested without running anything.

use crate::playlist::{Job, NonMusic};
use crate::tracklist::Source;
use std::ffi::OsString;

/// One line per progress update: bytes downloaded and total bytes (or `NA`).
const PROGRESS_TEMPLATE: &str = "download:PROGRESS %(progress.downloaded_bytes)s %(progress.total_bytes,progress.total_bytes_estimate)s";

/// How the name of every file of the track at `position` begins.
pub fn file_prefix(position: usize) -> String {
    format!("{position:02}_")
}

/// Reads a progress line of yt-dlp as a fraction between 0 and 1. Any other
/// line, or a download of unknown size, gives `None`.
pub fn parse_progress(line: &str) -> Option<f32> {
    let mut numbers = line.trim().strip_prefix("PROGRESS ")?.split_whitespace();
    let downloaded: f64 = numbers.next()?.parse().ok()?;
    let total: f64 = numbers.next()?.parse().ok()?;
    (total > 0.0 && downloaded >= 0.0).then(|| (downloaded / total).min(1.0) as f32)
}

/// Arguments that describe a video as JSON without downloading it.
pub fn probe_args(source: &Source) -> Vec<OsString> {
    [
        "--dump-json",
        "--no-playlist",
        "--no-warnings",
        "--sponsorblock-mark",
        "music_offtopic",
        "--",
    ]
    .into_iter()
    .map(OsString::from)
    .chain([source.to_ytdlp_arg().into()])
    .collect()
}

/// Arguments that download one track as a tagged MP3.
pub fn download_args(job: &Job<'_>) -> Vec<OsString> {
    let mut args: Vec<OsString> = [
        "--extract-audio",
        "--audio-format",
        "mp3",
        "--audio-quality",
        "320K",
        "--no-playlist",
        "--no-warnings",
        "--add-metadata",
        // Car stereos show "NA" for a missing artist: fall back to the channel.
        "--parse-metadata",
        "%(artist,uploader)s:%(artist)s",
        "--parse-metadata",
        "%(album_artist,artist,uploader)s:%(album_artist)s",
        "--embed-thumbnail",
        // Many car stereos cannot show WebP covers.
        "--convert-thumbnails",
        "jpg",
        "--newline",
        "--progress-template",
        PROGRESS_TEMPLATE,
    ]
    .into_iter()
    .map(OsString::from)
    .collect();
    args.push("--parse-metadata".into());
    args.push(format!("{}:%(album)s", literal(job.playlist)).into());
    if job.non_music == NonMusic::Remove {
        args.push("--sponsorblock-remove".into());
        args.push("music_offtopic".into());
    }
    args.push("--paths".into());
    args.push(job.directory.into());
    args.push("--output".into());
    args.push(format!("{}%(title)s.%(ext)s", file_prefix(job.entry.position)).into());
    args.push("--".into());
    args.push(job.entry.source.to_ytdlp_arg().into());
    args
}

/// Writes `text` as the source of a `--parse-metadata FROM:TO` so that yt-dlp
/// takes it literally. A source made only of letters and underscores is read
/// as the name of a field (`sc_song_ita` would give the album "NA"), so an
/// always-empty field is put in front. Backslashes cannot be written
/// unambiguously next to the `:` separator and are dropped.
fn literal(text: &str) -> String {
    let escaped = text
        .replace('\\', "")
        .replace('%', "%%")
        .replace(':', "\\:");
    format!("%(_literal|)s{escaped}")
}

/// The reason of a failure, taken from what yt-dlp wrote on standard error.
pub fn failure_message(stderr: &str) -> String {
    let lines = || stderr.lines().map(str::trim).filter(|l| !l.is_empty());
    let line = lines()
        .rfind(|l| l.starts_with("ERROR:"))
        .or_else(|| lines().next_back())
        .unwrap_or("yt-dlp failed without a message");
    line.trim_start_matches("ERROR:").trim().to_owned()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::tracklist::Entry;

    fn entry(position: usize, source: Source) -> Entry {
        Entry {
            position,
            label: None,
            source,
        }
    }

    fn strings(args: &[OsString]) -> Vec<String> {
        args.iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    fn value_after<'a>(args: &'a [String], flag: &str) -> &'a str {
        let index = args.iter().position(|a| a == flag).expect(flag);
        &args[index + 1]
    }

    fn job<'a>(entry: &'a Entry, playlist: &'a str, non_music: NonMusic) -> Job<'a> {
        Job {
            entry,
            playlist,
            directory: Path::new("music/mix"),
            non_music,
        }
    }

    #[test]
    fn probe_describes_without_downloading() {
        let args = strings(&probe_args(&Source::Search("a b".into())));
        assert!(args.contains(&"--dump-json".to_owned()));
        assert_eq!(value_after(&args, "--sponsorblock-mark"), "music_offtopic");
        assert_eq!(&args[args.len() - 2..], ["--", "ytsearch1:a b"]);
    }

    #[test]
    fn download_produces_a_numbered_mp3_in_the_playlist_folder() {
        let entry = entry(7, Source::Url("https://example.com/v".into()));
        let args = strings(&download_args(&job(&entry, "My Mix", NonMusic::Keep)));
        assert_eq!(value_after(&args, "--audio-format"), "mp3");
        assert_eq!(value_after(&args, "--convert-thumbnails"), "jpg");
        assert_eq!(value_after(&args, "--paths"), "music/mix");
        assert_eq!(value_after(&args, "--output"), "07_%(title)s.%(ext)s");
        assert_eq!(&args[args.len() - 2..], ["--", "https://example.com/v"]);
        assert!(!args.contains(&"--sponsorblock-remove".to_owned()));
    }

    #[test]
    fn positions_above_99_are_not_truncated() {
        let entry = entry(123, Source::Search("x".into()));
        let args = strings(&download_args(&job(&entry, "p", NonMusic::Keep)));
        assert_eq!(value_after(&args, "--output"), "123_%(title)s.%(ext)s");
    }

    #[test]
    fn removing_non_music_parts_is_opt_in() {
        let entry = entry(1, Source::Search("x".into()));
        let args = strings(&download_args(&job(&entry, "p", NonMusic::Remove)));
        assert_eq!(
            value_after(&args, "--sponsorblock-remove"),
            "music_offtopic"
        );
    }

    #[test]
    fn a_link_that_looks_like_an_option_stays_an_argument() {
        let entry = entry(1, Source::Url("--exec=evil".into()));
        let args = strings(&download_args(&job(&entry, "p", NonMusic::Keep)));
        let separator = args.iter().position(|a| a == "--").unwrap();
        assert_eq!(args[separator + 1], "--exec=evil");
        assert_eq!(separator + 2, args.len());
    }

    #[test]
    fn the_playlist_name_is_written_literally_in_the_album() {
        let cases = [
            ("sc_song_ita", "%(_literal|)ssc_song_ita:%(album)s"),
            ("My Mix 1", "%(_literal|)sMy Mix 1:%(album)s"),
            (
                "AC: 100% (live)",
                "%(_literal|)sAC\\: 100%% (live):%(album)s",
            ),
            ("back\\slash", "%(_literal|)sbackslash:%(album)s"),
            ("", "%(_literal|)s:%(album)s"),
        ];
        for (playlist, expected) in cases {
            let entry = entry(1, Source::Search("x".into()));
            let args = strings(&download_args(&job(&entry, playlist, NonMusic::Keep)));
            assert!(args.contains(&expected.to_owned()), "{playlist}: {args:?}");
        }
    }

    #[test]
    fn download_reports_progress_line_by_line() {
        let entry = entry(1, Source::Search("x".into()));
        let args = strings(&download_args(&job(&entry, "p", NonMusic::Keep)));
        assert!(args.contains(&"--newline".to_owned()));
        assert!(value_after(&args, "--progress-template").starts_with("download:PROGRESS "));
    }

    #[test]
    fn progress_lines_become_fractions() {
        assert_eq!(parse_progress("PROGRESS 50 200"), Some(0.25));
        assert_eq!(parse_progress("  PROGRESS 1024 4096.0\r"), Some(0.25));
        assert_eq!(parse_progress("PROGRESS 300 200"), Some(1.0));
        assert_eq!(parse_progress("PROGRESS 0 200"), Some(0.0));
    }

    #[test]
    fn lines_that_are_not_usable_progress_are_ignored() {
        for line in [
            "",
            "[download] Destination: 01_x.webm",
            "PROGRESS",
            "PROGRESS 10",
            "PROGRESS 10 NA",
            "PROGRESS NA NA",
            "PROGRESS 10 0",
            "PROGRESS -5 10",
            "PROGRESSION 1 2",
        ] {
            assert_eq!(parse_progress(line), None, "{line}");
        }
    }

    #[test]
    fn file_prefix_is_padded_to_two_digits() {
        assert_eq!(file_prefix(3), "03_");
        assert_eq!(file_prefix(42), "42_");
        assert_eq!(file_prefix(123), "123_");
    }

    #[test]
    fn failure_message_prefers_the_last_error_line() {
        let stderr = "WARNING: something\nERROR: first\n\nERROR: [youtube] abc: Video unavailable\nsome trailing noise\n";
        assert_eq!(failure_message(stderr), "[youtube] abc: Video unavailable");
    }

    #[test]
    fn failure_message_without_error_lines() {
        assert_eq!(failure_message("  \nboom\n"), "boom");
        assert_eq!(failure_message(""), "yt-dlp failed without a message");
    }
}
