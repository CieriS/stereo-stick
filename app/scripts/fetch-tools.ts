// Downloads the programs the app ships with into src-tauri/tools, checking
// each download against the checksum pinned in tools.json.
// Run with `bun run fetch-tools`.

import { createHash } from "node:crypto";
import { chmod, copyFile, mkdir, mkdtemp, readFile, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  extractCommand,
  fileName,
  isCurrent,
  type Stamp,
  sameChecksum,
  type Tool,
  toolsFor,
} from "./tools";
import manifest from "./tools.json";

const toolsDir = join(import.meta.dir, "..", "src-tauri", "tools");
const stampFile = join(toolsDir, ".fetched.json");
const { platform, arch } = process;

const exists = (path: string) =>
  stat(path).then(
    () => true,
    () => false,
  );

async function readStamp(): Promise<Stamp> {
  try {
    return JSON.parse(await readFile(stampFile, "utf8"));
  } catch {
    return {};
  }
}

const work = await mkdtemp(join(tmpdir(), "fetch-tools-"));

/** Downloads and verifies a tool's file once, even when two tools share it. */
const downloads = new Map<string, string>();
async function download(tool: Tool): Promise<string> {
  const known = downloads.get(tool.url);
  if (known) return known;

  console.log(`${tool.name}: downloading ${tool.url}`);
  const response = await fetch(tool.url);
  if (!response.ok) throw new Error(`${tool.name}: ${response.status} from ${tool.url}`);
  const bytes = new Uint8Array(await response.arrayBuffer());
  const checksum = createHash("sha256").update(bytes).digest("hex");
  if (!sameChecksum(tool.sha256, checksum)) {
    throw new Error(`${tool.name}: checksum is ${checksum}, expected ${tool.sha256}`);
  }
  const path = join(work, `${downloads.size}.download`);
  await writeFile(path, bytes);
  downloads.set(tool.url, path);
  return path;
}

await mkdir(toolsDir, { recursive: true });
const stamp = await readStamp();

try {
  for (const tool of toolsFor(manifest, platform, arch)) {
    const target = join(toolsDir, fileName(tool, platform));
    if (isCurrent(tool, stamp, await exists(target))) {
      console.log(`${tool.name}: already there`);
      continue;
    }

    const downloaded = await download(tool);
    let program = downloaded;
    if (tool.member) {
      const command = extractCommand(
        platform,
        downloaded,
        tool.member,
        work,
        process.env.SystemRoot,
      );
      const extraction = Bun.spawnSync(command, { stdout: "inherit", stderr: "inherit" });
      if (extraction.exitCode !== 0)
        throw new Error(`${tool.name}: could not extract ${tool.member}`);
      program = join(work, tool.member);
    }
    await rm(target, { force: true });
    await copyFile(program, target);
    await chmod(target, 0o755);

    stamp[tool.name] = tool.sha256;
    await writeFile(stampFile, `${JSON.stringify(stamp, null, 2)}\n`);
    console.log(`${tool.name}: ready`);
  }
} finally {
  await rm(work, { recursive: true, force: true });
}
