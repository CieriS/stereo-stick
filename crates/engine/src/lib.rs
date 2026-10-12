//! The engine behind the app: it turns a list of tracks into tagged MP3
//! files ready for a car stereo. It knows nothing about windows or buttons;
//! the outside world is reached only through [`playlist::Downloader`].

pub mod files;
pub mod playlist;
pub mod probe;
pub mod process;
pub mod tracklist;
pub mod ytdlp;
