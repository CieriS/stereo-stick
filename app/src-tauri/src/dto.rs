//! The shapes the window receives, and how the engine's types become them.

use engine::playlist::{Checked, Event, Outcome};
use engine::stick::Copy;
use engine::tracklist::Entry;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Track {
    pub position: usize,
    pub name: String,
}

impl From<&Entry> for Track {
    fn from(entry: &Entry) -> Self {
        Self {
            position: entry.position,
            name: entry.display_name().to_owned(),
        }
    }
}

/// What the check found out about one track.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CheckResult {
    #[serde(rename_all = "camelCase")]
    Found {
        position: usize,
        title: String,
        non_music_seconds: f64,
        non_music_parts: usize,
    },
    Unavailable {
        position: usize,
        error: String,
    },
}

impl From<&Checked> for CheckResult {
    fn from(checked: &Checked) -> Self {
        let position = checked.entry.position;
        match &checked.video {
            Ok(video) => Self::Found {
                position,
                title: video.title.clone(),
                non_music_seconds: video.non_music_seconds(),
                non_music_parts: video.non_music.len(),
            },
            Err(error) => Self::Unavailable {
                position,
                error: error.to_string(),
            },
        }
    }
}

/// What happens to one track while the playlist downloads.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Progress {
    Started {
        position: usize,
    },
    #[serde(rename = "progress")]
    Advanced {
        position: usize,
        fraction: f32,
    },
    Downloaded {
        position: usize,
    },
    AlreadyPresent {
        position: usize,
    },
    Stopped {
        position: usize,
    },
    Failed {
        position: usize,
        error: String,
    },
}

impl From<Event<'_>> for Progress {
    fn from(event: Event<'_>) -> Self {
        match event {
            Event::Started { position } => Self::Started { position },
            Event::Progress { position, fraction } => Self::Advanced { position, fraction },
            Event::Finished { position, outcome } => match outcome {
                Outcome::Downloaded => Self::Downloaded { position },
                Outcome::AlreadyPresent => Self::AlreadyPresent { position },
                Outcome::Stopped => Self::Stopped { position },
                Outcome::Failed(error) => Self::Failed {
                    position,
                    error: error.to_string(),
                },
            },
        }
    }
}

/// What happens while the music folder is copied to the stick.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum StickProgress {
    /// How many tracks will be copied and how many are already up to date.
    #[serde(rename_all = "camelCase")]
    Planned {
        to_copy: usize,
        up_to_date: usize,
    },
    Copied {
        path: String,
    },
    Failed {
        path: String,
        error: String,
    },
    /// The copy is over; `stopped` tracks were not reached.
    Finished {
        stopped: usize,
    },
}

impl From<Copy<'_>> for StickProgress {
    fn from(event: Copy<'_>) -> Self {
        match event {
            Copy::Copied { track } => Self::Copied {
                path: track.path.clone(),
            },
            Copy::Failed { track, error } => Self::Failed {
                path: track.path.clone(),
                error,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use engine::playlist::DownloadError;
    use engine::probe::{Segment, VideoInfo};
    use engine::tracklist;
    use serde_json::{json, to_value};

    use super::*;

    fn entry() -> Entry {
        tracklist::parse("# Nice name\nsome search").remove(0)
    }

    #[test]
    fn a_track_shows_its_name() {
        let track = Track::from(&entry());
        assert_eq!(
            to_value(track).unwrap(),
            json!({"position": 1, "name": "Nice name"})
        );
    }

    #[test]
    fn a_found_video_reports_its_non_music_parts() {
        let checked = Checked {
            entry: entry(),
            video: Ok(VideoInfo {
                id: "id".into(),
                title: "Title".into(),
                url: None,
                uploader: None,
                duration: Some(100.0),
                non_music: vec![
                    Segment {
                        start: 0.0,
                        end: 10.0,
                    },
                    Segment {
                        start: 90.0,
                        end: 100.0,
                    },
                ],
            }),
        };
        assert_eq!(
            to_value(CheckResult::from(&checked)).unwrap(),
            json!({"kind": "found", "position": 1, "title": "Title",
                   "nonMusicSeconds": 20.0, "nonMusicParts": 2})
        );
    }

    #[test]
    fn a_video_that_cannot_be_found_reports_why() {
        let checked = Checked {
            entry: entry(),
            video: Err(DownloadError::Failed("Video unavailable".into())),
        };
        assert_eq!(
            to_value(CheckResult::from(&checked)).unwrap(),
            json!({"kind": "unavailable", "position": 1, "error": "Video unavailable"})
        );
    }

    #[test]
    fn stick_events_have_their_shapes() {
        let track = engine::stick::Track {
            path: "rock/01_A.mp3".into(),
            size: 3,
            modified: None,
        };
        let cases = [
            (
                StickProgress::Planned {
                    to_copy: 2,
                    up_to_date: 5,
                },
                json!({"kind": "planned", "toCopy": 2, "upToDate": 5}),
            ),
            (
                StickProgress::from(Copy::Copied { track: &track }),
                json!({"kind": "copied", "path": "rock/01_A.mp3"}),
            ),
            (
                StickProgress::from(Copy::Failed {
                    track: &track,
                    error: "disk full".into(),
                }),
                json!({"kind": "failed", "path": "rock/01_A.mp3", "error": "disk full"}),
            ),
            (
                StickProgress::Finished { stopped: 1 },
                json!({"kind": "finished", "stopped": 1}),
            ),
        ];
        for (event, expected) in cases {
            assert_eq!(to_value(event).unwrap(), expected);
        }
    }

    #[test]
    fn every_event_has_its_shape() {
        let failed = Outcome::Failed(DownloadError::Failed("boom".into()));
        let cases = [
            (
                Event::Started { position: 2 },
                json!({"kind": "started", "position": 2}),
            ),
            (
                Event::Progress {
                    position: 2,
                    fraction: 0.5,
                },
                json!({"kind": "progress", "position": 2, "fraction": 0.5}),
            ),
            (
                Event::Finished {
                    position: 2,
                    outcome: &Outcome::Downloaded,
                },
                json!({"kind": "downloaded", "position": 2}),
            ),
            (
                Event::Finished {
                    position: 2,
                    outcome: &Outcome::AlreadyPresent,
                },
                json!({"kind": "alreadyPresent", "position": 2}),
            ),
            (
                Event::Finished {
                    position: 2,
                    outcome: &Outcome::Stopped,
                },
                json!({"kind": "stopped", "position": 2}),
            ),
            (
                Event::Finished {
                    position: 2,
                    outcome: &failed,
                },
                json!({"kind": "failed", "position": 2, "error": "boom"}),
            ),
        ];
        for (event, expected) in cases {
            assert_eq!(to_value(Progress::from(event)).unwrap(), expected);
        }
    }
}
