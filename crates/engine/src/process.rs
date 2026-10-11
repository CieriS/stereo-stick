//! The [`Downloader`] that runs the real yt-dlp program.

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;

use crate::files;
use crate::playlist::{DownloadError, Downloader, Job};
use crate::probe::{VideoInfo, parse_probe_json};
use crate::tracklist::Source;
use crate::ytdlp::{download_args, failure_message, parse_progress, probe_args};

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

    /// True when the program can be started at all.
    pub fn is_available(&self) -> bool {
        self.run(vec!["--version".into()], &mut |_| {}).is_ok()
    }

    fn unavailable(&self, error: &std::io::Error) -> DownloadError {
        DownloadError::Unavailable(format!("{}: {error}", self.program.display()))
    }

    /// Runs yt-dlp to the end, handing each line of its standard output to
    /// `on_line` as soon as it is written.
    fn run(&self, args: Vec<OsString>, on_line: &mut dyn FnMut(&str)) -> Result<(), DownloadError> {
        let mut child = Command::new(&self.program)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|error| self.unavailable(&error))?;

        // Standard error is drained on its own thread: left unread, a
        // talkative yt-dlp would fill the pipe and stop forever.
        let mut stderr = child.stderr.take().expect("stderr is piped");
        let errors = thread::spawn(move || {
            let mut text = Vec::new();
            let _ = stderr.read_to_end(&mut text);
            String::from_utf8_lossy(&text).into_owned()
        });
        let stdout = child.stdout.take().expect("stdout is piped");
        for line in BufReader::new(stdout).split(b'\n').map_while(Result::ok) {
            on_line(&String::from_utf8_lossy(&line));
        }

        let status = child.wait().map_err(|error| self.unavailable(&error))?;
        let errors = errors.join().unwrap_or_default();
        if status.success() {
            Ok(())
        } else {
            Err(DownloadError::Failed(failure_message(&errors)))
        }
    }
}

impl Downloader for YtDlp {
    fn probe(&self, source: &Source) -> Result<VideoInfo, DownloadError> {
        let mut json = String::new();
        self.run(probe_args(source), &mut |line| json.push_str(line))?;
        Ok(parse_probe_json(&json)?)
    }

    fn download(
        &self,
        job: &Job<'_>,
        on_progress: &mut dyn FnMut(f32),
    ) -> Result<(), DownloadError> {
        let result = self.run(download_args(job), &mut |line| {
            if let Some(fraction) = parse_progress(line) {
                on_progress(fraction);
            }
        });
        if result.is_err() {
            files::remove_leftovers(job.directory, job.entry.position);
        }
        result
    }
}
