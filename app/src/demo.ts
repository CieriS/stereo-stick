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
    for (const { position, ...track } of parse(request.text)) {
      onProgress({ kind: "started", position });
      for (const fraction of [0.2, 0.45, 0.7, 0.95]) {
        await pause(180);
        onProgress({ kind: "progress", position, fraction });
      }
      await pause(180);
      if (has({ position, ...track }, "missing")) {
        onProgress({ kind: "failed", position, error: "Video unavailable" });
      } else {
        onProgress({ kind: "downloaded", position });
      }
    }
  },
  chooseFolder: async () => "/Music/Car",
};
