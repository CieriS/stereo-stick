// A pretend backend for previewing the window in a plain browser. A song
// with "live" in its name has non-music parts; one with "missing" fails.

import type { Backend } from "./backend";
import type { Track } from "./state";

const pause = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** A rough copy of the real parser, enough to preview the window. */
function parse(text: string): Track[] {
  return text
    .split("\n")
    .map((line) => line.trim())
    .filter((line) => line !== "" && !line.startsWith("#"))
    .map((name, index) => ({ position: index + 1, name }));
}

const has = (track: Track, word: string) => track.name.toLowerCase().includes(word);

let stopRequested = false;
let folderRequests = 0;

export const demoBackend: Backend = {
  demo: true,
  toolAvailable: async () => true,
  parseList: async (text) => parse(text),
  async checkList(text, onResult) {
    for (const track of parse(text)) {
      await pause(350);
      if (has(track, "missing")) {
        onResult({ kind: "unavailable", position: track.position, error: "Video unavailable" });
      } else {
        const live = has(track, "live");
        onResult({
          kind: "found",
          position: track.position,
          title: `${track.name} (Official Video)`,
          nonMusicSeconds: live ? 74 : 0,
          nonMusicParts: live ? 2 : 0,
        });
      }
    }
  },
  async downloadList(request, onProgress) {
    stopRequested = false;
    for (const track of parse(request.text)) {
      const { position } = track;
      if (!stopRequested) onProgress({ kind: "started", position });
      for (const fraction of [0.2, 0.45, 0.7, 0.95]) {
        if (stopRequested) break;
        await pause(180);
        onProgress({ kind: "progress", position, fraction });
      }
      if (!stopRequested) await pause(180);
      if (stopRequested) {
        onProgress({ kind: "stopped", position });
      } else if (has(track, "missing")) {
        onProgress({ kind: "failed", position, error: "Video unavailable" });
      } else {
        onProgress({ kind: "downloaded", position });
      }
    }
  },
  async copyToStick(_musicFolder, stickFolder, onProgress) {
    stopRequested = false;
    if (stickFolder.startsWith("/Music/Car")) throw "stick-overlap";
    const songs = [
      "Rock/01_First.mp3",
      "Rock/02_Second.mp3",
      "Pop/01_Third.mp3",
      "Pop/02_Fourth.mp3",
    ];
    onProgress({ kind: "planned", toCopy: songs.length, upToDate: 6 });
    let done = 0;
    for (const path of songs) {
      if (stopRequested) break;
      await pause(400);
      done += 1;
      if (path.includes("Third"))
        onProgress({ kind: "failed", path, error: "No space left on device" });
      else onProgress({ kind: "copied", path });
    }
    onProgress({ kind: "finished", stopped: songs.length - done });
  },
  stopDownload: async () => {
    stopRequested = true;
  },
  // The first folder asked for is the music folder, the next ones the stick.
  chooseFolder: async () => (folderRequests++ === 0 ? "/Music/Car" : "/Volumes/STICK"),
};
