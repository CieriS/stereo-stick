import { describe, expect, test } from "bun:test";
import {
  applyCheck,
  applyProgress,
  initialState,
  missingForDownload,
  overallFraction,
  type State,
  settle,
  startChecking,
  startDownloading,
  summarize,
  withTracks,
} from "./state";

const three = () =>
  withTracks([
    { position: 1, name: "one" },
    { position: 2, name: "two" },
    { position: 3, name: "three" },
  ]);

const statuses = (state: State) => state.tracks.map((track) => track.status);

describe("editing", () => {
  test("starts empty", () => {
    expect(initialState).toEqual({ phase: "editing", tracks: [] });
  });

  test("a new list forgets what was known", () => {
    const checked = applyCheck(startChecking(three()), {
      kind: "found",
      position: 1,
      title: "T",
      nonMusicSeconds: 3,
      nonMusicParts: 1,
    });
    const edited = withTracks(checked.tracks.map(({ position, name }) => ({ position, name })));
    expect(edited.phase).toBe("editing");
    expect(edited.tracks[0]).toEqual({ position: 1, name: "one", status: "idle" });
  });
});

describe("checking", () => {
  test("marks every track as being checked", () => {
    const state = startChecking(three());
    expect(state.phase).toBe("checking");
    expect(statuses(state)).toEqual(["checking", "checking", "checking"]);
  });

  test("a clean video is found, one with non-music parts is flagged", () => {
    let state = startChecking(three());
    state = applyCheck(state, {
      kind: "found",
      position: 1,
      title: "Clean",
      nonMusicSeconds: 0,
      nonMusicParts: 0,
    });
    state = applyCheck(state, {
      kind: "found",
      position: 2,
      title: "Clip",
      nonMusicSeconds: 42.5,
      nonMusicParts: 2,
    });
    state = applyCheck(state, { kind: "unavailable", position: 3, error: "gone" });
    expect(statuses(state)).toEqual(["found", "flagged", "unavailable"]);
    expect(state.tracks[1]).toMatchObject({ title: "Clip", nonMusicSeconds: 42.5 });
    expect(state.tracks[2]?.error).toBe("gone");
  });

  test("a result for a track that is not there changes nothing", () => {
    const state = startChecking(three());
    expect(applyCheck(state, { kind: "unavailable", position: 9, error: "x" })).toEqual(state);
  });

  test("when the check ends, tracks without an answer go back to idle", () => {
    let state = startChecking(three());
    state = applyCheck(state, { kind: "unavailable", position: 1, error: "gone" });
    state = settle(state);
    expect(state.phase).toBe("editing");
    expect(statuses(state)).toEqual(["unavailable", "idle", "idle"]);
  });

  test("when the check breaks, tracks without an answer carry the error", () => {
    const state = settle(startChecking(three()), "tool missing");
    expect(state.phase).toBe("editing");
    expect(statuses(state)).toEqual(["unavailable", "unavailable", "unavailable"]);
    expect(state.tracks[0]?.error).toBe("tool missing");
  });
});

describe("downloading", () => {
  test("keeps what the check found", () => {
    const checked = applyCheck(startChecking(three()), {
      kind: "found",
      position: 2,
      title: "Clip",
      nonMusicSeconds: 10,
      nonMusicParts: 1,
    });
    const state = startDownloading(settle(checked));
    expect(state.phase).toBe("downloading");
    expect(statuses(state)).toEqual(["waiting", "waiting", "waiting"]);
    expect(state.tracks[1]).toMatchObject({ title: "Clip", nonMusicParts: 1 });
  });

  test("follows each track to its outcome", () => {
    let state = startDownloading(three());
    state = applyProgress(state, { kind: "started", position: 1 });
    expect(state.tracks[0]).toMatchObject({ status: "downloading", fraction: 0 });
    state = applyProgress(state, { kind: "progress", position: 1, fraction: 0.4 });
    expect(state.tracks[0]?.fraction).toBe(0.4);
    state = applyProgress(state, { kind: "downloaded", position: 1 });
    state = applyProgress(state, { kind: "alreadyPresent", position: 2 });
    state = applyProgress(state, { kind: "failed", position: 3, error: "gone" });
    expect(statuses(state)).toEqual(["downloaded", "alreadyPresent", "failed"]);
    expect(state.tracks[0]?.fraction).toBe(1);
    expect(state.tracks[2]?.error).toBe("gone");
  });

  test("a stopped download keeps what was done and marks the rest as stopped", () => {
    let state = startDownloading(three());
    state = applyProgress(state, { kind: "downloaded", position: 1 });
    state = applyProgress(state, { kind: "progress", position: 2, fraction: 0.6 });
    state = applyProgress(state, { kind: "stopped", position: 2 });
    state = applyProgress(state, { kind: "stopped", position: 3 });
    state = settle(state);
    expect(state.phase).toBe("done");
    expect(statuses(state)).toEqual(["downloaded", "stopped", "stopped"]);
    expect(state.tracks[1]?.fraction).toBeUndefined();
    expect(summarize(state)).toMatchObject({ downloaded: 1, stopped: 2, failed: 0 });
    expect(overallFraction(state)).toBe(1);
  });

  test("when the download ends the phase is done", () => {
    let state = startDownloading(three());
    for (const position of [1, 2, 3]) {
      state = applyProgress(state, { kind: "downloaded", position });
    }
    state = settle(state);
    expect(state.phase).toBe("done");
    expect(statuses(state)).toEqual(["downloaded", "downloaded", "downloaded"]);
  });

  test("when the download breaks, unfinished tracks are failed, finished ones stay", () => {
    let state = startDownloading(three());
    state = applyProgress(state, { kind: "downloaded", position: 1 });
    state = applyProgress(state, { kind: "started", position: 2 });
    state = settle(state, "tool missing");
    expect(state.phase).toBe("done");
    expect(statuses(state)).toEqual(["downloaded", "failed", "failed"]);
    expect(state.tracks[2]?.error).toBe("tool missing");
  });
});

describe("summary", () => {
  test("counts outcomes and flagged tracks", () => {
    let state = applyCheck(startChecking(three()), {
      kind: "found",
      position: 1,
      title: "Clip",
      nonMusicSeconds: 10,
      nonMusicParts: 1,
    });
    state = startDownloading(settle(state));
    state = applyProgress(state, { kind: "downloaded", position: 1 });
    state = applyProgress(state, { kind: "alreadyPresent", position: 2 });
    state = applyProgress(state, { kind: "failed", position: 3, error: "gone" });
    expect(summarize(state)).toEqual({
      total: 3,
      downloaded: 1,
      alreadyPresent: 1,
      stopped: 0,
      failed: 1,
      flagged: 1,
    });
  });

  test("an empty list is all zeros", () => {
    expect(summarize(initialState)).toEqual({
      total: 0,
      downloaded: 0,
      alreadyPresent: 0,
      stopped: 0,
      failed: 0,
      flagged: 0,
    });
    expect(overallFraction(initialState)).toBe(0);
  });

  test("overall progress counts finished tracks and the one in flight", () => {
    let state = startDownloading(three());
    expect(overallFraction(state)).toBe(0);
    state = applyProgress(state, { kind: "failed", position: 1, error: "gone" });
    state = applyProgress(state, { kind: "progress", position: 2, fraction: 0.5 });
    expect(overallFraction(state)).toBeCloseTo(0.5);
    state = applyProgress(state, { kind: "downloaded", position: 2 });
    state = applyProgress(state, { kind: "alreadyPresent", position: 3 });
    expect(overallFraction(state)).toBe(1);
  });
});

describe("what is missing before a download", () => {
  test("nothing when everything is filled in", () => {
    expect(missingForDownload({ playlist: "mix", musicFolder: "/m", trackCount: 2 })).toEqual([]);
  });

  test("everything on an empty form, tracks first", () => {
    expect(missingForDownload({ playlist: "  ", musicFolder: "", trackCount: 0 })).toEqual([
      "tracks",
      "playlist",
      "musicFolder",
    ]);
  });
});
