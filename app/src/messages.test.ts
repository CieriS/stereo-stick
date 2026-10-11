import { describe, expect, test } from "bun:test";
import { duration, messagesFor } from "./messages";

const en = messagesFor(["en"]);
const it = messagesFor(["it"]);

/** Every leaf of a message tree, functions called with sample arguments. */
function leaves(value: unknown, path = ""): [string, string][] {
  if (typeof value === "string") return [[path, value]];
  if (typeof value === "function") return [[path, String(value(2, 3, 4))]];
  return Object.entries(value as object).flatMap(([key, child]) => leaves(child, `${path}.${key}`));
}

describe("languages", () => {
  test("the first language the window speaks wins", () => {
    expect(messagesFor(["fr-FR", "it-IT", "en"])).toBe(it);
    expect(messagesFor(["IT"])).toBe(it);
    expect(messagesFor(["en-GB"])).toBe(en);
  });

  test("unknown or missing preferences fall back to English", () => {
    expect(messagesFor(["de-DE"])).toBe(en);
    expect(messagesFor([])).toBe(en);
    expect(messagesFor([""])).toBe(en);
  });

  test("Italian has every message English has, and none is left empty", () => {
    const english = leaves(en);
    const italian = leaves(it);
    expect(italian.map(([path]) => path)).toEqual(english.map(([path]) => path));
    for (const [path, text] of italian) {
      if (path !== ".status.idle") expect(text.trim(), path).not.toBe("");
    }
  });
});

describe("plurals", () => {
  test("English", () => {
    expect(en.count(1)).toBe("1 song");
    expect(en.count(0)).toBe("0 songs");
    expect(en.nonMusic("5 s", 1)).toBe("5 s that are not music, in 1 part");
  });

  test("Italian", () => {
    expect(it.count(1)).toBe("1 canzone");
    expect(it.count(3)).toBe("3 canzoni");
    expect(it.nonMusic("5 s", 2)).toBe("5 s che non sono musica, in 2 parti");
    expect(it.summary(1, 1, 1)).toBe("1 scaricata · 1 già presente · 1 non riuscita");
    expect(it.summary(2, 0, 3)).toBe("2 scaricate · 0 già presenti · 3 non riuscite");
  });
});

describe("durations", () => {
  test("seconds below a minute, minutes and seconds above", () => {
    expect(duration(en, 0)).toBe("0 s");
    expect(duration(en, 4.6)).toBe("5 s");
    expect(duration(en, 59.4)).toBe("59 s");
    expect(duration(en, 59.6)).toBe("1 min 0 s");
    expect(duration(en, 356)).toBe("5 min 56 s");
  });

  test("a negative length is shown as zero", () => {
    expect(duration(en, -3)).toBe("0 s");
  });
});
