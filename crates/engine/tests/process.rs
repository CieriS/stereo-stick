//! Runs the real process adapter against a stand-in for yt-dlp: a shell
//! script that answers like it, so no network and no real download is needed.
#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use engine::files;
use engine::playlist::{self, DownloadError, Downloader, Event, NonMusic, Request};
use engine::process::YtDlp;
use engine::tracklist::{self, Source};

/// Writes its arguments, one per line, to `args.log` next to itself. When
/// probing it prints a video description; when downloading it reports
/// progress and creates an empty MP3. Any source containing "missing" fails,
/// and a failed download leaves a cover and a partial file behind.
const STAND_IN: &str = r#"#!/bin/sh
here=$(dirname "$0")
for arg in "$@"; do printf '%s\n' "$arg" >> "$here/args.log"; done
[ "$1" = --version ] && { echo 2026.01.01; exit 0; }
probe=no; dir=; output=
while [ $# -gt 1 ]; do
  case "$1" in
    --dump-json) probe=yes;;
    --paths) dir=$2; shift;;
    --output) output=$2; shift;;
  esac
  shift
done
source=$1
name="${output%%%*}Song"
fail() {
  echo "WARNING: noise" >&2
  echo "ERROR: [youtube] xyz: Video unavailable" >&2
  exit 1
}
if [ "$probe" = yes ]; then
  case "$source" in *missing*) fail;; esac
  echo '{"id":"xyz","title":"A Song","duration":200,"sponsorblock_chapters":[{"start_time":0,"end_time":15,"category":"music_offtopic"}]}'
  exit 0
fi
mkdir -p "$dir"
echo "[download] Destination: $name.webm"
echo "PROGRESS 50 200"
case "$source" in *missing*)
  : > "$dir/$name.jpg"; : > "$dir/$name.webm.part"
  fail;;
esac
echo "PROGRESS 200 200"
: > "$dir/$name.mp3"
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

fn request<'a>(
    out: &'a Path,
    entries: &'a [tracklist::Entry],
    existing: &'a [String],
) -> Request<'a> {
    Request {
        playlist: "sc_song_ita",
        directory: out,
        entries,
        non_music: NonMusic::Remove,
        existing,
    }
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
    let mut progress = Vec::new();
    let report = playlist::download(
        &sandbox.downloader(),
        &request(&out, &entries, &[]),
        &mut |event| {
            if let Event::Progress { position, fraction } = event {
                progress.push((position, fraction));
            }
        },
    );

    assert_eq!(report.downloaded(), 2);
    let failed: Vec<_> = report
        .failed()
        .map(|(entry, error)| (entry.position, error.to_string()))
        .collect();
    assert_eq!(failed, [(2, "[youtube] xyz: Video unavailable".to_owned())]);
    assert_eq!(
        progress,
        [(1, 0.25), (1, 1.0), (2, 0.25), (3, 0.25), (3, 1.0)]
    );
    // The cover and the partial file of the failed track are gone.
    assert_eq!(files::list(&out).unwrap(), ["01_Song.mp3", "03_Song.mp3"]);

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
fn a_second_run_downloads_only_what_is_missing() {
    let sandbox = Sandbox::new("resume");
    let out = sandbox.path("out");
    let entries = tracklist::parse("first song\nsecond song");
    let downloader = sandbox.downloader();
    fs::create_dir_all(&out).unwrap();
    fs::write(out.join("01_Already here.mp3"), b"").unwrap();

    let existing = files::list(&out).unwrap();
    let report = playlist::download(
        &downloader,
        &request(&out, &entries, &existing),
        &mut |_| {},
    );
    assert_eq!(report.already_present(), 1);
    assert_eq!(report.downloaded(), 1);
    assert_eq!(
        files::list(&out).unwrap(),
        ["01_Already here.mp3", "02_Song.mp3"]
    );

    let existing = files::list(&out).unwrap();
    let report = playlist::download(
        &downloader,
        &request(&out, &entries, &existing),
        &mut |_| {},
    );
    assert_eq!(report.already_present(), 2);
    let log = fs::read_to_string(sandbox.path("args.log")).unwrap();
    assert_eq!(log.lines().filter(|line| *line == "--output").count(), 1);
}

#[test]
fn a_missing_program_is_reported_as_unavailable() {
    let sandbox = Sandbox::new("unavailable");
    assert!(sandbox.downloader().is_available());
    let downloader = YtDlp::new(sandbox.path("no-such-program"));
    assert!(!downloader.is_available());
    let error = downloader.probe(&Source::Search("x".into())).unwrap_err();
    assert!(matches!(error, DownloadError::Unavailable(_)));

    let entries = tracklist::parse("a\nb");
    let out = sandbox.path("out");
    let report = playlist::download(&downloader, &request(&out, &entries, &[]), &mut |_| {});
    assert_eq!(report.downloaded(), 0);
    assert_eq!(report.failed().count(), 2);
}

#[test]
fn a_lot_of_error_output_does_not_block() {
    let sandbox = Sandbox::new("noisy");
    fs::write(
        sandbox.path("yt-dlp"),
        "#!/bin/sh\ni=0; while [ $i -lt 20000 ]; do echo 'WARNING: a long line of noise to fill the pipe' >&2; i=$((i+1)); done\necho 'ERROR: boom' >&2; exit 1\n",
    )
    .unwrap();
    let error = sandbox
        .downloader()
        .probe(&Source::Search("x".into()))
        .unwrap_err();
    assert_eq!(error.to_string(), "boom");
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
