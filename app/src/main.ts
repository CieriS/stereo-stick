// The window itself: it draws the state and turns clicks into backend calls.

import { backend } from "./backend";
import { duration, messagesFor } from "./messages";
import {
  applyCheck,
  applyProgress,
  initialState,
  type Missing,
  missingForDownload,
  overallFraction,
  type State,
  settle,
  startChecking,
  startDownloading,
  summarize,
  type TrackView,
  withTracks,
} from "./state";

const t = messagesFor(navigator.languages);
document.documentElement.lang = t === messagesFor(["it"]) ? "it" : "en";
document.title = t.appTitle;

function el<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: Partial<HTMLElementTagNameMap[K]> & { class?: string } = {},
  ...children: (Node | string)[]
): HTMLElementTagNameMap[K] {
  const { class: className, ...rest } = props;
  const node = Object.assign(document.createElement(tag), rest);
  if (className) node.className = className;
  node.append(...children);
  return node;
}

const playlist = el("input", { id: "playlist", type: "text", placeholder: t.playlistPlaceholder });
const list = el("textarea", { id: "list", rows: 7, placeholder: t.listPlaceholder });
list.spellcheck = false;
const folderPath = el("output", { id: "folder", class: "folder-path" }, t.folderEmpty);
const chooseFolder = el("button", { type: "button", class: "secondary" }, t.chooseFolder);
const removeNonMusic = el("input", { id: "remove", type: "checkbox", checked: true });
const checkButton = el("button", { type: "button", class: "secondary" }, t.check);
const downloadButton = el("button", { type: "button", class: "primary" }, t.download);
const notice = el("p", { class: "notice", role: "alert", hidden: true });
const trackCount = el("span", { class: "muted" });
const overall = el("progress", { max: 1, value: 0, hidden: true });
overall.setAttribute("aria-label", t.working);
const summary = el("p", { class: "summary", hidden: true });
summary.setAttribute("aria-live", "polite");
const tracks = el("ol", { class: "tracks" });

function field(label: string, hint: string, control: HTMLElement, ...extra: Node[]) {
  const hintId = `${control.id}-hint`;
  control.setAttribute("aria-describedby", hintId);
  return el(
    "div",
    { class: "field" },
    el("label", { htmlFor: control.id }, label),
    el("div", { class: "control" }, control, ...extra),
    el("p", { class: "hint", id: hintId }, hint),
  );
}

const app = document.querySelector<HTMLElement>("#app");
if (!app) throw new Error("the page has no #app element");
app.append(
  el("header", {}, el("h1", {}, t.appTitle), el("p", { class: "muted" }, t.tagline)),
  ...(backend.demo ? [el("p", { class: "notice demo" }, t.demoNotice)] : []),
  el(
    "section",
    { class: "card form" },
    field(t.listLabel, t.listHint, list),
    el(
      "div",
      { class: "row" },
      field(t.playlistLabel, t.playlistHint, playlist),
      field(t.folderLabel, t.folderHint, folderPath, chooseFolder),
    ),
    el(
      "label",
      { class: "check" },
      removeNonMusic,
      el("span", {}, t.removeNonMusic, el("small", { class: "hint" }, t.removeNonMusicHint)),
    ),
    notice,
    el("div", { class: "actions" }, checkButton, downloadButton),
  ),
  el(
    "section",
    { class: "card" },
    el("div", { class: "tracks-head" }, el("h2", {}, t.tracksTitle), trackCount),
    overall,
    summary,
    tracks,
  ),
);

let state: State = initialState;
let musicFolder = "";
let toolAvailable = true;

function trackRow(track: TrackView): HTMLLIElement {
  const details: (Node | string)[] = [];
  if (track.title) details.push(el("span", { class: "muted" }, track.title));
  if (track.nonMusicParts) {
    const length = duration(t, track.nonMusicSeconds ?? 0);
    details.push(el("span", { class: "warning" }, t.nonMusic(length, track.nonMusicParts)));
  }
  if (track.error) details.push(el("span", { class: "error" }, track.error));

  const row = el(
    "li",
    { class: `track ${track.status}` },
    el("span", { class: "position" }, String(track.position).padStart(2, "0")),
    el("div", { class: "what" }, el("strong", {}, track.name), ...details),
    el("span", { class: "status" }, t.status[track.status]),
  );
  if (track.status === "downloading") {
    row.append(el("progress", { max: 1, value: track.fraction ?? 0 }));
  }
  return row;
}

const missingText: Record<Missing, string> = {
  tracks: t.missingTracks,
  playlist: t.missingPlaylist,
  musicFolder: t.missingFolder,
};

function showNotice(text: string | undefined) {
  notice.hidden = text === undefined;
  notice.textContent = text ?? "";
}

function render() {
  const busy = state.phase === "checking" || state.phase === "downloading";
  const empty = state.tracks.length === 0;
  for (const control of [playlist, list, chooseFolder, removeNonMusic]) control.disabled = busy;
  checkButton.disabled = busy || empty || !toolAvailable;
  downloadButton.disabled = busy || empty || !toolAvailable;
  downloadButton.textContent = busy ? t.working : t.download;

  trackCount.textContent = empty ? "" : t.count(state.tracks.length);
  tracks.replaceChildren(
    ...(empty ? [el("li", { class: "empty muted" }, t.noTracks)] : state.tracks.map(trackRow)),
  );

  overall.hidden = state.phase !== "downloading";
  overall.value = overallFraction(state);
  const totals = summarize(state);
  summary.hidden = state.phase !== "done";
  summary.textContent = t.summary(totals.downloaded, totals.alreadyPresent, totals.failed);
  summary.classList.toggle("has-failures", totals.failed > 0);
}

function set(next: State) {
  state = next;
  render();
}

const message = (error: unknown) => (error instanceof Error ? error.message : String(error));

let latestEdit = 0;
async function listEdited() {
  const edit = ++latestEdit;
  const parsed = await backend.parseList(list.value);
  // An answer to an older edit must not overwrite a newer one.
  if (edit === latestEdit) set(withTracks(parsed));
}

async function check() {
  showNotice(undefined);
  set(startChecking(state));
  try {
    await backend.checkList(list.value, (result) => set(applyCheck(state, result)));
    set(settle(state));
  } catch (error) {
    set(settle(state, message(error)));
  }
}

async function download() {
  const missing = missingForDownload({
    playlist: playlist.value,
    musicFolder,
    trackCount: state.tracks.length,
  });
  const first = missing[0];
  showNotice(first && missingText[first]);
  if (first) return;

  set(startDownloading(state));
  try {
    await backend.downloadList(
      {
        playlist: playlist.value,
        musicFolder,
        text: list.value,
        removeNonMusic: removeNonMusic.checked,
      },
      (event) => set(applyProgress(state, event)),
    );
    set(settle(state));
  } catch (error) {
    const reason = message(error);
    set(settle(state, reason === "playlist-name" ? t.missingPlaylist : reason));
  }
}

list.addEventListener("input", listEdited);
checkButton.addEventListener("click", check);
downloadButton.addEventListener("click", download);
chooseFolder.addEventListener("click", async () => {
  const chosen = await backend.chooseFolder();
  if (chosen) {
    musicFolder = chosen;
    folderPath.textContent = chosen;
    folderPath.title = chosen;
  }
});

render();
backend.toolAvailable().then((available) => {
  toolAvailable = available;
  if (!available) showNotice(t.toolMissing);
  render();
});
