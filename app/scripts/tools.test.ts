import { describe, expect, test } from "bun:test";
import { extractCommand, fileName, isCurrent, sameChecksum, type Tool, toolsFor } from "./tools";
import manifest from "./tools.json";

const tool: Tool = {
  name: "ffmpeg",
  url: "https://example.com/f.zip",
  sha256: "ab12",
  member: "ffmpeg",
};

describe("the manifest", () => {
  const systems = ["darwin-arm64", "darwin-x64", "linux-x64", "win32-x64"];

  test("covers the systems the app is built for", () => {
    expect(Object.keys(manifest).sort()).toEqual(systems);
  });

  test("every system gets yt-dlp, deno, ffmpeg and ffprobe, pinned and over https", () => {
    for (const system of systems) {
      const [platform = "", arch = ""] = system.split("-");
      const tools = toolsFor(manifest, platform, arch);
      expect(tools.map((t) => t.name)).toEqual(["yt-dlp", "deno", "ffmpeg", "ffprobe"]);
      for (const t of tools) {
        expect(t.url, t.url).toStartWith("https://");
        expect(t.url, "no moving address").not.toContain("/latest/");
        expect(t.sha256, t.url).toMatch(/^[0-9a-f]{64}$/);
      }
    }
  });

  test("Windows members are .exe files", () => {
    for (const t of toolsFor(manifest, "win32", "x64")) {
      expect(t.member ?? t.url).toEndWith(".exe");
    }
  });
});

describe("choosing tools", () => {
  test("an unsupported system is named in the error", () => {
    expect(() => toolsFor(manifest, "linux", "arm64")).toThrow(
      "no bundled tools for linux-arm64; supported: darwin-arm64, darwin-x64, linux-x64, win32-x64",
    );
  });

  test("file names get .exe only on Windows", () => {
    expect(fileName(tool, "win32")).toBe("ffmpeg.exe");
    expect(fileName(tool, "darwin")).toBe("ffmpeg");
    expect(fileName(tool, "linux")).toBe("ffmpeg");
  });
});

describe("checksums", () => {
  test("compare ignoring case and surrounding spaces", () => {
    expect(sameChecksum("AB12", " ab12\n")).toBe(true);
    expect(sameChecksum("ab12", "ab13")).toBe(false);
    expect(sameChecksum("ab12", "")).toBe(false);
  });
});

describe("extracting", () => {
  test("unzip on Linux", () => {
    expect(extractCommand("linux", "a.zip", "ffmpeg", "out")).toEqual([
      "unzip",
      "-o",
      "-q",
      "a.zip",
      "ffmpeg",
      "-d",
      "out",
    ]);
  });

  test("the system tar on macOS and Windows", () => {
    expect(extractCommand("darwin", "a.zip", "deno", "out")).toEqual([
      "tar",
      "-xf",
      "a.zip",
      "-C",
      "out",
      "deno",
    ]);
    expect(extractCommand("win32", "a.zip", "bin/ffmpeg.exe", "out", "D:\\Win")[0]).toBe(
      "D:\\Win\\System32\\tar.exe",
    );
  });
});

describe("skipping what is already there", () => {
  test("only when the file is present and its stamp matches", () => {
    expect(isCurrent(tool, { ffmpeg: "ab12" }, true)).toBe(true);
    expect(isCurrent(tool, { ffmpeg: "ab12" }, false)).toBe(false);
    expect(isCurrent(tool, { ffmpeg: "old" }, true)).toBe(false);
    expect(isCurrent(tool, {}, true)).toBe(false);
  });
});
