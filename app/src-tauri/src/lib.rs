//! The desktop window: thin commands over the engine. Everything that
//! decides something lives in the engine; here types are converted and long
//! work is moved off the window's thread.

mod dto;

use std::path::PathBuf;

use engine::playlist::{self, NonMusic, Request};
use engine::process::YtDlp;
use engine::{files, tracklist};
use serde::Deserialize;
use tauri::ipc::Channel;

use dto::{CheckResult, Progress, Track};

/// Until the installers ship their own copy, yt-dlp is the one in `PATH`.
fn downloader() -> YtDlp {
    YtDlp::new("yt-dlp")
}

/// Runs work that waits on yt-dlp without freezing the window.
async fn off_the_window_thread<T: Send + 'static>(
    work: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| error.to_string())?
}

/// The tracks of a list, as the window shows them while the user types.
#[tauri::command]
fn parse_list(text: &str) -> Vec<Track> {
    tracklist::parse(text).iter().map(Track::from).collect()
}

/// Describes every track without downloading, one result at a time.
#[tauri::command]
async fn check_list(text: String, on_result: Channel<CheckResult>) -> Result<(), String> {
    off_the_window_thread(move || {
        let downloader = downloader();
        for entry in tracklist::parse(&text) {
            let checked = playlist::check(&downloader, std::slice::from_ref(&entry));
            for result in &checked {
                on_result
                    .send(CheckResult::from(result))
                    .map_err(|error| error.to_string())?;
            }
        }
        Ok(())
    })
    .await
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DownloadRequest {
    playlist: String,
    /// The folder that holds one folder per playlist.
    music_folder: PathBuf,
    text: String,
    remove_non_music: bool,
}

/// Downloads a playlist into its own folder inside the music folder.
#[tauri::command]
async fn download_list(
    request: DownloadRequest,
    on_progress: Channel<Progress>,
) -> Result<(), String> {
    off_the_window_thread(move || {
        let folder = files::folder_name(&request.playlist).ok_or("playlist-name")?;
        let directory = request.music_folder.join(folder);
        let existing = files::list(&directory).map_err(|error| error.to_string())?;
        let entries = tracklist::parse(&request.text);
        playlist::download(
            &downloader(),
            &Request {
                playlist: request.playlist.trim(),
                directory: &directory,
                entries: &entries,
                non_music: if request.remove_non_music {
                    NonMusic::Remove
                } else {
                    NonMusic::Keep
                },
                existing: &existing,
            },
            &mut |event| {
                // A window that stopped listening is no reason to stop downloading.
                let _ = on_progress.send(Progress::from(event));
            },
        );
        Ok(())
    })
    .await
}

/// True when the download tool can be started at all.
#[tauri::command]
async fn tool_available() -> bool {
    off_the_window_thread(|| Ok(downloader().is_available()))
        .await
        .unwrap_or(false)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            parse_list,
            check_list,
            download_list,
            tool_available
        ])
        .run(tauri::generate_context!())
        .expect("the window could not be opened");
}
