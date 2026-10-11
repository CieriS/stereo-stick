//! Parsing of a playlist's track list: one track per line.
//!
//! A line is a link, a `ytsearchN:"query"` line (the format of the original
//! scripts) or plain text, which is searched as typed. Blank lines are
//! skipped; a `#` comment names the track on the line right below it.

/// Where the audio of a track comes from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// A direct link to a video.
    Url(String),
    /// A text search; the first result is taken.
    Search(String),
}

impl Source {
    /// The argument yt-dlp expects for this source.
    pub fn to_ytdlp_arg(&self) -> String {
        match self {
            Self::Url(url) => url.clone(),
            Self::Search(query) => format!("ytsearch1:{query}"),
        }
    }
}

/// One track of a playlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// 1-based position among the tracks of the list; it prefixes the file
    /// name, so it stays the same however many times the list is downloaded.
    pub position: usize,
    /// Name given by the comment above the line, if any.
    pub label: Option<String>,
    pub source: Source,
}

impl Entry {
    /// The name to show for this track.
    pub fn display_name(&self) -> &str {
        match (&self.label, &self.source) {
            (Some(label), _) => label,
            (None, Source::Url(text) | Source::Search(text)) => text,
        }
    }
}

/// Parses the text of a track list. It never fails: every line that is not
/// blank or a comment is a track.
pub fn parse(text: &str) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut label = None;
    for line in text.lines() {
        let line = line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() {
            label = None;
        } else if let Some(comment) = line.strip_prefix('#') {
            let comment = comment.trim();
            label = (!comment.is_empty()).then(|| comment.to_owned());
        } else if let Some(source) = parse_source(line) {
            entries.push(Entry {
                position: entries.len() + 1,
                label: label.take(),
                source,
            });
        }
    }
    entries
}

fn parse_source(line: &str) -> Option<Source> {
    if is_url(line) {
        return Some(Source::Url(line.to_owned()));
    }
    let query = strip_search_prefix(line).unwrap_or(line);
    let query = query.trim().trim_matches('"').trim();
    (!query.is_empty()).then(|| Source::Search(query.to_owned()))
}

fn is_url(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

/// Strips `ytsearch:`, `ytsearch1:`, `ytsearch5:`… The number of results is
/// ignored: a track is always one result.
fn strip_search_prefix(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("ytsearch")?;
    rest.trim_start_matches(|c: char| c.is_ascii_digit())
        .strip_prefix(':')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn search(position: usize, label: Option<&str>, query: &str) -> Entry {
        Entry {
            position,
            label: label.map(str::to_owned),
            source: Source::Search(query.to_owned()),
        }
    }

    #[test]
    fn parses_the_format_of_the_original_scripts() {
        let text = "# Artist One - First Song\nytsearch1:\"Artist One First Song\"\n\n# Artist Two - Second Song\nytsearch1:\"Artist Two Second Song\"\n";
        assert_eq!(
            parse(text),
            vec![
                search(1, Some("Artist One - First Song"), "Artist One First Song"),
                search(
                    2,
                    Some("Artist Two - Second Song"),
                    "Artist Two Second Song"
                ),
            ]
        );
    }

    #[test]
    fn plain_text_is_a_search() {
        assert_eq!(
            parse("Artist - Song"),
            vec![search(1, None, "Artist - Song")]
        );
    }

    #[test]
    fn links_are_kept_as_they_are() {
        let entries = parse("  https://example.com/watch?v=abc#t=1  \nHTTP://example.com/x");
        assert_eq!(
            entries[0].source,
            Source::Url("https://example.com/watch?v=abc#t=1".into())
        );
        assert_eq!(
            entries[1].source,
            Source::Url("HTTP://example.com/x".into())
        );
    }

    #[test]
    fn search_prefix_variants() {
        for line in ["ytsearch:a b", "ytsearch1:a b", "ytsearch12: \"a b\" "] {
            assert_eq!(parse(line), vec![search(1, None, "a b")], "{line}");
        }
    }

    #[test]
    fn a_comment_names_only_the_line_right_below() {
        let entries = parse("# Playlist header\n\n# Named\nfirst\nsecond");
        assert_eq!(entries[0].label.as_deref(), Some("Named"));
        assert_eq!(entries[1].label, None);
    }

    #[test]
    fn the_last_of_several_comments_wins() {
        let entries = parse("# old\n# new\nsong");
        assert_eq!(entries[0].label.as_deref(), Some("new"));
    }

    #[test]
    fn empty_input_and_lines_without_a_track_give_nothing() {
        assert!(parse("").is_empty());
        assert!(parse("\n  \n#\n# only a comment\nytsearch1:\"\"\n\"\"").is_empty());
    }

    #[test]
    fn positions_count_tracks_not_lines() {
        let entries = parse("# c\n\na\n\n\nb\r\nc");
        let positions: Vec<_> = entries.iter().map(|e| e.position).collect();
        assert_eq!(positions, [1, 2, 3]);
        assert_eq!(entries[1].source, Source::Search("b".into()));
    }

    #[test]
    fn a_byte_order_mark_is_ignored() {
        assert_eq!(parse("\u{feff}song"), vec![search(1, None, "song")]);
    }

    #[test]
    fn display_name_prefers_the_label() {
        assert_eq!(search(1, Some("Label"), "query").display_name(), "Label");
        assert_eq!(search(1, None, "query").display_name(), "query");
    }

    #[test]
    fn ytdlp_argument() {
        assert_eq!(Source::Search("a b".into()).to_ytdlp_arg(), "ytsearch1:a b");
        assert_eq!(
            Source::Url("https://x.y".into()).to_ytdlp_arg(),
            "https://x.y"
        );
    }
}
