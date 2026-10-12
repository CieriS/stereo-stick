# Programs shipped with Stereo Stick

Stereo Stick itself is under the [MIT licence](LICENSE). The installers also
contain the programs below, unmodified, each under its own licence. They are
separate programs that the app starts; they are not linked into it.

| Program | What it does here | Licence | Source |
|---|---|---|---|
| yt-dlp | Finds and downloads the audio | Unlicense; the released executable bundles components under GPLv3+ | <https://github.com/yt-dlp/yt-dlp> |
| FFmpeg (`ffmpeg`, `ffprobe`) | Converts to MP3, cuts, embeds covers | GPL version 3 (builds configured with `--enable-gpl --enable-version3`) | <https://ffmpeg.org>; macOS and Linux builds from <https://ffmpeg.martin-riedl.de>, Windows builds from <https://github.com/yt-dlp/FFmpeg-Builds> |
| Deno | Runs the JavaScript yt-dlp needs for some sites | MIT | <https://github.com/denoland/deno> |

The exact files, with their addresses and checksums, are listed in
[`app/scripts/tools.json`](app/scripts/tools.json). The source code of each
program is available from the links above; the pages of the FFmpeg builds
link to the build scripts used.

Inside an installed app these programs are in the `tools` folder of the
app's resources and can be replaced with other builds.
