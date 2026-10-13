# Engine

`crates/engine` holds the logic of the app and no user interface. Pure code
sits in the middle; the only way out to the world is the `Downloader` trait.

| Module | Role |
|---|---|
| `tracklist` | Parses a track list into entries (pure) |
| `probe` | Reads the description of a video and its non-music parts (pure) |
| `ytdlp` | Builds the yt-dlp command lines and reads its error output (pure) |
| `playlist` | The `Downloader` port, plus checking and downloading a whole playlist, with events and a report |
| `files` | Which tracks are already in a folder, cleaning up after a failure, safe folder names |
| `stick` | What to copy to a USB stick, and the copy itself |
| `process` | The adapter that runs the real yt-dlp program, and tells whether it can be started |

## Track list format

One track per line.

```text
# Artist - Title          <- optional name for the line right below
ytsearch1:"Artist Title"  <- format of the original scripts
Artist - Title            <- plain text is searched as typed
https://…                 <- a link is used as it is
```

Blank lines are skipped. The position of a track in the list prefixes its file
name (`07_Title.mp3`), so downloading a list again gives the same names.

## Non-music parts

Music videos often include parts that are not the song. The check asks yt-dlp
for the [SponsorBlock](https://sponsor.ajay.app) `music_offtopic` segments of
the video and reports them; downloading with `NonMusic::Remove` cuts them out.

The data is crowd-sourced: a video with no segments may simply not have been
reviewed. "No warning" therefore means "nothing known", not "clean". Comparing
the length of the video with the official length of the song will cover that
case once song details are available (see the roadmap).

## yt-dlp behaviours the code relies on

- One yt-dlp run per track: the exit status is the outcome of that track.
- `--parse-metadata "FROM:TO"` reads a `FROM` made only of letters and
  underscores as a *field name*, so a playlist called `sc_song_ita` would be
  tagged with the album `NA`. `ytdlp::literal` puts an always-empty field in
  front (`%(_literal|)s…`) and escapes `%` and `:`.
- YouTube support needs a JavaScript runtime next to yt-dlp (Deno by default)
  since yt-dlp 2025.11.12. The installers will have to ship one.

## Downloading a playlist

`playlist::download` takes a `Request` and reports `Event`s as they happen
(`Started`, `Progress`, `Finished`), then returns a `Report`. Each track ends
as `Downloaded`, `AlreadyPresent`, `Stopped` or `Failed`.

- **Already there**: a track is skipped when the folder holds an MP3 whose
  name begins with its position (`07_`). Tracks are recognised by position
  alone, so new songs belong at the end of a list: inserting or reordering
  makes the list and the folder disagree. Pass an empty `existing` to download
  everything again.
- **Progress**: yt-dlp prints one `PROGRESS <downloaded> <total>` line per
  update (`--progress-template`); it covers the download, not the conversion
  to MP3 that follows. Downloads of unknown size report nothing.
- **Stopping**: setting the request's `stop` flag abandons the track in
  flight and marks it and the ones after it as `Stopped`. The adapter ends
  yt-dlp together with everything it started (the released yt-dlp is a
  launcher that starts the real program, which starts ffmpeg), and deletes
  the files of the abandoned track.
- **After a failure**: every file of that track is deleted, the MP3 too:
  yt-dlp writes it before the steps that cut and tag it, so after a failure it
  cannot be trusted, and left there it would pass for a finished track.

## USB stick

`stick::scan` lists the tracks of a folder: MP3 files inside its playlist
folders. Hidden files and folders (`.DS_Store`, `._name`), which make some
car stereos stumble, are left out, and so is anything that is not music.
`stick::plan` compares the music folder with the stick and returns what is
missing or changed there: a different size, or a copy older than the
original by more than two seconds (FAT keeps times to two seconds).
`stick::copy` copies those tracks through a `.part` name, so an interrupted
copy never passes for a finished file.

Nothing on the stick is ever deleted, and folders that contain each other
are refused (`stick::overlap`).

Downloads use `--windows-filenames`, so that titles with characters FAT
refuses (`?`, `:`, `"`) still produce files a stick accepts.

## Tools

yt-dlp needs ffmpeg (conversion, covers) and a JavaScript runtime (Deno) for
YouTube. `YtDlp::new` expects all three in `PATH`. `YtDlp::bundled(folder)`
uses the copies found in a folder (ffprobe must sit next to ffmpeg: cutting
the non-music parts needs it) and tells yt-dlp where they are
(`--ffmpeg-location`, `--js-runtimes deno:<path>`). Every run passes
`--ignore-config`, so a yt-dlp configuration file on the user's computer
cannot change what the app does.
