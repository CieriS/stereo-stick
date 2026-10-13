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
import {
  applyStick,
  idleStick,
  missingForCopy,
  type StickState,
  settleStick,
  startCopying,
  stickFraction,
} from "./stick";

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
const stopButton = el("button", { type: "button", class: "secondary", hidden: true }, t.stop);
const notice = el("p", { class: "notice", role: "alert", hidden: true });
const trackCount = el("span", { class: "muted" });
const overall = el("progress", { max: 1, value: 0, hidden: true });
overall.setAttribute("aria-label", t.working);
const summary = el("p", { class: "summary", hidden: true });
summary.setAttribute("aria-live", "polite");
const tracks = el("ol", { class: "tracks" });
const stickPath = el("output", { id: "stick", class: "folder-path" }, t.stickEmpty);
const chooseStick = el("button", { type: "button", class: "secondary" }, t.chooseFolder);
const copyButton = el("button", { type: "button", class: "primary" }, t.copyToStick);
const stopCopyButton = el("button", { type: "button", class: "secondary", hidden: true }, t.stop);
const stickNotice = el("p", { class: "notice", role: "alert", hidden: true });
const stickProgress = el("progress", { max: 1, value: 0, hidden: true });
stickProgress.setAttribute("aria-label", t.working);
const stickSummary = el("p", { class: "summary", hidden: true });
stickSummary.setAttribute("aria-live", "polite");
const stickFailures = el("ul", { class: "failures", hidden: true });

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
    el("div", { class: "actions" }, checkButton, stopButton, downloadButton),
  ),
  el(
    "section",
    { class: "card" },
    el("div", { class: "tracks-head" }, el("h2", {}, t.tracksTitle), trackCount),
    overall,
    summary,
    tracks,
  ),
  el(
    "section",
    { class: "card" },
    el("h2", {}, t.stickTitle),
    field(t.stickFolderLabel, t.stickHint, stickPath, chooseStick),
    stickNotice,
    stickProgress,
    stickSummary,
    stickFailures,
    el("div", { class: "actions" }, stopCopyButton, copyButton),
  ),
);

let state: State = initialState;
let stick: StickState = idleStick;
let stickFolder = "";
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

function renderStick(busy: boolean) {
  const copying = stick.phase === "copying";
  chooseStick.disabled = copying;
  copyButton.disabled = copying || busy;
  copyButton.textContent = copying ? t.working : t.copyToStick;
  stopCopyButton.hidden = !copying;
  if (!copying) {
    stopCopyButton.disabled = false;
    stopCopyButton.textContent = t.stop;
  }
  stickProgress.hidden = !copying;
  stickProgress.value = stickFraction(stick);

  const done = stick.phase === "done" && stick.error === undefined;
  const nothingToDo = done && stick.toCopy === 0;
  const outcome = t.stickSummary(stick.copied, stick.upToDate, stick.failed.length);
  const lines = nothingToDo
    ? [t.stickUpToDate]
    : [stick.stopped > 0 ? `${outcome} · ${t.stoppedCount(stick.stopped)}` : outcome];
  stickSummary.hidden = !done;
  stickSummary.textContent = [...lines, t.stickEject].join(" ");
  stickSummary.classList.toggle("has-failures", stick.failed.length > 0);
  stickFailures.hidden = stick.failed.length === 0;
  stickFailures.replaceChildren(
    ...stick.failed.map(({ path, error }) =>
      el("li", {}, el("strong", {}, path), " ", el("span", { class: "error" }, error)),
    ),
  );
  if (stick.error !== undefined) {
    stickNotice.hidden = false;
    stickNotice.textContent = stick.error === "stick-overlap" ? t.stickOverlap : stick.error;
  }
}

function render() {
  const copying = stick.phase === "copying";
  const busy = state.phase === "checking" || state.phase === "downloading" || copying;
  const empty = state.tracks.length === 0;
  for (const control of [playlist, list, chooseFolder, removeNonMusic]) control.disabled = busy;
  checkButton.disabled = busy || empty || !toolAvailable;
  downloadButton.disabled = busy || empty || !toolAvailable;
  downloadButton.textContent = busy ? t.working : t.download;
  renderStick(state.phase === "checking" || state.phase === "downloading");
  stopButton.hidden = state.phase !== "downloading";
  if (state.phase !== "downloading") {
    stopButton.disabled = false;
    stopButton.textContent = t.stop;
  }

  trackCount.textContent = empty ? "" : t.count(state.tracks.length);
  tracks.replaceChildren(
    ...(empty ? [el("li", { class: "empty muted" }, t.noTracks)] : state.tracks.map(trackRow)),
  );

  overall.hidden = state.phase !== "downloading";
  overall.value = overallFraction(state);
  const totals = summarize(state);
  summary.hidden = state.phase !== "done";
  const outcome = t.summary(totals.downloaded, totals.alreadyPresent, totals.failed);
  summary.textContent =
    totals.stopped > 0 ? `${outcome} · ${t.stoppedCount(totals.stopped)}` : outcome;
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

async function copyToStick() {
  const missing = missingForCopy(musicFolder, stickFolder);
  stickNotice.hidden = missing === undefined;
  stickNotice.textContent =
    missing === "musicFolder" ? t.missingFolder : missing === "stickFolder" ? t.missingStick : "";
  if (missing) return;

  const setStick = (next: StickState) => {
    stick = next;
    render();
  };
  setStick(startCopying());
  try {
    await backend.copyToStick(musicFolder, stickFolder, (event) =>
      setStick(applyStick(stick, event)),
    );
    setStick(settleStick(stick));
  } catch (error) {
    setStick(settleStick(stick, message(error)));
  }
}

async function stopCopy() {
  stopCopyButton.disabled = true;
  stopCopyButton.textContent = t.stopping;
  await backend.stopDownload();
}

copyButton.addEventListener("click", copyToStick);
stopCopyButton.addEventListener("click", stopCopy);
chooseStick.addEventListener("click", async () => {
  const chosen = await backend.chooseFolder();
  if (chosen) {
    stickFolder = chosen;
    stickPath.textContent = chosen;
    stickPath.title = chosen;
  }
});
list.addEventListener("input", listEdited);
checkButton.addEventListener("click", check);
downloadButton.addEventListener("click", download);
stopButton.addEventListener("click", async () => {
  stopButton.disabled = true;
  stopButton.textContent = t.stopping;
  await backend.stopDownload();
});
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
