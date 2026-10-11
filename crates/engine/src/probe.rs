//! What is known about a video before downloading it, read from the JSON
//! yt-dlp prints for it. The parts of a music video that are not music
//! (spoken intro, applause, a story scene in the middle) come from
//! SponsorBlock's crowd-sourced `music_offtopic` category, which yt-dlp
//! reports as chapters.

use std::fmt;

use serde::Deserialize;

const NON_MUSIC_CATEGORY: &str = "music_offtopic";

/// A stretch of a video, in seconds from its start.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct VideoInfo {
    pub id: String,
    pub title: String,
    pub url: Option<String>,
    pub uploader: Option<String>,
    /// Length in seconds; live streams and some sites do not report it.
    pub duration: Option<f64>,
    /// Parts that are not music: sorted, not overlapping. Empty also when
    /// nobody has reviewed the video yet, so empty does not mean "clean".
    pub non_music: Vec<Segment>,
}

impl VideoInfo {
    pub fn has_non_music(&self) -> bool {
        !self.non_music.is_empty()
    }

    /// Seconds of the video that are not music.
    pub fn non_music_seconds(&self) -> f64 {
        self.non_music.iter().map(|s| s.end - s.start).sum()
    }

    /// Length of the music alone, when the length of the video is known.
    pub fn music_seconds(&self) -> Option<f64> {
        self.duration
            .map(|duration| (duration - self.non_music_seconds()).max(0.0))
    }
}

#[derive(Debug)]
pub struct ProbeError(serde_json::Error);

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "unreadable video description: {}", self.0)
    }
}

impl std::error::Error for ProbeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

#[derive(Deserialize)]
struct RawInfo {
    id: String,
    title: String,
    webpage_url: Option<String>,
    uploader: Option<String>,
    duration: Option<f64>,
    sponsorblock_chapters: Option<Vec<RawChapter>>,
}

#[derive(Deserialize)]
struct RawChapter {
    start_time: f64,
    end_time: f64,
    category: Option<String>,
}

/// Parses the JSON printed by `yt-dlp --dump-json --sponsorblock-mark music_offtopic`.
pub fn parse_probe_json(json: &str) -> Result<VideoInfo, ProbeError> {
    let raw: RawInfo = serde_json::from_str(json).map_err(ProbeError)?;
    let segments = raw
        .sponsorblock_chapters
        .unwrap_or_default()
        .into_iter()
        .filter(|chapter| chapter.category.as_deref() == Some(NON_MUSIC_CATEGORY))
        .map(|chapter| Segment {
            start: chapter.start_time,
            end: chapter.end_time,
        })
        .collect();
    Ok(VideoInfo {
        id: raw.id,
        title: raw.title,
        url: raw.webpage_url,
        uploader: raw.uploader,
        duration: raw.duration,
        non_music: merge(segments),
    })
}

/// Sorts the segments, drops the empty or malformed ones and joins those
/// that overlap or touch, so that their lengths can be summed.
fn merge(mut segments: Vec<Segment>) -> Vec<Segment> {
    segments.retain(|s| s.start.is_finite() && s.end.is_finite() && s.end > s.start);
    segments.sort_by(|a, b| a.start.total_cmp(&b.start));
    let mut merged: Vec<Segment> = Vec::with_capacity(segments.len());
    for segment in segments {
        match merged.last_mut() {
            Some(last) if segment.start <= last.end => last.end = last.end.max(segment.end),
            _ => merged.push(segment),
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seg(start: f64, end: f64) -> Segment {
        Segment { start, end }
    }

    #[test]
    fn reads_a_video_with_non_music_parts() {
        let json = r#"{
            "id": "abc123",
            "title": "Some Band - Some Song (Official Video)",
            "webpage_url": "https://example.com/watch?v=abc123",
            "uploader": "Some Band",
            "duration": 300,
            "unknown_field": [1, 2],
            "sponsorblock_chapters": [
                {"start_time": 0, "end_time": 20.5, "category": "music_offtopic", "title": "Non-Music Section"},
                {"start_time": 100, "end_time": 130, "category": "sponsor"},
                {"start_time": 280, "end_time": 300, "category": "music_offtopic"}
            ]
        }"#;
        let info = parse_probe_json(json).unwrap();
        assert_eq!(info.id, "abc123");
        assert_eq!(info.uploader.as_deref(), Some("Some Band"));
        assert_eq!(
            info.url.as_deref(),
            Some("https://example.com/watch?v=abc123")
        );
        assert_eq!(info.non_music, vec![seg(0.0, 20.5), seg(280.0, 300.0)]);
        assert!(info.has_non_music());
        assert_eq!(info.non_music_seconds(), 40.5);
        assert_eq!(info.music_seconds(), Some(259.5));
    }

    #[test]
    fn a_video_nobody_reviewed_has_no_segments() {
        for json in [
            r#"{"id": "a", "title": "t", "duration": 10.0}"#,
            r#"{"id": "a", "title": "t", "duration": 10.0, "sponsorblock_chapters": null}"#,
            r#"{"id": "a", "title": "t", "duration": 10.0, "sponsorblock_chapters": []}"#,
        ] {
            let info = parse_probe_json(json).unwrap();
            assert!(!info.has_non_music());
            assert_eq!(info.music_seconds(), Some(10.0));
        }
    }

    #[test]
    fn missing_duration_and_uploader_are_accepted() {
        let info = parse_probe_json(r#"{"id": "a", "title": "t", "duration": null}"#).unwrap();
        assert_eq!(info.duration, None);
        assert_eq!(info.uploader, None);
        assert_eq!(info.music_seconds(), None);
    }

    #[test]
    fn overlapping_and_unsorted_segments_are_joined() {
        let merged = merge(vec![
            seg(50.0, 60.0),
            seg(0.0, 10.0),
            seg(5.0, 12.0),
            seg(12.0, 15.0),
            seg(52.0, 55.0),
        ]);
        assert_eq!(merged, vec![seg(0.0, 15.0), seg(50.0, 60.0)]);
    }

    #[test]
    fn empty_reversed_and_non_finite_segments_are_dropped() {
        let merged = merge(vec![
            seg(3.0, 3.0),
            seg(9.0, 4.0),
            seg(f64::NAN, 5.0),
            seg(1.0, f64::INFINITY),
        ]);
        assert!(merged.is_empty());
    }

    #[test]
    fn music_length_never_goes_below_zero() {
        let json = r#"{"id": "a", "title": "t", "duration": 10,
            "sponsorblock_chapters": [{"start_time": 0, "end_time": 25, "category": "music_offtopic"}]}"#;
        assert_eq!(parse_probe_json(json).unwrap().music_seconds(), Some(0.0));
    }

    #[test]
    fn invalid_input_is_an_error() {
        for json in ["", "not json", "[]", r#"{"title": "no id"}"#] {
            let error = parse_probe_json(json).unwrap_err();
            assert!(
                error
                    .to_string()
                    .starts_with("unreadable video description")
            );
        }
    }
}
