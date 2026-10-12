# Bundled tools

The programs the app ships with go here before a release build: `yt-dlp`,
`ffmpeg` and `deno` (with `.exe` on Windows). Everything in this folder
except this file is ignored by git.

When `yt-dlp` is here the app uses these copies; otherwise it falls back to
the programs found in `PATH`, which is how development works.
