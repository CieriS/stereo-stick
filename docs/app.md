# Desktop app

`app/` is the window: a [Tauri 2](https://tauri.app) shell (`app/src-tauri`)
around a small TypeScript frontend (`app/src`), with no UI framework.

## Frontend

| File | Role |
|---|---|
| `stick.ts` | What the window knows about copying to the stick (pure, tested) |
| `state.ts` | What the window knows and how it changes (pure, tested) |
| `messages.ts` | Every word shown, in English and Italian (pure, tested) |
| `backend.ts` | The only door to the rest of the app: the `Backend` interface and its desktop version |
| `demo.ts` | A pretend `Backend` used when the page runs in a plain browser |
| `main.ts` | Draws the state and turns clicks into backend calls |

The language follows the system: Italian when it comes first among the
user's languages, English otherwise. A new language is one more object in
`messages.ts`; a test checks that no message is missing or empty.

## Shell

`app/src-tauri/src/lib.rs` exposes six commands, all thin:

| Command | What it does |
|---|---|
| `parse_list` | The tracks of the text, to show while the user types |
| `check_list` | Describes each track without downloading, one result at a time |
| `download_list` | Downloads the playlist into `<music folder>/<playlist>`, sending progress |
| `copy_to_stick` | Copies what is new or changed from the music folder to the stick |
| `stop_download` | Raises the flag that stops the download or the copy in progress |
| `tool_available` | Whether yt-dlp can be started |

`dto.rs` converts the engine's types into what the window receives; the
shapes are pinned by tests and mirrored by the types in `state.ts`.

Work that waits on yt-dlp runs off the window's thread, so the window never
freezes.

## Trying it

- `just dev` opens the real app. It uses the `yt-dlp` found in `PATH`, with
  ffmpeg and Deno next to it.
- `just preview` serves the window alone at <http://localhost:4340> with the
  pretend backend: a song with "live" in its name has non-music parts, one
  with "missing" fails. Nothing is downloaded, and Stop works as in the app.

## Installers

`just bundle` fetches the tools and builds the installer for the system it
runs on. `app/scripts/tools.json` pins, for each system, the address and the
SHA-256 of yt-dlp, ffmpeg, ffprobe and Deno; `bun run fetch-tools` downloads
them into `app/src-tauri/tools`, refusing any file whose checksum differs.
That folder is bundled as a resource, and at run time the app uses the tools
in it, falling back to `PATH` when it is empty (development).

To update a tool, change its address and checksum in `tools.json`.

The `Release` workflow builds the installers for Linux, Windows and macOS
(Apple Silicon) on every `v*` tag and attaches them to a draft release; it
also runs on pull requests that touch the packaging.

The installers are not signed: macOS and Windows warn before opening them.

## Not there yet

- No installer for Intel Macs, although the tools for them are pinned.
- yt-dlp inside an installed app does not update itself.
- The stick is chosen as a folder: the app does not list removable drives and
  does not eject them.

- The window is tested through its pure parts and by hand in the preview;
  there is no automated test that drives the real window.
