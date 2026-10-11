# Engine

`crates/engine` holds the logic of the app and no user interface. Pure code
sits in the middle; the only way out to the world is the `Downloader` trait.

| Module | Role |
|---|---|
| `tracklist` | Parses a track list into entries (pure) |
| `probe` | Reads the description of a video and its non-music parts (pure) |
| `ytdlp` | Builds the yt-dlp command lines and reads its error output (pure) |
| `playlist` | The `Downloader` port, plus checking and downloading a whole playlist |
| `process` | The adapter that runs the real yt-dlp program |

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

## Not there yet

- Progress while a track downloads (the adapter waits for yt-dlp to finish).
- Skipping tracks that are already on disk.
- Cleaning up the cover file left behind when a download fails halfway.
