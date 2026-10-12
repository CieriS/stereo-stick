//! What is on disk for a playlist. Every file of a track begins with the
//! position of the track (`07_`), which is how tracks are recognised.

use std::fs;
use std::io;
use std::path::Path;

use crate::ytdlp::file_prefix;

fn is_mp3(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("mp3"))
}

/// True when `names` holds the finished MP3 of the track at `position`.
///
/// Tracks are recognised by position alone: a list that is reordered, or
/// that gets a track inserted in the middle, no longer matches the folder.
pub fn is_downloaded<S: AsRef<str>>(position: usize, names: &[S]) -> bool {
    let prefix = file_prefix(position);
    names
        .iter()
        .map(AsRef::as_ref)
        .any(|name| name.starts_with(&prefix) && is_mp3(name))
}

/// True for every file of the track at `position`, finished or not.
pub fn belongs_to(position: usize, name: &str) -> bool {
    name.starts_with(&file_prefix(position))
}

/// The folder name for a playlist: the characters some file systems (and USB
/// sticks are usually FAT) refuse are replaced, and the dots and spaces
/// Windows drops from the ends are trimmed. `None` when nothing is left.
pub fn folder_name(playlist: &str) -> Option<String> {
    let cleaned: String = playlist
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim_matches(|c: char| c == '.' || c.is_whitespace());
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// The names of the files in `directory`; a folder that does not exist yet
/// is an empty one.
pub fn list(directory: &Path) -> io::Result<Vec<String>> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry = entry?;
        if entry.file_type()?.is_file() {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    names.sort();
    Ok(names)
}

/// Deletes every file of the track at `position` from `directory`: after a
/// failure even an MP3 is not to be trusted, since yt-dlp writes it before
/// the steps that cut and tag it. Best effort: a file that cannot be deleted
/// is left there.
pub fn remove_track(directory: &Path, position: usize) {
    for name in list(directory).unwrap_or_default() {
        if belongs_to(position, &name) {
            let _ = fs::remove_file(directory.join(name));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_track_is_downloaded_when_its_mp3_is_there() {
        let names = [
            "01_First.mp3",
            "02_Second.webm",
            "03_Third.MP3",
            "10_Tenth.jpg",
        ];
        assert!(is_downloaded(1, &names));
        assert!(!is_downloaded(2, &names));
        assert!(is_downloaded(3, &names));
        assert!(!is_downloaded(10, &names));
        assert!(!is_downloaded(4, &names));
        assert!(!is_downloaded(1, &[] as &[&str]));
    }

    #[test]
    fn positions_are_not_confused_with_longer_ones() {
        let names = ["100_Hundredth.mp3", "11_Eleventh.mp3"];
        assert!(!is_downloaded(10, &names));
        assert!(!is_downloaded(1, &names));
        assert!(is_downloaded(100, &names));
    }

    #[test]
    fn a_track_owns_every_file_with_its_position() {
        assert!(belongs_to(2, "02_Song.jpg"));
        assert!(belongs_to(2, "02_Song.webm.part"));
        assert!(belongs_to(2, "02_Song.mp3"));
        assert!(!belongs_to(2, "03_Other.jpg"));
        assert!(!belongs_to(2, "102_Other.mp3"));
        assert!(!belongs_to(2, "cover.jpg"));
        assert!(!belongs_to(2, "lista_brani.txt"));
    }

    #[test]
    fn folder_names_are_safe_on_every_file_system() {
        assert_eq!(folder_name("sc_song_ita").as_deref(), Some("sc_song_ita"));
        assert_eq!(
            folder_name("  Rock & Roll  ").as_deref(),
            Some("Rock & Roll")
        );
        assert_eq!(folder_name("AC/DC: live?").as_deref(), Some("AC_DC_ live_"));
        assert_eq!(folder_name("a\\b\tc").as_deref(), Some("a_b_c"));
        assert_eq!(folder_name("..").as_deref(), None);
        assert_eq!(folder_name(" . ").as_deref(), None);
        assert_eq!(folder_name("").as_deref(), None);
        assert_eq!(folder_name("Città ♥").as_deref(), Some("Città ♥"));
    }

    #[test]
    fn listing_and_cleaning_a_real_folder() {
        let dir = std::env::temp_dir().join(format!("engine-files-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        assert!(list(&dir).unwrap().is_empty());

        fs::create_dir_all(dir.join("03_a_folder")).unwrap();
        for name in [
            "03_Song.jpg",
            "03_Song.webm.part",
            "03_Song.mp3",
            "04_Other.jpg",
            "notes.txt",
        ] {
            fs::write(dir.join(name), b"").unwrap();
        }
        remove_track(&dir, 3);
        assert_eq!(list(&dir).unwrap(), ["04_Other.jpg", "notes.txt"]);
        assert!(dir.join("03_a_folder").is_dir());

        remove_track(&dir.join("missing"), 1);
        fs::remove_dir_all(&dir).unwrap();
    }
}
