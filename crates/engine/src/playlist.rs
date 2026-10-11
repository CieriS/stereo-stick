//! Checking and downloading a whole playlist through the [`Downloader`]
//! port. A track that fails never stops the others: every track gets its own
//! outcome, and the report is built from those.

use std::fmt;
use std::path::Path;

use crate::probe::{ProbeError, VideoInfo};
use crate::tracklist::{Entry, Source};

#[derive(Debug)]
pub enum DownloadError {
    /// The download tool could not be started at all.
    Unavailable(String),
    /// The tool ran and reported a failure for this track.
    Failed(String),
    /// The tool answered with something that cannot be read.
    Unreadable(ProbeError),
}

impl fmt::Display for DownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(reason) => write!(f, "download tool not available: {reason}"),
            Self::Failed(reason) => f.write_str(reason),
            Self::Unreadable(error) => error.fmt(f),
        }
    }
}

impl std::error::Error for DownloadError {}

impl From<ProbeError> for DownloadError {
    fn from(error: ProbeError) -> Self {
        Self::Unreadable(error)
    }
}

/// What to do with the parts of a video that are not music.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NonMusic {
    /// Download the audio of the whole video.
    #[default]
    Keep,
    /// Cut the reviewed non-music parts out of the audio.
    Remove,
}

/// One track to download into a playlist folder.
#[derive(Debug, Clone, Copy)]
pub struct Job<'a> {
    pub entry: &'a Entry,
    /// Name of the playlist: car stereos group tracks by the album tag, so
    /// it is written there.
    pub playlist: &'a str,
    pub directory: &'a Path,
    pub non_music: NonMusic,
}

/// The outside world as the engine sees it: something that can describe a
/// video and download a track.
pub trait Downloader {
    fn probe(&self, source: &Source) -> Result<VideoInfo, DownloadError>;
    fn download(&self, job: &Job<'_>) -> Result<(), DownloadError>;
}

/// A track together with what was found out about its video.
#[derive(Debug)]
pub struct Checked {
    pub entry: Entry,
    pub video: Result<VideoInfo, DownloadError>,
}

impl Checked {
    /// True when the video is known to contain parts that are not music.
    pub fn needs_attention(&self) -> bool {
        self.video.as_ref().is_ok_and(VideoInfo::has_non_music)
    }
}

/// Describes every track without downloading anything, so that problems can
/// be shown before the download starts.
pub fn check(downloader: &impl Downloader, entries: &[Entry]) -> Vec<Checked> {
    entries
        .iter()
        .map(|entry| Checked {
            entry: entry.clone(),
            video: downloader.probe(&entry.source),
        })
        .collect()
}

#[derive(Debug)]
pub struct TrackResult {
    pub entry: Entry,
    pub outcome: Result<(), DownloadError>,
}

/// The outcome of a playlist, track by track.
#[derive(Debug)]
pub struct Report {
    pub playlist: String,
    pub tracks: Vec<TrackResult>,
}

impl Report {
    pub fn downloaded(&self) -> usize {
        self.tracks.iter().filter(|t| t.outcome.is_ok()).count()
    }

    pub fn failed(&self) -> impl Iterator<Item = (&Entry, &DownloadError)> {
        self.tracks
            .iter()
            .filter_map(|t| t.outcome.as_ref().err().map(|error| (&t.entry, error)))
    }
}

/// Downloads every track into `directory`, tagged as part of `playlist`.
pub fn download(
    downloader: &impl Downloader,
    playlist: &str,
    directory: &Path,
    entries: &[Entry],
    non_music: NonMusic,
) -> Report {
    let tracks = entries
        .iter()
        .map(|entry| TrackResult {
            entry: entry.clone(),
            outcome: downloader.download(&Job {
                entry,
                playlist,
                directory,
                non_music,
            }),
        })
        .collect();
    Report {
        playlist: playlist.to_owned(),
        tracks,
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;
    use crate::probe::Segment;
    use crate::tracklist;

    /// Fails every track whose search contains "missing"; flags as partly
    /// non-music every track whose search contains "videoclip".
    #[derive(Default)]
    struct FakeDownloader {
        downloads: RefCell<Vec<String>>,
    }

    impl Downloader for FakeDownloader {
        fn probe(&self, source: &Source) -> Result<VideoInfo, DownloadError> {
            let query = source.to_ytdlp_arg();
            if query.contains("missing") {
                return Err(DownloadError::Failed("Video unavailable".into()));
            }
            let non_music = if query.contains("videoclip") {
                vec![Segment {
                    start: 0.0,
                    end: 12.0,
                }]
            } else {
                Vec::new()
            };
            Ok(VideoInfo {
                id: "id".into(),
                title: query,
                url: None,
                uploader: None,
                duration: Some(200.0),
                non_music,
            })
        }

        fn download(&self, job: &Job<'_>) -> Result<(), DownloadError> {
            let query = job.entry.source.to_ytdlp_arg();
            self.downloads.borrow_mut().push(format!(
                "{}|{}|{}|{:?}",
                job.entry.position,
                job.playlist,
                job.directory.display(),
                job.non_music
            ));
            if query.contains("missing") {
                Err(DownloadError::Failed("Video unavailable".into()))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn check_flags_videos_with_non_music_parts() {
        let entries = tracklist::parse("plain song\nsong videoclip\nmissing song");
        let checked = check(&FakeDownloader::default(), &entries);
        let flags: Vec<_> = checked.iter().map(Checked::needs_attention).collect();
        assert_eq!(flags, [false, true, false]);
        assert!(checked[2].video.is_err());
        assert_eq!(checked[1].entry, entries[1]);
    }

    #[test]
    fn a_failed_track_does_not_stop_the_others() {
        let entries = tracklist::parse("one\nmissing two\nthree");
        let downloader = FakeDownloader::default();
        let report = download(
            &downloader,
            "mix",
            Path::new("out"),
            &entries,
            NonMusic::Remove,
        );
        assert_eq!(report.playlist, "mix");
        assert_eq!(report.downloaded(), 2);
        let failed: Vec<_> = report
            .failed()
            .map(|(entry, error)| (entry.position, error.to_string()))
            .collect();
        assert_eq!(failed, [(2, "Video unavailable".to_owned())]);
        assert_eq!(
            *downloader.downloads.borrow(),
            ["1|mix|out|Remove", "2|mix|out|Remove", "3|mix|out|Remove"]
        );
    }

    #[test]
    fn an_empty_playlist_gives_an_empty_report() {
        let report = download(
            &FakeDownloader::default(),
            "empty",
            Path::new("out"),
            &[],
            NonMusic::Keep,
        );
        assert_eq!(report.downloaded(), 0);
        assert_eq!(report.failed().count(), 0);
        assert!(check(&FakeDownloader::default(), &[]).is_empty());
    }

    #[test]
    fn errors_read_well() {
        assert_eq!(
            DownloadError::Unavailable("not found".into()).to_string(),
            "download tool not available: not found"
        );
        assert_eq!(DownloadError::Failed("boom".into()).to_string(), "boom");
    }
}
