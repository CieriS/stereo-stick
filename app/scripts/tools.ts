// Which programs ship inside the app, where they come from and how to
// unpack them. Pure: the downloading is in fetch-tools.ts.

export interface Tool {
  /** File name inside the app, without `.exe`. */
  name: string;
  url: string;
  /** Expected SHA-256 of the downloaded file, in hex. */
  sha256: string;
  /** Path of the program inside the archive; absent when the download is the program. */
  member?: string;
}

export type Manifest = Record<string, Tool[]>;

/** The tools for a system such as `darwin-arm64`, or an error naming the ones supported. */
export function toolsFor(manifest: Manifest, platform: string, arch: string): Tool[] {
  const key = `${platform}-${arch}`;
  const tools = manifest[key];
  if (!tools) {
    const supported = Object.keys(manifest).sort().join(", ");
    throw new Error(`no bundled tools for ${key}; supported: ${supported}`);
  }
  return tools;
}

/** The name of a tool's file on a system. */
export function fileName(tool: Tool, platform: string): string {
  return platform === "win32" ? `${tool.name}.exe` : tool.name;
}

export function sameChecksum(expected: string, actual: string): boolean {
  return expected.trim().toLowerCase() === actual.trim().toLowerCase();
}

/**
 * The command that extracts one member of a zip archive into a folder.
 * Linux has `unzip`; macOS and Windows ship a `tar` that reads zip files,
 * but on Windows a different `tar` may come first in `PATH`, hence the full path.
 */
export function extractCommand(
  platform: string,
  archive: string,
  member: string,
  destination: string,
  systemRoot = "C:\\Windows",
): string[] {
  if (platform === "linux") return ["unzip", "-o", "-q", archive, member, "-d", destination];
  const tar = platform === "win32" ? `${systemRoot}\\System32\\tar.exe` : "tar";
  return [tar, "-xf", archive, "-C", destination, member];
}

/** What was fetched last time, to skip tools that are already there. */
export type Stamp = Record<string, string>;

export function isCurrent(tool: Tool, stamp: Stamp, present: boolean): boolean {
  return present && stamp[tool.name] === tool.sha256;
}
