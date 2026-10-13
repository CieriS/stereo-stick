// The window's only way to the rest of the app. The desktop shell provides
// the real one; in a plain browser a stand-in takes its place for previews.

import { Channel, invoke, isTauri } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { demoBackend } from "./demo";
import type { CheckResult, Progress, Track } from "./state";
import type { StickEvent } from "./stick";

export interface DownloadRequest {
  playlist: string;
  musicFolder: string;
  text: string;
  removeNonMusic: boolean;
}

export interface Backend {
  /** True when nothing is really downloaded. */
  readonly demo: boolean;
  toolAvailable(): Promise<boolean>;
  parseList(text: string): Promise<Track[]>;
  checkList(text: string, onResult: (result: CheckResult) => void): Promise<void>;
  downloadList(request: DownloadRequest, onProgress: (event: Progress) => void): Promise<void>;
  /** Copies what is new or changed from the music folder to the stick. */
  copyToStick(
    musicFolder: string,
    stickFolder: string,
    onProgress: (event: StickEvent) => void,
  ): Promise<void>;
  /** Asks the download or the copy in progress to stop; it then ends early. */
  stopDownload(): Promise<void>;
  /** Asks the user for a folder; `null` when they change their mind. */
  chooseFolder(): Promise<string | null>;
}

const desktopBackend: Backend = {
  demo: false,
  toolAvailable: () => invoke("tool_available"),
  parseList: (text) => invoke("parse_list", { text }),
  checkList(text, onResult) {
    const channel = new Channel<CheckResult>();
    channel.onmessage = onResult;
    return invoke("check_list", { text, onResult: channel });
  },
  downloadList(request, onProgress) {
    const channel = new Channel<Progress>();
    channel.onmessage = onProgress;
    return invoke("download_list", { request, onProgress: channel });
  },
  copyToStick(musicFolder, stickFolder, onProgress) {
    const channel = new Channel<StickEvent>();
    channel.onmessage = onProgress;
    return invoke("copy_to_stick", { musicFolder, stickFolder, onProgress: channel });
  },
  stopDownload: () => invoke("stop_download"),
  chooseFolder: () => open({ directory: true, multiple: false }),
};

export const backend: Backend = isTauri() ? desktopBackend : demoBackend;
