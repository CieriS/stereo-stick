import { describe, expect, test } from "bun:test";
import {
  applyStick,
  idleStick,
  missingForCopy,
  type StickEvent,
  settleStick,
  startCopying,
  stickFraction,
} from "./stick";

const run = (events: StickEvent[]) => events.reduce(applyStick, startCopying());

describe("copying to the stick", () => {
  test("starts from nothing", () => {
    expect(startCopying()).toEqual({ ...idleStick, phase: "copying" });
    expect(stickFraction(startCopying())).toBe(0);
  });

  test("follows the plan to the end", () => {
    const events: StickEvent[] = [
      { kind: "planned", toCopy: 4, upToDate: 6 },
      { kind: "copied", path: "a/1.mp3" },
      { kind: "failed", path: "a/2.mp3", error: "disk full" },
    ];
    const midway = run(events);
    expect(midway).toMatchObject({ phase: "copying", toCopy: 4, upToDate: 6, copied: 1 });
    expect(midway.failed).toEqual([{ path: "a/2.mp3", error: "disk full" }]);
    expect(stickFraction(midway)).toBe(0.5);

    const done = applyStick(midway, { kind: "finished", stopped: 2 });
    expect(done).toMatchObject({ phase: "done", stopped: 2, copied: 1 });
  });

  test("nothing to copy is a finished copy", () => {
    const done = run([
      { kind: "planned", toCopy: 0, upToDate: 9 },
      { kind: "finished", stopped: 0 },
    ]);
    expect(done).toMatchObject({ phase: "done", copied: 0, upToDate: 9 });
    expect(stickFraction(done)).toBe(1);
  });

  test("a copy that breaks ends with its error", () => {
    const broken = settleStick(run([{ kind: "planned", toCopy: 3, upToDate: 0 }]), "unplugged");
    expect(broken).toMatchObject({ phase: "done", error: "unplugged" });
  });

  test("a copy that finished is not touched by settling", () => {
    const done = run([{ kind: "finished", stopped: 0 }]);
    expect(settleStick(done, "late error")).toBe(done);
  });
});

describe("what is missing before a copy", () => {
  test("the music folder first, then the stick", () => {
    expect(missingForCopy("", "")).toBe("musicFolder");
    expect(missingForCopy("/music", "")).toBe("stickFolder");
    expect(missingForCopy("/music", "/stick")).toBeUndefined();
  });
});
