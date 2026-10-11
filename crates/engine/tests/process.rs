//! Runs the real process adapter against a stand-in for yt-dlp: a shell
//! script that answers like it, so no network and no real download is needed.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use engine::playlist::{self, DownloadError, Downloader, NonMusic};
use engine::process::YtDlp;
use engine::tracklist::{self, Source};

/// Writes its arguments, one per line, to `args.log` next to itself. Fails
/// on any source containing "missing"; otherwise prints a video description
/// when probing and creates an empty MP3 when downloading.
const STAND_IN: &str = r#"#!/bin/sh
here=$(dirname "$0")
for arg in "$@"; do printf '%s\n' "$arg" >> "$here/args.log"; done
for arg in "$@"; do source=$arg; done
case "$source" in *missing*)
  echo "WARNING: noise" >&2
  echo "ERROR: [youtube] xyz: Video unavailable" >&2
  exit 1;;
esac
case " $* " in
  *" --dump-json "*)
    echo '{"id":"xyz","title":"A Song","duration":200,"sponsorblock_chapters":[{"start_time":0,"end_time":15,"category":"music_offtopic"}]}';;
  *)
    while [ "$1" != "--paths" ]; do shift; done
    mkdir -p "$2" && : > "$2/$(ls "$2" | wc -l | tr -d ' ').mp3";;
esac
"#;

struct Sandbox(PathBuf);

impl Sandbox {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("engine-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let program = dir.join("yt-dlp");
        fs::write(&program, STAND_IN).unwrap();
        fs::set_permissions(&program, fs::Permissions::from_mode(0o755)).unwrap();
        Self(dir)
    }

    fn downloader(&self) -> YtDlp {
        YtDlp::new(self.0.join("yt-dlp"))
    }

    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for Sandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn mp3_count(dir: &Path) -> usize {
    fs::read_dir(dir).map_or(0, Iterator::count)
}

#[test]
fn probe_reads_the_description_of_a_video() {
    let sandbox = Sandbox::new("probe");
    let info = sandbox
        .downloader()
        .probe(&Source::Search("a song".into()))
        .unwrap();
    assert_eq!(info.title, "A Song");
    assert_eq!(info.non_music_seconds(), 15.0);
    assert_eq!(info.music_seconds(), Some(185.0));
}

#[test]
fn probe_reports_the_reason_of_a_failure() {
    let sandbox = Sandbox::new("probe-failure");
    let error = sandbox
        .downloader()
        .probe(&Source::Search("missing song".into()))
        .unwrap_err();
    assert!(matches!(error, DownloadError::Failed(_)));
    assert_eq!(error.to_string(), "[youtube] xyz: Video unavailable");
}

#[test]
fn a_playlist_is_downloaded_track_by_track() {
    let sandbox = Sandbox::new("download");
    let out = sandbox.path("out dir");
    let entries = tracklist::parse("first song\nmissing song\n# Third\nhttps://example.com/v");
    let report = playlist::download(
        &sandbox.downloader(),
        "sc_song_ita",
        &out,
        &entries,
        NonMusic::Remove,
    );

    assert_eq!(report.downloaded(), 2);
    assert_eq!(mp3_count(&out), 2);
    let failed: Vec<_> = report.failed().map(|(entry, _)| entry.position).collect();
    assert_eq!(failed, [2]);

    let log = fs::read_to_string(sandbox.path("args.log")).unwrap();
    for expected in [
        "%(_literal|)ssc_song_ita:%(album)s",
        "01_%(title)s.%(ext)s",
        "03_%(title)s.%(ext)s",
        "ytsearch1:first song",
        "https://example.com/v",
        "music_offtopic",
    ] {
        assert!(
            log.lines().any(|line| line == expected),
            "missing {expected}"
        );
    }
    assert!(log.lines().any(|line| Path::new(line) == out));
}

#[test]
fn a_missing_program_is_reported_as_unavailable() {
    let sandbox = Sandbox::new("unavailable");
    let downloader = YtDlp::new(sandbox.path("no-such-program"));
    let error = downloader.probe(&Source::Search("x".into())).unwrap_err();
    assert!(matches!(error, DownloadError::Unavailable(_)));

    let entries = tracklist::parse("a\nb");
    let report = playlist::download(
        &downloader,
        "mix",
        &sandbox.path("out"),
        &entries,
        NonMusic::Keep,
    );
    assert_eq!(report.downloaded(), 0);
    assert_eq!(report.failed().count(), 2);
}

#[test]
fn unreadable_output_is_an_error_not_a_crash() {
    let sandbox = Sandbox::new("unreadable");
    fs::write(sandbox.path("yt-dlp"), "#!/bin/sh\necho 'not json'\n").unwrap();
    let error = sandbox
        .downloader()
        .probe(&Source::Search("x".into()))
        .unwrap_err();
    assert!(matches!(error, DownloadError::Unreadable(_)));
}
