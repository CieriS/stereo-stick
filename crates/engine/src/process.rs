//! The [`Downloader`] that runs the real yt-dlp program.

use std::ffi::OsString;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::Duration;

use crate::files;
use crate::playlist::{DownloadError, Downloader, Job};
use crate::probe::{VideoInfo, parse_probe_json};
use crate::tracklist::Source;
use crate::ytdlp::{Tools, base_args, download_args, failure_message, parse_progress, probe_args};

/// yt-dlp as an external program. ffmpeg, which yt-dlp needs to convert the
/// audio, must be findable by it.
#[derive(Debug, Clone)]
pub struct YtDlp {
    program: PathBuf,
    tools: Tools,
}

/// How often a running yt-dlp is checked for a request to stop.
const STOP_CHECK: Duration = Duration::from_millis(100);

/// Prepares a command so that the whole family of processes it starts can
/// be ended together: the released yt-dlp is a launcher that starts the real
/// program, which in turn starts ffmpeg.
#[cfg(unix)]
fn prepare(command: &mut Command) {
    use std::os::unix::process::CommandExt;
    command.process_group(0);
}

/// On Windows a console program started by a window would flash a console.
#[cfg(windows)]
fn prepare(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NO_WINDOW);
}

/// Ends a process and everything it started.
fn end_family(child: &mut Child) {
    let pid = child.id().to_string();
    #[cfg(unix)]
    let mut killer = {
        let mut killer = Command::new("kill");
        killer.args(["-KILL", "--", &format!("-{pid}")]);
        killer
    };
    #[cfg(windows)]
    let mut killer = {
        let mut killer = Command::new("taskkill");
        killer.args(["/PID", &pid, "/T", "/F"]);
        prepare(&mut killer);
        killer
    };
    let _ = killer
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
    let _ = child.wait();
}

/// The file name of a program on this system.
fn executable(name: &str) -> String {
    format!("{name}{}", std::env::consts::EXE_SUFFIX)
}

impl YtDlp {
    /// `program` is a path, or a bare name to look up in `PATH`; ffmpeg and
    /// Deno are then expected in `PATH` too.
    pub fn new(program: impl Into<PathBuf>) -> Self {
        Self {
            program: program.into(),
            tools: Tools::default(),
        }
    }

    /// The yt-dlp shipped in `directory`, with the ffmpeg and Deno found next
    /// to it. `None` when yt-dlp itself is not there.
    pub fn bundled(directory: &Path) -> Option<Self> {
        let find = |name| Some(directory.join(executable(name))).filter(|path| path.is_file());
        Some(Self {
            program: find("yt-dlp")?,
            tools: Tools {
                ffmpeg: find("ffmpeg"),
                deno: find("deno"),
            },
        })
    }

    /// True when the tools come with the app rather than from `PATH`.
    pub fn is_bundled(&self) -> bool {
        self.tools != Tools::default()
    }

    /// True when the program can be started at all.
    pub fn is_available(&self) -> bool {
        self.run(
            vec!["--version".into()],
            &mut |_| {},
            &AtomicBool::new(false),
        )
        .is_ok()
    }

    fn unavailable(&self, error: &std::io::Error) -> DownloadError {
        DownloadError::Unavailable(format!("{}: {error}", self.program.display()))
    }

    /// Runs yt-dlp to the end, handing each line of its standard output to
    /// `on_line` as soon as it is written. When `stop` becomes true yt-dlp
    /// and everything it started are ended.
    fn run(
        &self,
        args: Vec<OsString>,
        on_line: &mut dyn FnMut(&str),
        stop: &AtomicBool,
    ) -> Result<(), DownloadError> {
        let mut command = Command::new(&self.program);
        command
            .args(base_args(&self.tools))
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        prepare(&mut command);
        let mut child = command.spawn().map_err(|error| self.unavailable(&error))?;

        // Standard error is drained on its own thread: left unread, a
        // talkative yt-dlp would fill the pipe and stop forever.
        let mut stderr = child.stderr.take().expect("stderr is piped");
        let errors = thread::spawn(move || {
            let mut text = Vec::new();
            let _ = stderr.read_to_end(&mut text);
            String::from_utf8_lossy(&text).into_owned()
        });
        // Standard output too, so that waiting for a line never keeps a
        // request to stop from being noticed.
        let stdout = child.stdout.take().expect("stdout is piped");
        let (sender, lines) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).split(b'\n').map_while(Result::ok) {
                if sender.send(line).is_err() {
                    break;
                }
            }
        });
        loop {
            if stop.load(Ordering::Relaxed) {
                end_family(&mut child);
                return Err(DownloadError::Stopped);
            }
            match lines.recv_timeout(STOP_CHECK) {
                Ok(line) => on_line(&String::from_utf8_lossy(&line)),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
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
        self.run(
            probe_args(source),
            &mut |line| json.push_str(line),
            &AtomicBool::new(false),
        )?;
        Ok(parse_probe_json(&json)?)
    }

    fn download(
        &self,
        job: &Job<'_>,
        on_progress: &mut dyn FnMut(f32),
        stop: &AtomicBool,
    ) -> Result<(), DownloadError> {
        let result = self.run(
            download_args(job),
            &mut |line| {
                if let Some(fraction) = parse_progress(line) {
                    on_progress(fraction);
                }
            },
            stop,
        );
        if result.is_err() {
            files::remove_track(job.directory, job.entry.position);
        }
        result
    }
}
