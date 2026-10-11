# youtube-bulk-downloader

> Working title: the app will get its own name before the first release.

Turn a list of songs into a USB stick your car stereo understands: tagged MP3
files, one folder per playlist, covers the dashboard can show.

It started as a pair of shell scripts. The goal is a desktop app for people
who do not use a terminal: paste titles or links, pick a folder, press a
button.

**Status: early development.** The engine works and is tested; there is no
window yet.

## What it does today

- Reads a track list: one song per line, as a link or as plain text to search.
- Checks every track before downloading and **flags videos that contain parts
  that are not music** (spoken intros, applause, story scenes in the middle of
  a video clip), with the option to cut them out.
- Downloads each track as a 320 kbps MP3 with a JPG cover, the playlist name
  as album (car stereos group by album) and the channel as artist when the
  video has none.
- Reports the outcome track by track; one failure never stops the rest.

## Roadmap

1. ~~Engine: track list, checks, download, report~~
2. Desktop app (Tauri): playlists, progress per track, final report
3. USB stick: detect the drive, copy only what changed, eject
4. Song details from a music catalogue (official title, artist, length) to
   pick the right video and to catch edits the community has not reviewed
5. Import a playlist from Spotify
6. Installers for Windows, macOS and Linux, with the download tools built in

## Development

Needs [Rust](https://rustup.rs), [just](https://just.systems) and, to try it
against real videos, [yt-dlp](https://github.com/yt-dlp/yt-dlp) with ffmpeg
and Deno in `PATH`.

| Command | What it does |
|---|---|
| `just check` | Formatting, lints and tests: what CI runs |
| `just test` | Tests only |
| `just fmt` | Format the code |
| `cargo run --example check -- list.txt` | Describe the tracks of a list without downloading |

The tests never touch the network: a stand-in script plays the part of yt-dlp.

## Structure

| Path | Content |
|---|---|
| `crates/engine` | Everything that is not user interface; see [docs/engine.md](docs/engine.md) |
| `docs/` | Documentation by area |

## Responsible use

This is a tool, like a browser or a recorder. Download only what you have the
right to download: your own content, content under a free licence, or content
whose owner allows it. You are responsible for respecting the terms of the
sites you use and the copyright law of your country.

## Licence

[MIT](LICENSE)
