//! Copying the music folder to a USB stick: only what is new or changed,
//! and only music. Nothing on the stick is ever deleted.
//!
//! Deciding what to copy is pure ([`plan`]); reading folders and copying
//! files are the thin parts around it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

/// FAT, the usual file system of a USB stick, keeps times to two seconds.
const TIME_TOLERANCE: Duration = Duration::from_secs(2);

/// A track on disk: `playlist/file.mp3`, always with a forward slash.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    pub path: String,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

fn is_hidden(name: &str) -> bool {
    name.starts_with('.')
}

fn is_music(name: &str) -> bool {
    !is_hidden(name)
        && Path::new(name)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("mp3"))
}

/// True when the copy on the stick can be kept: same size and not older
/// than the track in the music folder.
fn is_up_to_date(source: &Track, on_stick: &Track) -> bool {
    let older = match (source.modified, on_stick.modified) {
        (Some(source), Some(on_stick)) => on_stick + TIME_TOLERANCE < source,
        _ => false,
    };
    source.size == on_stick.size && !older
}

/// The tracks of `source` that are missing from the stick or differ from
/// their copy there, in the order of `source`.
pub fn plan(source: &[Track], on_stick: &[Track]) -> Vec<Track> {
    source
        .iter()
        .filter(|track| {
            !on_stick
                .iter()
                .any(|copy| copy.path == track.path && is_up_to_date(track, copy))
        })
        .cloned()
        .collect()
}

/// True when one folder is the other or lies inside it: copying between
/// them would be a mistake.
pub fn overlap(a: &Path, b: &Path) -> bool {
    a.starts_with(b) || b.starts_with(a)
}

/// The tracks under `root`: MP3 files inside its playlist folders, sorted.
/// Hidden files and folders, which some car stereos stumble on, are left
/// out, and so is anything that is not music. A missing `root` has no tracks.
pub fn scan(root: &Path) -> io::Result<Vec<Track>> {
    let mut tracks = Vec::new();
    for playlist in read_dir(root)? {
        let playlist = playlist?;
        let folder = playlist.file_name().to_string_lossy().into_owned();
        if is_hidden(&folder) || !playlist.file_type()?.is_dir() {
            continue;
        }
        for file in fs::read_dir(playlist.path())? {
            let file = file?;
            let name = file.file_name().to_string_lossy().into_owned();
            let metadata = file.metadata()?;
            if is_music(&name) && metadata.is_file() {
                tracks.push(Track {
                    path: format!("{folder}/{name}"),
                    size: metadata.len(),
                    modified: metadata.modified().ok(),
                });
            }
        }
    }
    tracks.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(tracks)
}

fn read_dir(root: &Path) -> io::Result<Box<dyn Iterator<Item = io::Result<fs::DirEntry>>>> {
    match fs::read_dir(root) {
        Ok(entries) => Ok(Box::new(entries)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Box::new(std::iter::empty())),
        Err(error) => Err(error),
    }
}

fn locate(root: &Path, track: &Track) -> PathBuf {
    track
        .path
        .split('/')
        .fold(root.to_owned(), |path, part| path.join(part))
}

/// Copies one track through a temporary name, so that a copy interrupted
/// halfway never passes for a finished file.
fn copy_track(source: &Path, destination: &Path, track: &Track) -> io::Result<()> {
    let target = locate(destination, track);
    if let Some(folder) = target.parent() {
        fs::create_dir_all(folder)?;
    }
    let mut partial = target.clone().into_os_string();
    partial.push(".part");
    let partial = PathBuf::from(partial);
    let copied =
        fs::copy(locate(source, track), &partial).and_then(|_| fs::rename(&partial, &target));
    if copied.is_err() {
        let _ = fs::remove_file(&partial);
    }
    copied
}

/// What happens while tracks are copied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Copy<'a> {
    Copied { track: &'a Track },
    Failed { track: &'a Track, error: String },
}

/// How a copy ended.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Outcome {
    pub copied: usize,
    /// Tracks that could not be copied, with the reason.
    pub failed: Vec<(String, String)>,
    /// Tracks not reached because the user stopped.
    pub stopped: usize,
}

/// Copies `tracks` from the music folder to the stick, telling `on_copy`
/// about each one. A track that fails does not stop the others; raising
/// `stop` ends the copy after the track in flight.
pub fn copy(
    source: &Path,
    destination: &Path,
    tracks: &[Track],
    on_copy: &mut dyn FnMut(Copy<'_>),
    stop: &AtomicBool,
) -> Outcome {
    let mut outcome = Outcome::default();
    for (index, track) in tracks.iter().enumerate() {
        if stop.load(Ordering::Relaxed) {
            outcome.stopped = tracks.len() - index;
            break;
        }
        match copy_track(source, destination, track) {
            Ok(()) => {
                outcome.copied += 1;
                on_copy(Copy::Copied { track });
            }
            Err(error) => {
                let error = error.to_string();
                outcome.failed.push((track.path.clone(), error.clone()));
                on_copy(Copy::Failed { track, error });
            }
        }
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOON: Duration = Duration::from_secs(1_000_000);

    fn track(path: &str, size: u64, seconds_after_noon: i64) -> Track {
        let modified = if seconds_after_noon >= 0 {
            SystemTime::UNIX_EPOCH + NOON + Duration::from_secs(seconds_after_noon.unsigned_abs())
        } else {
            SystemTime::UNIX_EPOCH + NOON - Duration::from_secs(seconds_after_noon.unsigned_abs())
        };
        Track {
            path: path.to_owned(),
            size,
            modified: Some(modified),
        }
    }

    fn paths(tracks: &[Track]) -> Vec<&str> {
        tracks.iter().map(|t| t.path.as_str()).collect()
    }

    #[test]
    fn an_empty_stick_gets_everything() {
        let source = [track("a/01.mp3", 10, 0), track("b/01.mp3", 20, 0)];
        assert_eq!(plan(&source, &[]), source);
        assert!(plan(&[], &source).is_empty());
    }

    #[test]
    fn tracks_already_on_the_stick_are_not_copied_again() {
        let source = [track("a/01.mp3", 10, 0), track("a/02.mp3", 20, 0)];
        // Copied later than the original was written: that is up to date.
        let on_stick = [track("a/01.mp3", 10, 500)];
        assert_eq!(paths(&plan(&source, &on_stick)), ["a/02.mp3"]);
    }

    #[test]
    fn a_changed_track_is_copied_again() {
        let on_stick = [track("a/01.mp3", 10, 0)];
        let bigger = [track("a/01.mp3", 11, 0)];
        let newer = [track("a/01.mp3", 10, 60)];
        assert_eq!(plan(&bigger, &on_stick).len(), 1);
        assert_eq!(plan(&newer, &on_stick).len(), 1);
    }

    #[test]
    fn the_two_second_resolution_of_a_stick_is_not_a_change() {
        let source = [track("a/01.mp3", 10, 2)];
        let on_stick = [track("a/01.mp3", 10, 0)];
        assert!(plan(&source, &on_stick).is_empty());
        let clearly_newer = [track("a/01.mp3", 10, 3)];
        assert_eq!(plan(&clearly_newer, &on_stick).len(), 1);
    }

    #[test]
    fn unknown_times_fall_back_to_the_size() {
        let mut source = track("a/01.mp3", 10, 100);
        source.modified = None;
        assert!(plan(std::slice::from_ref(&source), &[track("a/01.mp3", 10, 0)]).is_empty());
        assert_eq!(plan(&[source], &[track("a/01.mp3", 9, 0)]).len(), 1);
    }

    #[test]
    fn the_same_name_in_another_playlist_is_another_track() {
        let source = [track("b/01.mp3", 10, 0)];
        let on_stick = [track("a/01.mp3", 10, 0)];
        assert_eq!(paths(&plan(&source, &on_stick)), ["b/01.mp3"]);
    }

    #[test]
    fn what_is_only_on_the_stick_is_left_alone() {
        let on_stick = [track("old/01.mp3", 10, 0)];
        assert!(plan(&[], &on_stick).is_empty());
    }

    #[test]
    fn folders_that_contain_each_other_overlap() {
        let music = Path::new("/home/me/music");
        assert!(overlap(music, music));
        assert!(overlap(music, Path::new("/home/me/music/stick")));
        assert!(overlap(Path::new("/home/me"), music));
        assert!(!overlap(music, Path::new("/home/me/music2")));
        assert!(!overlap(music, Path::new("/media/stick")));
    }

    struct Sandbox(PathBuf);

    impl Sandbox {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("engine-stick-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }

        fn write(&self, path: &str, content: &str) {
            let path = self.0.join(path);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, content).unwrap();
        }
    }

    impl Drop for Sandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn sync(source: &Path, destination: &Path, stop: &AtomicBool) -> (Outcome, Vec<String>) {
        let to_copy = plan(&scan(source).unwrap(), &scan(destination).unwrap());
        let mut story = Vec::new();
        let outcome = copy(
            source,
            destination,
            &to_copy,
            &mut |event| {
                story.push(match event {
                    Copy::Copied { track } => format!("copied {}", track.path),
                    Copy::Failed { track, .. } => format!("failed {}", track.path),
                });
            },
            stop,
        );
        (outcome, story)
    }

    #[test]
    fn scanning_finds_only_music_inside_playlist_folders() {
        let sandbox = Sandbox::new("scan");
        for path in [
            "music/rock/01_A.mp3",
            "music/rock/02_B.MP3",
            "music/rock/lista_brani.txt",
            "music/rock/.DS_Store",
            "music/rock/._01_A.mp3",
            "music/rock/03_C.mp3.part",
            "music/rock/deeper/04_D.mp3",
            "music/.hidden/01_X.mp3",
            "music/loose.mp3",
        ] {
            sandbox.write(path, "x");
        }
        let tracks = scan(&sandbox.0.join("music")).unwrap();
        assert_eq!(paths(&tracks), ["rock/01_A.mp3", "rock/02_B.MP3"]);
        assert_eq!(tracks[0].size, 1);
        assert!(scan(&sandbox.0.join("missing")).unwrap().is_empty());
    }

    #[test]
    fn a_second_copy_moves_nothing_and_a_new_track_moves_alone() {
        let sandbox = Sandbox::new("copy");
        let (music, stick) = (sandbox.0.join("music"), sandbox.0.join("stick"));
        sandbox.write("music/rock/01_A.mp3", "aaa");
        sandbox.write("music/pop/01_B.mp3", "bb");
        sandbox.write("music/pop/notes.txt", "not music");
        sandbox.write("stick/old/01_Kept.mp3", "kept");
        let never = AtomicBool::new(false);

        let (outcome, story) = sync(&music, &stick, &never);
        assert_eq!(outcome.copied, 2);
        assert_eq!(story, ["copied pop/01_B.mp3", "copied rock/01_A.mp3"]);
        assert_eq!(
            fs::read_to_string(stick.join("rock/01_A.mp3")).unwrap(),
            "aaa"
        );
        assert!(!stick.join("pop/notes.txt").exists());
        assert!(stick.join("old/01_Kept.mp3").exists());
        assert!(!stick.join("rock/01_A.mp3.part").exists());

        assert_eq!(sync(&music, &stick, &never).0, Outcome::default());

        sandbox.write("music/rock/02_New.mp3", "new");
        sandbox.write("music/rock/01_A.mp3", "longer now");
        let (outcome, story) = sync(&music, &stick, &never);
        assert_eq!(outcome.copied, 2);
        assert_eq!(story, ["copied rock/01_A.mp3", "copied rock/02_New.mp3"]);
        assert_eq!(
            fs::read_to_string(stick.join("rock/01_A.mp3")).unwrap(),
            "longer now"
        );
    }

    #[test]
    fn a_track_that_cannot_be_copied_does_not_stop_the_others() {
        let sandbox = Sandbox::new("failure");
        let (music, stick) = (sandbox.0.join("music"), sandbox.0.join("stick"));
        sandbox.write("music/rock/02_B.mp3", "b");
        let gone = track("rock/01_Gone.mp3", 1, 0);
        let there = scan(&music).unwrap().remove(0);
        let outcome = copy(
            &music,
            &stick,
            &[gone, there],
            &mut |_| {},
            &AtomicBool::new(false),
        );
        assert_eq!(outcome.copied, 1);
        assert_eq!(outcome.failed.len(), 1);
        assert_eq!(outcome.failed[0].0, "rock/01_Gone.mp3");
        assert!(!stick.join("rock/01_Gone.mp3.part").exists());
    }

    #[test]
    fn stopping_leaves_the_remaining_tracks_for_next_time() {
        let sandbox = Sandbox::new("stop");
        let (music, stick) = (sandbox.0.join("music"), sandbox.0.join("stick"));
        for name in ["01", "02", "03"] {
            sandbox.write(&format!("music/rock/{name}.mp3"), name);
        }
        let stop = AtomicBool::new(false);
        let tracks = scan(&music).unwrap();
        let outcome = copy(
            &music,
            &stick,
            &tracks,
            &mut |_| stop.store(true, Ordering::Relaxed),
            &stop,
        );
        assert_eq!((outcome.copied, outcome.stopped), (1, 2));
        assert_eq!(paths(&scan(&stick).unwrap()), ["rock/01.mp3"]);

        let (outcome, _) = sync(&music, &stick, &AtomicBool::new(false));
        assert_eq!(outcome.copied, 2);
    }
}
