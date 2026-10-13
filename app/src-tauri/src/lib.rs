//! The desktop window: thin commands over the engine. Everything that
//! decides something lives in the engine; here types are converted and long
//! work is moved off the window's thread.

mod dto;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use engine::playlist::{self, NonMusic, Request};
use engine::process::YtDlp;
use engine::{files, stick, tracklist};
use serde::Deserialize;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use dto::{CheckResult, Progress, StickProgress, Track};

/// Raised by the window to stop the download in progress.
#[derive(Default)]
struct Stop(Arc<AtomicBool>);

/// The tools shipped with the app when they are there, otherwise the yt-dlp
/// found in `PATH` (development).
fn downloader(app: &AppHandle) -> YtDlp {
    app.path()
        .resource_dir()
        .ok()
        .and_then(|resources| YtDlp::bundled(&resources.join("tools")))
        .unwrap_or_else(|| YtDlp::new("yt-dlp"))
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
async fn check_list(
    app: AppHandle,
    text: String,
    on_result: Channel<CheckResult>,
) -> Result<(), String> {
    off_the_window_thread(move || {
        let downloader = downloader(&app);
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
    app: AppHandle,
    stop: State<'_, Stop>,
    request: DownloadRequest,
    on_progress: Channel<Progress>,
) -> Result<(), String> {
    let stop = Arc::clone(&stop.0);
    stop.store(false, Ordering::Relaxed);
    off_the_window_thread(move || {
        let folder = files::folder_name(&request.playlist).ok_or("playlist-name")?;
        let directory = request.music_folder.join(folder);
        let existing = files::list(&directory).map_err(|error| error.to_string())?;
        let entries = tracklist::parse(&request.text);
        playlist::download(
            &downloader(&app),
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
                stop: &stop,
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

/// Copies to the stick the tracks of the music folder that are missing or
/// changed there. Nothing on the stick is deleted.
#[tauri::command]
async fn copy_to_stick(
    stop: State<'_, Stop>,
    music_folder: PathBuf,
    stick_folder: PathBuf,
    on_progress: Channel<StickProgress>,
) -> Result<(), String> {
    let stop = Arc::clone(&stop.0);
    stop.store(false, Ordering::Relaxed);
    off_the_window_thread(move || {
        if stick::overlap(&music_folder, &stick_folder) {
            return Err("stick-overlap".to_owned());
        }
        let source = stick::scan(&music_folder).map_err(|error| error.to_string())?;
        let on_stick = stick::scan(&stick_folder).map_err(|error| error.to_string())?;
        let to_copy = stick::plan(&source, &on_stick);
        // A window that stopped listening is no reason to stop copying.
        let send = |event| drop(on_progress.send(event));
        send(StickProgress::Planned {
            to_copy: to_copy.len(),
            up_to_date: source.len() - to_copy.len(),
        });
        let outcome = stick::copy(
            &music_folder,
            &stick_folder,
            &to_copy,
            &mut |event| send(StickProgress::from(event)),
            &stop,
        );
        send(StickProgress::Finished {
            stopped: outcome.stopped,
        });
        Ok(())
    })
    .await
}

/// Stops the download or the copy in progress: the track in flight is abandoned and the
/// ones after it are not started.
#[tauri::command]
fn stop_download(stop: State<'_, Stop>) {
    stop.0.store(true, Ordering::Relaxed);
}

/// True when the download tool can be started at all.
#[tauri::command]
async fn tool_available(app: AppHandle) -> bool {
    off_the_window_thread(move || Ok(downloader(&app).is_available()))
        .await
        .unwrap_or(false)
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(Stop::default())
        .invoke_handler(tauri::generate_handler![
            parse_list,
            check_list,
            download_list,
            stop_download,
            copy_to_stick,
            tool_available
        ])
        .run(tauri::generate_context!())
        .expect("the window could not be opened");
}
