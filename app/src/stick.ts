// What the window knows about copying to the USB stick. Pure.

export type StickEvent =
  | { kind: "planned"; toCopy: number; upToDate: number }
  | { kind: "copied"; path: string }
  | { kind: "failed"; path: string; error: string }
  | { kind: "finished"; stopped: number };

export interface StickState {
  phase: "idle" | "copying" | "done";
  toCopy: number;
  upToDate: number;
  copied: number;
  failed: { path: string; error: string }[];
  stopped: number;
  /** Why the copy could not even start. */
  error?: string;
}

export const idleStick: StickState = {
  phase: "idle",
  toCopy: 0,
  upToDate: 0,
  copied: 0,
  failed: [],
  stopped: 0,
};

export function startCopying(): StickState {
  return { ...idleStick, phase: "copying" };
}

export function applyStick(state: StickState, event: StickEvent): StickState {
  switch (event.kind) {
    case "planned":
      return { ...state, toCopy: event.toCopy, upToDate: event.upToDate };
    case "copied":
      return { ...state, copied: state.copied + 1 };
    case "failed":
      return { ...state, failed: [...state.failed, { path: event.path, error: event.error }] };
    case "finished":
      return { ...state, phase: "done", stopped: event.stopped };
  }
}

/** The copy ended without its last event: it broke, or never started. */
export function settleStick(state: StickState, error?: string): StickState {
  return state.phase === "done" ? state : { ...state, phase: "done", error };
}

/** Fraction of the tracks to copy that have been dealt with. */
export function stickFraction(state: StickState): number {
  if (state.toCopy === 0) return state.phase === "done" ? 1 : 0;
  return (state.copied + state.failed.length) / state.toCopy;
}

/** What is wrong with the two folders, if anything, before asking to copy. */
export function missingForCopy(musicFolder: string, stickFolder: string) {
  if (musicFolder === "") return "musicFolder" as const;
  if (stickFolder === "") return "stickFolder" as const;
  return undefined;
}
