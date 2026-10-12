// Every word the window shows, in the languages it speaks.

const en = {
  appTitle: "Stereo Stick",
  tagline: "From a list of songs to a USB stick your car stereo understands.",
  playlistLabel: "Playlist name",
  playlistHint: "It becomes the folder and the album your car shows.",
  playlistPlaceholder: "Road trip",
  folderLabel: "Music folder",
  folderHint: "Each playlist gets its own folder in here.",
  folderEmpty: "No folder chosen",
  chooseFolder: "Choose…",
  listLabel: "Songs",
  listHint: "One per line: a title with its artist, or a link.",
  listPlaceholder: "Queen - Bohemian Rhapsody\nhttps://…",
  removeNonMusic: "Cut out the parts that are not music",
  removeNonMusicHint: "Spoken intros, applause, scenes in the middle of a video clip.",
  check: "Check",
  download: "Download",
  working: "Working…",
  tracksTitle: "Tracks",
  noTracks: "Write a song above to get started.",
  toolMissing:
    "The download tool (yt-dlp) was not found on this computer. Install it, then reopen the app.",
  missingTracks: "Add at least one song.",
  missingPlaylist: "Give the playlist a name.",
  missingFolder: "Choose the music folder.",
  demoNotice: "Preview without the desktop app: nothing is really downloaded.",
  status: {
    idle: "",
    checking: "Checking…",
    found: "Found",
    flagged: "Has non-music parts",
    unavailable: "Not found",
    waiting: "Waiting",
    downloading: "Downloading",
    downloaded: "Downloaded",
    alreadyPresent: "Already there",
    failed: "Failed",
  },
  nonMusic: (seconds: string, parts: number) =>
    `${seconds} that are not music, in ${parts} ${parts === 1 ? "part" : "parts"}`,
  count: (n: number) => `${n} ${n === 1 ? "song" : "songs"}`,
  summary: (downloaded: number, present: number, failed: number) =>
    `${downloaded} downloaded · ${present} already there · ${failed} failed`,
  seconds: (n: number) => `${n} s`,
  minutes: (m: number, s: number) => `${m} min ${s} s`,
};

export type Messages = typeof en;

const it: Messages = {
  appTitle: "Stereo Stick",
  tagline: "Da una lista di canzoni a una chiavetta che la tua autoradio capisce.",
  playlistLabel: "Nome della playlist",
  playlistHint: "Diventa la cartella e l'album che vedi in auto.",
  playlistPlaceholder: "Viaggio in auto",
  folderLabel: "Cartella della musica",
  folderHint: "Ogni playlist avrà la sua cartella qui dentro.",
  folderEmpty: "Nessuna cartella scelta",
  chooseFolder: "Scegli…",
  listLabel: "Canzoni",
  listHint: "Una per riga: il titolo con l'artista, oppure un link.",
  listPlaceholder: "Queen - Bohemian Rhapsody\nhttps://…",
  removeNonMusic: "Taglia le parti che non sono musica",
  removeNonMusicHint: "Introduzioni parlate, applausi, scene in mezzo al videoclip.",
  check: "Controlla",
  download: "Scarica",
  working: "Sto lavorando…",
  tracksTitle: "Brani",
  noTracks: "Scrivi una canzone qui sopra per iniziare.",
  toolMissing:
    "Lo strumento di download (yt-dlp) non è stato trovato su questo computer. Installalo e riapri l'app.",
  missingTracks: "Aggiungi almeno una canzone.",
  missingPlaylist: "Dai un nome alla playlist.",
  missingFolder: "Scegli la cartella della musica.",
  demoNotice: "Anteprima senza l'app desktop: non viene scaricato nulla davvero.",
  status: {
    idle: "",
    checking: "Controllo…",
    found: "Trovato",
    flagged: "Ha parti non musicali",
    unavailable: "Non trovato",
    waiting: "In attesa",
    downloading: "Scarico",
    downloaded: "Scaricato",
    alreadyPresent: "Già presente",
    failed: "Non riuscito",
  },
  nonMusic: (seconds, parts) =>
    `${seconds} che non sono musica, in ${parts} ${parts === 1 ? "parte" : "parti"}`,
  count: (n) => `${n} ${n === 1 ? "canzone" : "canzoni"}`,
  summary: (downloaded, present, failed) =>
    `${downloaded} ${downloaded === 1 ? "scaricata" : "scaricate"} · ${present} già ${
      present === 1 ? "presente" : "presenti"
    } · ${failed} non ${failed === 1 ? "riuscita" : "riuscite"}`,
  seconds: (n) => `${n} s`,
  minutes: (m, s) => `${m} min ${s} s`,
};

const languages: Record<string, Messages> = { en, it };

/** The messages for the first preferred language the window speaks. */
export function messagesFor(preferred: readonly string[]): Messages {
  for (const tag of preferred) {
    const language = languages[tag.toLowerCase().split("-")[0] ?? ""];
    if (language) return language;
  }
  return en;
}

/** A length of time the way people say it: "45 s", "5 min 56 s". */
export function duration(messages: Messages, seconds: number): string {
  const whole = Math.max(0, Math.round(seconds));
  if (whole < 60) return messages.seconds(whole);
  return messages.minutes(Math.floor(whole / 60), whole % 60);
}
