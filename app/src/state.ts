// What the window knows, and how it changes. Pure: no DOM, no backend.

export interface Track {
  position: number;
  name: string;
}

export type CheckResult =
  | {
      kind: "found";
      position: number;
      title: string;
      nonMusicSeconds: number;
      nonMusicParts: number;
    }
  | { kind: "unavailable"; position: number; error: string };

export type Progress =
  | { kind: "started"; position: number }
  | { kind: "progress"; position: number; fraction: number }
  | { kind: "downloaded"; position: number }
  | { kind: "alreadyPresent"; position: number }
  | { kind: "failed"; position: number; error: string };

export type TrackStatus =
  | "idle"
  | "checking"
  | "found"
  | "flagged"
  | "unavailable"
  | "waiting"
  | "downloading"
  | "downloaded"
  | "alreadyPresent"
  | "failed";

export interface TrackView extends Track {
  status: TrackStatus;
  /** Title of the video the track resolved to. */
  title?: string;
  nonMusicSeconds?: number;
  nonMusicParts?: number;
  /** Fraction downloaded, from 0 to 1. */
  fraction?: number;
  error?: string;
}

export type Phase = "editing" | "checking" | "downloading" | "done";

export interface State {
  phase: Phase;
  tracks: TrackView[];
}

export const initialState: State = { phase: "editing", tracks: [] };

/** Replaces the tracks after the list was edited: what was known is void. */
export function withTracks(tracks: Track[]): State {
  return { phase: "editing", tracks: tracks.map((track) => ({ ...track, status: "idle" })) };
}

function update(
  state: State,
  position: number,
  change: (track: TrackView) => TrackView,
): TrackView[] {
  return state.tracks.map((track) => (track.position === position ? change(track) : track));
}

export function startChecking(state: State): State {
  return {
    phase: "checking",
    tracks: state.tracks.map(({ position, name }) => ({ position, name, status: "checking" })),
  };
}

export function applyCheck(state: State, result: CheckResult): State {
  const tracks = update(state, result.position, (track) =>
    result.kind === "found"
      ? {
          ...track,
          status: result.nonMusicParts > 0 ? "flagged" : "found",
          title: result.title,
          nonMusicSeconds: result.nonMusicSeconds,
          nonMusicParts: result.nonMusicParts,
        }
      : { ...track, status: "unavailable", error: result.error },
  );
  return { ...state, tracks };
}

/** The work in progress ended, well or badly: nothing is left spinning. */
export function settle(state: State, error?: string): State {
  const pending: TrackStatus[] = ["checking", "waiting", "downloading"];
  const downloading = state.phase === "downloading";
  const tracks = state.tracks.map((track): TrackView => {
    if (!pending.includes(track.status)) return track;
    if (downloading || error !== undefined) {
      return { ...track, status: downloading ? "failed" : "unavailable", error };
    }
    return { ...track, status: "idle" };
  });
  return { phase: downloading ? "done" : "editing", tracks };
}

/** Keeps what the check found (title, non-music parts) next to the download. */
export function startDownloading(state: State): State {
  return {
    phase: "downloading",
    tracks: state.tracks.map((track) => ({
      ...track,
      status: "waiting",
      fraction: undefined,
      error: undefined,
    })),
  };
}

export function applyProgress(state: State, event: Progress): State {
  const tracks = update(state, event.position, (track): TrackView => {
    switch (event.kind) {
      case "started":
        return { ...track, status: "downloading", fraction: 0 };
      case "progress":
        return { ...track, status: "downloading", fraction: event.fraction };
      case "downloaded":
        return { ...track, status: "downloaded", fraction: 1 };
      case "alreadyPresent":
        return { ...track, status: "alreadyPresent" };
      case "failed":
        return { ...track, status: "failed", error: event.error };
    }
  });
  return { ...state, tracks };
}

export interface Summary {
  total: number;
  downloaded: number;
  alreadyPresent: number;
  failed: number;
  flagged: number;
}

export function summarize(state: State): Summary {
  const count = (status: TrackStatus) => state.tracks.filter((t) => t.status === status).length;
  return {
    total: state.tracks.length,
    downloaded: count("downloaded"),
    alreadyPresent: count("alreadyPresent"),
    failed: count("failed"),
    flagged: state.tracks.filter((t) => (t.nonMusicParts ?? 0) > 0).length,
  };
}

/** Fraction of the whole playlist that is done, for the overall bar. */
export function overallFraction(state: State): number {
  if (state.tracks.length === 0) return 0;
  const done: TrackStatus[] = ["downloaded", "alreadyPresent", "failed"];
  const sum = state.tracks.reduce((total, track) => {
    if (done.includes(track.status)) return total + 1;
    return total + (track.status === "downloading" ? (track.fraction ?? 0) : 0);
  }, 0);
  return sum / state.tracks.length;
}

export interface Form {
  playlist: string;
  musicFolder: string;
  trackCount: number;
}

export type Missing = "playlist" | "musicFolder" | "tracks";

/** What still has to be filled in before a download can start. */
export function missingForDownload(form: Form): Missing[] {
  const missing: Missing[] = [];
  if (form.trackCount === 0) missing.push("tracks");
  if (form.playlist.trim() === "") missing.push("playlist");
  if (form.musicFolder === "") missing.push("musicFolder");
  return missing;
}
