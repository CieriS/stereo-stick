//! Checking and downloading a whole playlist through the [`Downloader`]
//! port. A track that fails never stops the others: every track gets its own
//! outcome, and the report is built from those.

use std::fmt;
use std::path::Path;

use crate::files;
use crate::probe::{ProbeError, VideoInfo};
use crate::tracklist::{Entry, Source};

#[derive(Debug, Clone, PartialEq)]
pub enum DownloadError {
    /// The download tool could not be started at all.
    Unavailable(String),
    /// The tool ran and reported a failure for this track.
    Failed(String),
    /// The tool answered with something that cannot be read.
    Unreadable(String),
}

impl fmt::Display for DownloadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(reason) => write!(f, "download tool not available: {reason}"),
            Self::Failed(reason) | Self::Unreadable(reason) => f.write_str(reason),
        }
    }
}

impl std::error::Error for DownloadError {}

impl From<ProbeError> for DownloadError {
    fn from(error: ProbeError) -> Self {
        Self::Unreadable(error.to_string())
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

    /// Downloads one track, calling `on_progress` with the fraction (0 to 1)
    /// downloaded so far. After a failure nothing unfinished is left on disk.
    fn download(
        &self,
        job: &Job<'_>,
        on_progress: &mut dyn FnMut(f32),
    ) -> Result<(), DownloadError>;
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

/// How a track ended.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Downloaded,
    /// Its MP3 was already in the folder; nothing was downloaded.
    AlreadyPresent,
    Failed(DownloadError),
}

/// What happens while a playlist downloads, in order, for whoever shows it.
#[derive(Debug, Clone, PartialEq)]
pub enum Event<'a> {
    Started {
        position: usize,
    },
    Progress {
        position: usize,
        fraction: f32,
    },
    Finished {
        position: usize,
        outcome: &'a Outcome,
    },
}

#[derive(Debug)]
pub struct TrackResult {
    pub entry: Entry,
    pub outcome: Outcome,
}

/// The outcome of a playlist, track by track.
#[derive(Debug)]
pub struct Report {
    pub playlist: String,
    pub tracks: Vec<TrackResult>,
}

impl Report {
    fn count(&self, wanted: &Outcome) -> usize {
        self.tracks.iter().filter(|t| t.outcome == *wanted).count()
    }

    pub fn downloaded(&self) -> usize {
        self.count(&Outcome::Downloaded)
    }

    pub fn already_present(&self) -> usize {
        self.count(&Outcome::AlreadyPresent)
    }

    pub fn failed(&self) -> impl Iterator<Item = (&Entry, &DownloadError)> {
        self.tracks.iter().filter_map(|t| match &t.outcome {
            Outcome::Failed(error) => Some((&t.entry, error)),
            _ => None,
        })
    }
}

/// A playlist to download.
#[derive(Debug, Clone, Copy)]
pub struct Request<'a> {
    pub playlist: &'a str,
    pub directory: &'a Path,
    pub entries: &'a [Entry],
    pub non_music: NonMusic,
    /// Names of the files already in `directory`: tracks whose MP3 is among
    /// them are skipped. Pass an empty slice to download everything again.
    pub existing: &'a [String],
}

/// Downloads every track of the request that is not already there, telling
/// `on_event` what happens as it happens.
pub fn download(
    downloader: &impl Downloader,
    request: &Request<'_>,
    on_event: &mut dyn FnMut(Event<'_>),
) -> Report {
    let mut tracks = Vec::with_capacity(request.entries.len());
    for entry in request.entries {
        let position = entry.position;
        on_event(Event::Started { position });
        let outcome = if files::is_downloaded(position, request.existing) {
            Outcome::AlreadyPresent
        } else {
            let job = Job {
                entry,
                playlist: request.playlist,
                directory: request.directory,
                non_music: request.non_music,
            };
            let mut on_progress = |fraction| on_event(Event::Progress { position, fraction });
            match downloader.download(&job, &mut on_progress) {
                Ok(()) => Outcome::Downloaded,
                Err(error) => Outcome::Failed(error),
            }
        };
        on_event(Event::Finished {
            position,
            outcome: &outcome,
        });
        tracks.push(TrackResult {
            entry: entry.clone(),
            outcome,
        });
    }
    Report {
        playlist: request.playlist.to_owned(),
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

        fn download(
            &self,
            job: &Job<'_>,
            on_progress: &mut dyn FnMut(f32),
        ) -> Result<(), DownloadError> {
            let query = job.entry.source.to_ytdlp_arg();
            self.downloads.borrow_mut().push(format!(
                "{}|{}|{}|{:?}",
                job.entry.position,
                job.playlist,
                job.directory.display(),
                job.non_music
            ));
            on_progress(0.5);
            if query.contains("missing") {
                Err(DownloadError::Failed("Video unavailable".into()))
            } else {
                on_progress(1.0);
                Ok(())
            }
        }
    }

    fn request<'a>(entries: &'a [Entry], existing: &'a [String]) -> Request<'a> {
        Request {
            playlist: "mix",
            directory: Path::new("out"),
            entries,
            non_music: NonMusic::Remove,
            existing,
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
        let report = download(&downloader, &request(&entries, &[]), &mut |_| {});
        assert_eq!(report.playlist, "mix");
        assert_eq!(report.downloaded(), 2);
        assert_eq!(report.already_present(), 0);
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
    fn tracks_already_in_the_folder_are_not_downloaded_again() {
        let entries = tracklist::parse("one\ntwo\nthree");
        let existing = ["01_One.mp3".to_owned(), "03_Three.jpg".to_owned()];
        let downloader = FakeDownloader::default();
        let report = download(&downloader, &request(&entries, &existing), &mut |_| {});
        assert_eq!(report.already_present(), 1);
        assert_eq!(report.downloaded(), 2);
        assert_eq!(report.tracks[0].outcome, Outcome::AlreadyPresent);
        assert_eq!(
            *downloader.downloads.borrow(),
            ["2|mix|out|Remove", "3|mix|out|Remove"]
        );
    }

    #[test]
    fn events_tell_the_story_in_order() {
        let entries = tracklist::parse("one\nmissing two\nthree");
        let existing = ["03_Three.mp3".to_owned()];
        let mut story = Vec::new();
        download(
            &FakeDownloader::default(),
            &request(&entries, &existing),
            &mut |event| {
                story.push(match event {
                    Event::Started { position } => format!("{position} started"),
                    Event::Progress { position, fraction } => format!("{position} at {fraction}"),
                    Event::Finished { position, outcome } => match outcome {
                        Outcome::Downloaded => format!("{position} downloaded"),
                        Outcome::AlreadyPresent => format!("{position} present"),
                        Outcome::Failed(error) => format!("{position} failed: {error}"),
                    },
                });
            },
        );
        assert_eq!(
            story,
            [
                "1 started",
                "1 at 0.5",
                "1 at 1",
                "1 downloaded",
                "2 started",
                "2 at 0.5",
                "2 failed: Video unavailable",
                "3 started",
                "3 present",
            ]
        );
    }

    #[test]
    fn an_empty_playlist_gives_an_empty_report() {
        let mut events = 0;
        let report = download(&FakeDownloader::default(), &request(&[], &[]), &mut |_| {
            events += 1
        });
        assert_eq!(report.downloaded(), 0);
        assert_eq!(report.failed().count(), 0);
        assert_eq!(events, 0);
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
