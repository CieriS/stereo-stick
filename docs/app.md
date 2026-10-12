# Desktop app

`app/` is the window: a [Tauri 2](https://tauri.app) shell (`app/src-tauri`)
around a small TypeScript frontend (`app/src`), with no UI framework.

## Frontend

| File | Role |
|---|---|
| `state.ts` | What the window knows and how it changes (pure, tested) |
| `messages.ts` | Every word shown, in English and Italian (pure, tested) |
| `backend.ts` | The only door to the rest of the app: the `Backend` interface and its desktop version |
| `demo.ts` | A pretend `Backend` used when the page runs in a plain browser |
| `main.ts` | Draws the state and turns clicks into backend calls |

The language follows the system: Italian when it comes first among the
user's languages, English otherwise. A new language is one more object in
`messages.ts`; a test checks that no message is missing or empty.

## Shell

`app/src-tauri/src/lib.rs` exposes four commands, all thin:

| Command | What it does |
|---|---|
| `parse_list` | The tracks of the text, to show while the user types |
| `check_list` | Describes each track without downloading, one result at a time |
| `download_list` | Downloads the playlist into `<music folder>/<playlist>`, sending progress |
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
  with "missing" fails. Nothing is downloaded.

## Not there yet

- The installers do not bundle yt-dlp, ffmpeg and Deno yet. The app already
  looks for them in its `tools` resource folder (`app/src-tauri/tools` at
  build time) and falls back to `PATH`; what is missing is the step that
  fetches them for each system. An app opened from the Finder or the Start
  menu does not see the `PATH` of a terminal, so until then the app is only
  usable through `just dev`.
- No way to stop a download once started.
- The window is tested through its pure parts and by hand in the preview;
  there is no automated test that drives the real window.
