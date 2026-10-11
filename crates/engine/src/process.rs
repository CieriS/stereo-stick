//! The [`Downloader`] that runs the real yt-dlp program.

use std::ffi::OsString;
use std::path::PathBuf;
use std::process::{Command, Output, Stdio};

use crate::playlist::{DownloadError, Downloader};
use crate::probe::{VideoInfo, parse_probe_json};
use crate::tracklist::Source;
use crate::ytdlp::{Job, download_args, failure_message, probe_args};

/// yt-dlp as an external program. ffmpeg, which yt-dlp needs to convert the
/// audio, must be findable by it.
#[derive(Debug, Clone)]
pub struct YtDlp {
    program: PathBuf,
}

impl YtDlp {
    /// `program` is a path, or a bare name to look up in `PATH`.
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
        }
    }

    fn run(&self, args: Vec<OsString>) -> Result<Output, DownloadError> {
        let output = Command::new(&self.program)
            .args(args)
            .stdin(Stdio::null())
            .output()
            .map_err(|error| {
                DownloadError::Unavailable(format!("{}: {error}", self.program.display()))
            })?;
        if output.status.success() {
            Ok(output)
        } else {
            Err(DownloadError::Failed(failure_message(
                &String::from_utf8_lossy(&output.stderr),
            )))
        }
    }
}

impl Downloader for YtDlp {
    fn probe(&self, source: &Source) -> Result<VideoInfo, DownloadError> {
        let output = self.run(probe_args(source))?;
        Ok(parse_probe_json(&String::from_utf8_lossy(&output.stdout))?)
    }

    fn download(&self, job: &Job<'_>) -> Result<(), DownloadError> {
        self.run(download_args(job)).map(drop)
    }
}
