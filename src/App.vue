<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from "vue";
import {
  AlertTriangle,
  CheckCircle2,
  FolderOpen,
  ImagePlus,
  Languages,
  LoaderCircle,
  Plus,
  RotateCcw,
  Save,
  Trash2,
  X,
} from "lucide-vue-next";
import {
  applyAlbumMetadata,
  chooseAlbumFolder,
  chooseCoverImage,
  loadCoverPreview,
  parseTracklist,
  scanAlbumFolder,
} from "./api/musicTags";
import { detectLocale, labels, localeNames, type Locale } from "./i18n";
import type { MusicFile, TrackMetadata } from "./types";

type StatusKind = "idle" | "info" | "success" | "error";

const locale = ref<Locale>(detectLocale());
const albumTitle = ref("");
const albumArtist = ref("");
const albumYear = ref("");
const folderPath = ref("");
const coverPath = ref("");
const coverPreview = ref("");
const isCoverPreviewOpen = ref(false);
const tracklistText = ref("");
const files = ref<MusicFile[]>([]);
const tracks = ref<TrackMetadata[]>([]);
const targetFileNames = ref<string[]>([]);
const editedTargetFileNames = ref<boolean[]>([]);
const pendingDeleteIndex = ref<number | null>(null);
const statusKind = ref<StatusKind>("idle");
const statusText = ref("");
const isBusy = ref(false);
const isParsing = ref(false);
const showParsingStatus = ref(false);

let parseTimer: number | undefined;
let parsingStatusTimer: number | undefined;
let parseRequestId = 0;

const text = computed(() => labels[locale.value]);
const languageOptions = computed(() =>
  (Object.keys(localeNames) as Locale[]).map((key) => ({
    key,
    label: localeNames[key],
  })),
);
const pairedRows = computed(() => {
  const max = Math.max(files.value.length, tracks.value.length);
  return Array.from({ length: max }, (_, index) => ({
    file: files.value[index],
    track: tracks.value[index],
  }));
});
const countMessage = computed(() => {
  return `${text.value.fileCount}: ${files.value.length} / ${text.value.trackCount}: ${tracks.value.length}`;
});
const countState = computed(() =>
  files.value.length > 0 && tracks.value.length > 0 && files.value.length === tracks.value.length
    ? "ok"
    : "error",
);
const showCountMessage = computed(() => files.value.length > 0 || tracks.value.length > 0);
const displayStatusText = computed(() => statusText.value || (showCountMessage.value ? countMessage.value : ""));
const displayStatusKind = computed<StatusKind>(() => {
  if (statusText.value) return statusKind.value;
  return countState.value === "ok" ? "success" : "error";
});
const hasInvalidTracks = computed(() =>
  tracks.value.some((track) => !track.title.trim() || !track.artist.trim()),
);
const hasInvalidTargetFiles = computed(() =>
  tracks.value.some((_, index) => !displayTargetFileName(index).trim()),
);
const albumYearNumber = computed(() => {
  const trimmed = String(albumYear.value).trim();
  if (!trimmed) return null;
  if (!/^\d{4}$/.test(trimmed)) return null;

  const year = Number(trimmed);
  return Number.isInteger(year) && year >= 1000 && year <= 9999 ? year : null;
});
const hasInvalidAlbumYear = computed(
  () => Boolean(String(albumYear.value).trim()) && albumYearNumber.value === null,
);
const albumPreviewMeta = computed(() =>
  [albumArtist.value.trim(), String(albumYear.value).trim()].filter(Boolean).join(" · "),
);
const hasAlbumPreview = computed(() =>
  Boolean(coverPreview.value || albumTitle.value.trim() || albumPreviewMeta.value),
);
const canSubmit = computed(
  () =>
    Boolean(
      folderPath.value.trim() &&
        albumTitle.value.trim() &&
        !hasInvalidAlbumYear.value &&
        coverPath.value.trim() &&
        tracks.value.length > 0 &&
        !hasInvalidTracks.value &&
        !hasInvalidTargetFiles.value &&
        files.value.length === tracks.value.length &&
        !isBusy.value,
    ),
);

watch(tracklistText, () => {
  if (parseTimer) {
    window.clearTimeout(parseTimer);
  }

  parseTimer = window.setTimeout(() => {
    void refreshParsedTracks();
  }, 350);
});

watch(coverPreview, (preview) => {
  if (!preview) {
    isCoverPreviewOpen.value = false;
  }
});

onMounted(() => {
  window.addEventListener("keydown", handleWindowKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener("keydown", handleWindowKeydown);
});

async function pickFolder() {
  const selected = await chooseAlbumFolder();
  if (!selected) return;

  folderPath.value = selected;
  await refreshFiles();
}

async function pickCover() {
  const selected = await chooseCoverImage();
  if (!selected) return;

  coverPath.value = selected;
  await refreshCoverPreview(selected);
}

async function handleCoverSourceChange() {
  const source = coverPath.value.trim();
  if (!source) {
    if (coverPreview.value) {
      URL.revokeObjectURL(coverPreview.value);
    }
    coverPreview.value = "";
    return;
  }

  await refreshCoverPreview(source);
}

async function refreshFiles() {
  if (!folderPath.value) return;

  isBusy.value = true;
  try {
    files.value = await scanAlbumFolder(folderPath.value);
    syncManualTracksFromFiles();
    syncTargetFileNames(true);
  } catch (error) {
    files.value = [];
    syncManualTracksFromFiles();
    syncTargetFileNames(true);
    setStatus("error", stringifyError(error));
  } finally {
    isBusy.value = false;
  }
}

async function refreshParsedTracks() {
  const content = tracklistText.value.trim();
  if (!content) {
    parseRequestId += 1;
    clearParsingStatusTimer();
    isParsing.value = false;
    showParsingStatus.value = false;
    syncManualTracksFromFiles();
    syncTargetFileNames(true);
    statusKind.value = "idle";
    statusText.value = "";
    return;
  }

  const requestId = ++parseRequestId;
  isParsing.value = true;
  showParsingStatus.value = false;
  clearParsingStatusTimer();
  parsingStatusTimer = window.setTimeout(() => {
    if (isParsing.value && requestId === parseRequestId) {
      showParsingStatus.value = true;
    }
  }, 250);

  try {
    const parsedTracks = await parseTracklist(content);
    if (requestId !== parseRequestId) return;

    tracks.value = parsedTracks;
    syncTargetFileNames(true);
    statusKind.value = "idle";
    statusText.value = "";
  } catch (error) {
    if (requestId !== parseRequestId) return;

    tracks.value = [];
    syncTargetFileNames(true);
    setStatus("error", stringifyError(error));
  } finally {
    if (requestId === parseRequestId) {
      isParsing.value = false;
      showParsingStatus.value = false;
      clearParsingStatusTimer();
    }
  }
}

function clearParsingStatusTimer() {
  if (parsingStatusTimer) {
    window.clearTimeout(parsingStatusTimer);
    parsingStatusTimer = undefined;
  }
}

async function refreshCoverPreview(path: string) {
  try {
    const image = await loadCoverPreview(path);
    if (coverPreview.value) {
      URL.revokeObjectURL(coverPreview.value);
    }

    coverPreview.value = URL.createObjectURL(
      new Blob([new Uint8Array(image.data)], { type: image.mimeType }),
    );
  } catch (error) {
    coverPreview.value = "";
    setStatus("error", stringifyError(error));
  }
}

function openCoverPreview() {
  if (!coverPreview.value) return;
  isCoverPreviewOpen.value = true;
}

function closeCoverPreview() {
  isCoverPreviewOpen.value = false;
}

function handleWindowKeydown(event: KeyboardEvent) {
  if (event.key === "Escape") {
    closeCoverPreview();
  }
}

function resetWorkspace() {
  parseRequestId += 1;
  if (parseTimer) {
    window.clearTimeout(parseTimer);
  }
  clearParsingStatusTimer();
  if (coverPreview.value) {
    URL.revokeObjectURL(coverPreview.value);
  }

  albumTitle.value = "";
  albumArtist.value = "";
  albumYear.value = "";
  folderPath.value = "";
  coverPath.value = "";
  coverPreview.value = "";
  tracklistText.value = "";
  files.value = [];
  tracks.value = [];
  targetFileNames.value = [];
  editedTargetFileNames.value = [];
  pendingDeleteIndex.value = null;
  isCoverPreviewOpen.value = false;
  isParsing.value = false;
  showParsingStatus.value = false;
  statusKind.value = "idle";
  statusText.value = "";
}

async function submit() {
  if (!canSubmit.value) {
    setStatus(
      "error",
      hasInvalidAlbumYear.value
        ? text.value.invalidYear
        : showCountMessage.value
          ? countMessage.value
          : text.value.emptyPreview,
    );
    return;
  }

  isBusy.value = true;
  setStatus("info", text.value.updating);

  try {
    const result = await applyAlbumMetadata({
      folderPath: folderPath.value,
      albumTitle: albumTitle.value,
      albumArtist: albumArtist.value,
      albumYear: albumYearNumber.value,
      coverPath: coverPath.value,
      tracks: tracks.value.map((track, index) => ({
        targetFileName: displayTargetFileName(index).trim(),
        number: track.number,
        title: track.title.trim(),
        artist: track.artist.trim(),
      })),
    });
    setStatus("success", `${text.value.success}: ${result.updatedCount}`);
    folderPath.value = result.folderPath;
    files.value = result.files.map((file) => ({
      fileName: file.fileName,
      path: `${result.folderPath}\\${file.fileName}`,
    }));
    syncTargetFileNames(true);
  } catch (error) {
    setStatus("error", stringifyError(error));
  } finally {
    isBusy.value = false;
  }
}

function setStatus(kind: StatusKind, message: string) {
  statusKind.value = kind;
  statusText.value = message;
}

function stringifyError(error: unknown): string {
  const message = error instanceof Error ? error.message : String(error);
  if (message === "Tracklist is empty.") return text.value.errorTracklistEmpty;
  if (message === "Could not detect any tracks from the album content.") {
    return text.value.errorTracksUndetected;
  }
  if (message === "Selected cover image does not exist.") return text.value.errorCoverMissing;
  if (message === "Cover image must be JPG, PNG, or WEBP.") return text.value.errorCoverType;
  if (message === "Cover image is empty.") return text.value.errorCoverEmpty;
  if (message === "Cover image is too large.") return text.value.errorCoverTooLarge;
  if (message === "Cover image has no extension.") return text.value.errorCoverNoExtension;
  if (message.startsWith("Could not read cover image:")) return text.value.errorCoverRead;
  if (message.startsWith("Could not download cover image:")) return text.value.errorCoverDownload;
  if (message.startsWith("Could not read downloaded cover image:")) return text.value.errorCoverDownload;
  if (message.startsWith("Could not copy cover image into album folder:")) {
    return text.value.errorCoverCopy;
  }
  if (message.startsWith("Invalid cover image:")) return text.value.errorCoverInvalid;

  return message;
}

watch(
  [files, tracks],
  () => {
    syncTargetFileNames(false);
  },
  { deep: true },
);

function syncTargetFileNames(resetEdits: boolean) {
  const max = Math.max(files.value.length, tracks.value.length);
  const names = Array.from({ length: max }, (_, index) =>
    !resetEdits && editedTargetFileNames.value[index]
      ? targetFileNames.value[index] || ""
      : defaultTargetFileName(index),
  );

  targetFileNames.value = names;
  editedTargetFileNames.value = Array.from({ length: max }, (_, index) =>
    resetEdits ? false : Boolean(editedTargetFileNames.value[index]),
  );
}

function displayTargetFileName(index: number): string {
  if (editedTargetFileNames.value[index]) {
    return targetFileNames.value[index] ?? "";
  }

  return targetFileNames.value[index] || defaultTargetFileName(index);
}

function defaultTargetFileName(index: number): string {
  const file = files.value[index];
  const track = tracks.value[index];
  if (!file) return "-";
  if (!track) return file.fileName;

  const extension = file.fileName.split(".").pop() || "flac";
  return `${String(track.number).padStart(2, "0")} ${sanitizeFileName(track.title)}.${extension}`;
}

function handleTargetFileNameInput(index: number, event: Event) {
  targetFileNames.value[index] = (event.target as HTMLInputElement).value;
  editedTargetFileNames.value[index] = true;
}

function handleTrackInput(index: number, field: "title" | "artist", event: Event) {
  ensureTrackAt(index);
  tracks.value[index][field] = (event.target as HTMLInputElement).value;
  syncTargetFileNames(false);
}

function addTrack() {
  pendingDeleteIndex.value = null;
  const lastTrack = tracks.value[tracks.value.length - 1];
  tracks.value.push({
    number: lastTrack ? lastTrack.number + 1 : tracks.value.length + 1,
    title: "",
    artist: lastTrack?.artist || "",
  });
  syncTargetFileNames(false);
}

function syncManualTracksFromFiles() {
  if (tracklistText.value.trim()) return;

  tracks.value = files.value.map((file, index) => {
    const existing = tracks.value[index];
    return {
      number: index + 1,
      title: existing?.title ?? inferTitleFromFileName(file.fileName),
      artist: existing?.artist ?? "",
    };
  });
}

function ensureTrackAt(index: number) {
  while (tracks.value.length <= index) {
    tracks.value.push(createManualTrack(tracks.value.length));
  }
}

function createManualTrack(index: number) {
  return {
    number: index + 1,
    title: files.value[index] ? inferTitleFromFileName(files.value[index].fileName) : "",
    artist: "",
  };
}

function inferTitleFromFileName(fileName: string): string {
  const stem = fileName.replace(/\.[^.\\/]+$/, "").trim();
  const withoutNumber = stem
    .replace(/^\s*(?:tr(?:ack)?\.?\s*)?\d{1,3}[\s._-]+/i, "")
    .trim();

  return withoutNumber || stem;
}

function removeTrack(index: number) {
  if (!tracks.value[index]) return;

  tracks.value.splice(index, 1);
  targetFileNames.value.splice(index, 1);
  editedTargetFileNames.value.splice(index, 1);
  pendingDeleteIndex.value = null;
  syncTargetFileNames(false);
}

function requestRemoveTrack(index: number) {
  pendingDeleteIndex.value = pendingDeleteIndex.value === index ? null : index;
}

function cancelRemoveTrack() {
  pendingDeleteIndex.value = null;
}

function sanitizeFileName(title: string): string {
  const cleaned = title
    .replace(/[<>:"/\\|?*\x00-\x1F]/g, " ")
    .replace(/\s+/g, " ")
    .trim()
    .replace(/^[. ]+|[. ]+$/g, "");

  return cleaned || "Untitled";
}
</script>

<template>
  <main class="app-shell">
    <header class="topbar">
      <div>
        <h1>{{ text.appTitle }}</h1>
        <p>{{ text.appSubtitle }}</p>
      </div>
      <label class="language-select">
        <Languages :size="18" aria-hidden="true" />
        <span>{{ text.language }}</span>
        <select v-model="locale" aria-label="Language">
          <option v-for="option in languageOptions" :key="option.key" :value="option.key">
            {{ option.label }}
          </option>
        </select>
      </label>
    </header>

    <section class="workspace">
      <form class="editor" @submit.prevent="submit">
        <div class="field-group">
          <label class="field-label" for="album-title">{{ text.albumTitle }}</label>
          <input
            id="album-title"
            v-model="albumTitle"
            class="text-input"
            type="text"
            :placeholder="text.albumPlaceholder"
          />
        </div>

        <div class="metadata-row">
          <div class="field-group">
            <label class="field-label" for="album-artist">{{ text.albumArtist }}</label>
            <input
              id="album-artist"
              v-model="albumArtist"
              class="text-input"
              type="text"
              :placeholder="text.albumArtistPlaceholder"
            />
          </div>

          <div class="field-group year-field">
            <label class="field-label" for="album-year">{{ text.albumYear }}</label>
            <input
              id="album-year"
              v-model="albumYear"
              class="text-input"
              type="number"
              inputmode="numeric"
              min="1000"
              max="9999"
              step="1"
              :placeholder="text.albumYearPlaceholder"
            />
          </div>
        </div>

        <div class="picker-grid">
          <section class="picker-panel cover-panel">
            <div class="panel-header">
              <span>{{ text.cover }}</span>
              <button class="icon-button" type="button" @click="pickCover">
                <ImagePlus :size="18" aria-hidden="true" />
                <span>{{ text.chooseCover }}</span>
              </button>
            </div>
            <input
              v-model="coverPath"
              class="text-input cover-source-input"
              type="text"
              :placeholder="text.coverSourcePlaceholder"
              @change="handleCoverSourceChange"
              @blur="handleCoverSourceChange"
            />
          </section>

          <section class="picker-panel">
            <div class="panel-header">
              <span>{{ text.folder }}</span>
              <button class="icon-button" type="button" @click="pickFolder">
                <FolderOpen :size="18" aria-hidden="true" />
                <span>{{ text.chooseFolder }}</span>
              </button>
            </div>
            <p class="path-text">{{ folderPath || text.noFolder }}</p>
          </section>
        </div>

        <div class="field-group">
          <div class="label-row">
            <label class="field-label" for="tracklist">{{ text.tracklist }}</label>
            <span v-if="showParsingStatus" class="subtle-status">
              <LoaderCircle class="spin" :size="15" aria-hidden="true" />
              {{ text.parsing }}
            </span>
          </div>
          <textarea
            id="tracklist"
            v-model="tracklistText"
            class="tracklist-input"
            :placeholder="text.tracklistPlaceholder"
          />
        </div>

        <div class="form-actions">
          <button class="secondary-button" type="button" :disabled="isBusy" @click="resetWorkspace">
            <RotateCcw :size="18" aria-hidden="true" />
            <span>{{ text.reset }}</span>
          </button>
          <button class="submit-button" type="submit" :disabled="!canSubmit">
            <LoaderCircle v-if="isBusy" class="spin" :size="18" aria-hidden="true" />
            <Save v-else :size="18" aria-hidden="true" />
            <span>{{ text.submit }}</span>
          </button>
        </div>
      </form>

      <aside class="preview">
        <div class="preview-header">
          <div>
            <h2>{{ text.parsedTracks }}</h2>
            <p>{{ text.editHint }}</p>
          </div>
          <div class="preview-actions">
            <button class="tool-button" type="button" :title="text.addTrack" @click="addTrack">
              <Plus :size="17" aria-hidden="true" />
            </button>
          </div>
        </div>

        <section v-if="hasAlbumPreview" class="album-preview">
          <button
            v-if="coverPreview"
            class="preview-cover preview-cover-button"
            type="button"
            :title="text.previewCover"
            @click="openCoverPreview"
          >
            <img :src="coverPreview" alt="" />
          </button>
          <div v-else class="preview-cover">
            <ImagePlus :size="28" aria-hidden="true" />
          </div>
          <div>
            <h3 v-if="albumTitle.trim()">{{ albumTitle }}</h3>
            <p v-if="albumPreviewMeta" class="album-preview-meta">{{ albumPreviewMeta }}</p>
          </div>
        </section>

        <div v-if="pairedRows.length" class="table-wrap">
          <table>
            <thead>
              <tr>
                <th>{{ text.targetFile }}</th>
                <th>{{ text.number }}</th>
                <th>{{ text.title }}</th>
                <th>{{ text.artist }}</th>
                <th>{{ text.actions }}</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(row, index) in pairedRows" :key="index">
                <td class="file-name-cell">
                  <input
                    v-if="row.file"
                    class="table-input file-name-input"
                    :value="displayTargetFileName(index)"
                    :aria-label="text.targetFile"
                    @input="handleTargetFileNameInput(index, $event)"
                  />
                  <span v-else>-</span>
                </td>
                <td>{{ row.track?.number || "-" }}</td>
                <td>
                  <input
                    class="table-input"
                    :value="tracks[index]?.title ?? ''"
                    :aria-label="text.title"
                    @input="handleTrackInput(index, 'title', $event)"
                  />
                </td>
                <td>
                  <input
                    class="table-input"
                    :value="tracks[index]?.artist ?? ''"
                    :aria-label="text.artist"
                    @input="handleTrackInput(index, 'artist', $event)"
                  />
                </td>
                <td class="actions-cell">
                  <div
                    v-if="row.track"
                    class="delete-popover-wrap"
                    :class="{
                      'delete-popover-wrap-active': pendingDeleteIndex === index,
                      'delete-popover-wrap-below': index < 2,
                    }"
                  >
                    <button
                      class="row-action-button"
                      type="button"
                      :title="text.deleteTrack"
                      :aria-expanded="pendingDeleteIndex === index"
                      @click="requestRemoveTrack(index)"
                    >
                      <Trash2 :size="16" aria-hidden="true" />
                    </button>
                    <div
                      v-if="pendingDeleteIndex === index"
                      class="delete-popover"
                      :class="{ 'delete-popover-below': index < 2 }"
                    >
                      <span>{{ text.confirmDeleteTrack }}</span>
                      <div class="delete-popover-actions">
                        <button class="confirm-delete-button" type="button" @click="removeTrack(index)">
                          {{ text.deleteTrack }}
                        </button>
                        <button class="cancel-delete-button" type="button" @click="cancelRemoveTrack">
                          {{ text.cancel }}
                        </button>
                      </div>
                    </div>
                  </div>
                  <span v-else>-</span>
                </td>
              </tr>
            </tbody>
          </table>
        </div>
        <p v-else class="empty-preview">{{ text.emptyPreview }}</p>

        <div v-if="displayStatusText" class="status-box" :class="displayStatusKind">
          <AlertTriangle v-if="displayStatusKind === 'error'" :size="18" aria-hidden="true" />
          <CheckCircle2 v-else-if="displayStatusKind === 'success'" :size="18" aria-hidden="true" />
          <LoaderCircle v-else-if="displayStatusKind === 'info'" class="spin" :size="18" aria-hidden="true" />
          <span>{{ displayStatusText }}</span>
        </div>
      </aside>
    </section>
  </main>

  <Teleport to="body">
    <Transition name="cover-lightbox">
      <div
        v-if="isCoverPreviewOpen"
        class="cover-lightbox"
        role="dialog"
        aria-modal="true"
        @click.self="closeCoverPreview"
      >
        <button class="cover-lightbox-close" type="button" :title="text.closePreview" @click="closeCoverPreview">
          <X :size="22" aria-hidden="true" />
        </button>
        <img class="cover-lightbox-image" :src="coverPreview" :alt="text.previewCover" />
      </div>
    </Transition>
  </Teleport>
</template>

<style>
* {
  box-sizing: border-box;
}

body {
  margin: 0;
}

button,
input,
select,
textarea {
  font-family: inherit;
}
</style>

<style scoped>
.app-shell {
  min-height: 100vh;
  background: #eef3f6;
  color: #181b1f;
  padding: 28px;
  --content-width: 80vw;
}

.topbar {
  align-items: flex-start;
  display: flex;
  gap: 20px;
  justify-content: space-between;
  margin: 0 auto 24px;
  width: var(--content-width);
}

.topbar h1 {
  font-size: 30px;
  line-height: 1.15;
  margin: 0 0 8px;
}

.topbar p {
  color: #5f6772;
  margin: 0;
}

.language-select {
  align-items: center;
  background: #ffffff;
  border: 1px solid #cfd8de;
  border-radius: 8px;
  display: flex;
  flex-shrink: 0;
  gap: 8px;
  padding: 9px 10px;
}

.language-select span {
  color: #48505a;
  font-size: 14px;
}

.language-select select {
  background: transparent;
  border: 0;
  color: #181b1f;
  font: inherit;
  outline: none;
}

.workspace {
  display: grid;
  gap: 20px;
  grid-template-columns: minmax(320px, 0.7fr) minmax(680px, 1.3fr);
  margin: 0 auto;
  width: var(--content-width);
}

.editor,
.preview {
  background: #ffffff;
  border: 1px solid #cfd8de;
  border-radius: 8px;
  box-shadow: 0 14px 35px rgba(30, 43, 54, 0.09);
}

.editor {
  display: flex;
  flex-direction: column;
  gap: 18px;
  padding: 20px;
}

.field-group {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.metadata-row {
  display: grid;
  gap: 12px;
  grid-template-columns: minmax(0, 1fr) 150px;
}

.year-field {
  min-width: 0;
}

.field-label,
.panel-header span {
  color: #262a30;
  font-size: 14px;
  font-weight: 700;
}

.text-input,
.tracklist-input {
  background: #f7fafb;
  border: 1px solid #cfd8de;
  border-radius: 8px;
  color: #181b1f;
  font: inherit;
  outline: none;
  padding: 11px 12px;
  width: 100%;
}

.text-input:focus,
.tracklist-input:focus {
  border-color: #267a86;
  box-shadow: 0 0 0 3px rgba(38, 122, 134, 0.16);
}

.tracklist-input {
  min-height: 220px;
  resize: vertical;
}

.picker-grid {
  display: grid;
  gap: 12px;
  grid-template-columns: 1fr;
}

.picker-panel {
  background: #f7fafb;
  border: 1px solid #d7e0e5;
  border-radius: 8px;
  padding: 14px;
}

.panel-header,
.label-row,
.status-box {
  align-items: center;
  display: flex;
  gap: 10px;
}

.panel-header,
.label-row {
  justify-content: space-between;
}

.icon-button,
.secondary-button,
.submit-button {
  align-items: center;
  border: 0;
  border-radius: 8px;
  cursor: pointer;
  display: inline-flex;
  font: inherit;
  font-weight: 700;
  gap: 8px;
  justify-content: center;
}

.form-actions {
  display: grid;
  gap: 10px;
  grid-template-columns: minmax(112px, 0.36fr) minmax(180px, 1fr);
}

.icon-button {
  background: #e6f2f1;
  color: #165d68;
  min-height: 36px;
  padding: 8px 11px;
}

.icon-button:hover {
  background: #d9ebea;
}

.secondary-button {
  background: #eef2f4;
  color: #48515b;
  min-height: 44px;
  padding: 11px 14px;
}

.secondary-button:hover {
  background: #e3e9ec;
}

.secondary-button:disabled {
  cursor: not-allowed;
  opacity: 0.5;
}

.path-text {
  color: #68717d;
  font-size: 13px;
  line-height: 1.45;
  margin: 10px 0 0;
  overflow-wrap: anywhere;
}

.cover-source-input {
  margin-top: 10px;
}

.subtle-status {
  align-items: center;
  color: #68717d;
  display: inline-flex;
  font-size: 13px;
  gap: 6px;
}

.submit-button {
  background: #252a31;
  color: #ffffff;
  min-height: 44px;
  padding: 11px 16px;
}

.submit-button:disabled {
  cursor: not-allowed;
  opacity: 0.45;
}

.preview {
  display: flex;
  flex-direction: column;
  min-height: 520px;
  overflow: hidden;
}

.preview-header {
  align-items: center;
  border-bottom: 1px solid #d7e0e5;
  display: flex;
  gap: 14px;
  justify-content: space-between;
  padding: 18px 20px;
}

.preview-header h2 {
  font-size: 18px;
  line-height: 1.2;
  margin: 0 0 4px;
}

.preview-header p {
  color: #68717d;
  margin: 0;
}

.preview-actions {
  align-items: center;
  display: inline-flex;
  gap: 8px;
}

.tool-button,
.row-action-button {
  align-items: center;
  background: #eef5f6;
  border: 1px solid #cfdfe3;
  border-radius: 7px;
  color: #165d68;
  cursor: pointer;
  display: inline-flex;
  height: 32px;
  justify-content: center;
  width: 32px;
}

.tool-button:hover,
.row-action-button:hover {
  background: #e1eef0;
}

.row-action-button {
  background: #f6f8f9;
  border-color: #d9e1e5;
  color: #65717d;
}

.row-action-button:hover {
  background: #eef2f4;
  color: #a13b2a;
}

.delete-popover-wrap {
  display: inline-flex;
  justify-content: center;
  position: relative;
}

.delete-popover-wrap-active {
  z-index: 12;
}

.delete-popover {
  background: #ffffff;
  border: 1px solid #d9e1e5;
  border-radius: 8px;
  box-shadow: 0 12px 34px rgba(29, 36, 43, 0.16);
  color: #252a31;
  display: grid;
  gap: 10px;
  min-width: 176px;
  padding: 10px;
  position: absolute;
  bottom: calc(100% + 8px);
  right: 0;
  z-index: 6;
}

.delete-popover-below {
  bottom: auto;
  top: calc(100% + 8px);
}

.delete-popover::before {
  background: #ffffff;
  border-left: 1px solid #d9e1e5;
  border-top: 1px solid #d9e1e5;
  bottom: -5px;
  content: "";
  height: 9px;
  position: absolute;
  right: 12px;
  transform: rotate(225deg);
  width: 9px;
}

.delete-popover-below::before {
  bottom: auto;
  top: -5px;
  transform: rotate(45deg);
}

.delete-popover span {
  font-size: 13px;
  line-height: 1.4;
  text-align: left;
}

.delete-popover-actions {
  display: flex;
  gap: 8px;
  justify-content: flex-end;
}

.confirm-delete-button,
.cancel-delete-button {
  border: 0;
  border-radius: 6px;
  cursor: pointer;
  font: inherit;
  font-size: 13px;
  min-height: 30px;
  padding: 6px 10px;
}

.confirm-delete-button {
  background: #a13b2a;
  color: #ffffff;
}

.confirm-delete-button:hover {
  background: #8c3123;
}

.cancel-delete-button {
  background: #eef2f4;
  color: #48515b;
}

.cancel-delete-button:hover {
  background: #e3e9ec;
}

.album-preview {
  align-items: center;
  border-bottom: 1px solid #d7e0e5;
  display: grid;
  gap: 14px;
  grid-template-columns: 86px minmax(0, 1fr);
  padding: 16px 20px;
}

.preview-cover {
  align-items: center;
  aspect-ratio: 1;
  background: #e6f2f1;
  border: 1px solid #c7d9dc;
  border-radius: 8px;
  color: #165d68;
  display: flex;
  justify-content: center;
  overflow: hidden;
  width: 86px;
}

.preview-cover-button {
  cursor: zoom-in;
  padding: 0;
  transition:
    border-color 0.18s ease,
    transform 0.18s ease;
}

.preview-cover-button:hover {
  border-color: #267a86;
  transform: translateY(-1px);
}

.preview-cover-button:focus-visible {
  border-color: #267a86;
  outline: 3px solid rgba(38, 122, 134, 0.18);
  outline-offset: 2px;
}

.preview-cover img {
  height: 100%;
  object-fit: cover;
  transition: transform 0.18s ease;
  width: 100%;
}

.preview-cover-button:hover img {
  transform: scale(1.04);
}

.album-preview h3 {
  font-size: 18px;
  line-height: 1.25;
  margin: 0;
  overflow-wrap: anywhere;
}

.album-preview-meta {
  color: #68717d;
  font-size: 13px;
  line-height: 1.35;
  margin: 6px 0 0;
  overflow-wrap: anywhere;
}

.table-wrap {
  overflow: auto;
}

table {
  border-collapse: collapse;
  table-layout: fixed;
  width: 100%;
}

th:nth-child(1),
td:nth-child(1) {
  width: 31%;
}

th:nth-child(2),
td:nth-child(2) {
  width: 56px;
}

th:nth-child(3),
td:nth-child(3) {
  width: 29%;
}

th:nth-child(4),
td:nth-child(4) {
  width: calc(40% - 112px);
}

th:nth-child(5),
td:nth-child(5) {
  width: 56px;
}

th,
td {
  border-bottom: 1px solid #e1e8ec;
  font-size: 13px;
  line-height: 1.4;
  padding: 10px 12px;
  text-align: left;
  vertical-align: middle;
}

th {
  background: #f7fafb;
  color: #4b535e;
  font-weight: 700;
  position: sticky;
  top: 0;
  white-space: nowrap;
  z-index: 8;
}

td {
  color: #20252b;
  overflow-wrap: anywhere;
}

.table-input {
  background: #ffffff;
  border: 1px solid transparent;
  border-radius: 6px;
  color: #20252b;
  font: inherit;
  min-width: 0;
  outline: none;
  padding: 6px 7px;
  width: 100%;
}

.file-name-input {
  min-width: 0;
}

.actions-cell {
  text-align: center;
}

.table-input:focus {
  border-color: #267a86;
  box-shadow: 0 0 0 3px rgba(38, 122, 134, 0.12);
}

.empty-preview {
  color: #68717d;
  margin: 0;
  padding: 22px 20px;
}

.status-box {
  border-top: 1px solid #d7e0e5;
  margin-top: auto;
  padding: 14px 20px;
}

.status-box.success {
  color: #1d6a3b;
}

.status-box.error {
  color: #a13b2a;
}

.status-box.info {
  color: #165d68;
}

.spin {
  animation: spin 0.9s linear infinite;
}

.cover-lightbox {
  align-items: center;
  background: rgba(21, 25, 30, 0.82);
  display: flex;
  inset: 0;
  justify-content: center;
  padding: 48px;
  position: fixed;
  z-index: 20;
}

.cover-lightbox-enter-active,
.cover-lightbox-leave-active {
  transition: background-color 0.2s ease;
}

.cover-lightbox-enter-from,
.cover-lightbox-leave-to {
  background: rgba(21, 25, 30, 0);
}

.cover-lightbox-image {
  box-shadow: 0 24px 70px rgba(0, 0, 0, 0.35);
  max-height: min(82vh, 1100px);
  max-width: min(82vw, 1100px);
  object-fit: contain;
  transition:
    opacity 0.2s ease,
    transform 0.2s ease;
}

.cover-lightbox-enter-from .cover-lightbox-image,
.cover-lightbox-leave-to .cover-lightbox-image {
  opacity: 0;
  transform: scale(0.96);
}

.cover-lightbox-enter-to .cover-lightbox-image,
.cover-lightbox-leave-from .cover-lightbox-image {
  opacity: 1;
  transform: scale(1);
}

.cover-lightbox-close {
  align-items: center;
  background: rgba(255, 255, 255, 0.94);
  border: 1px solid rgba(255, 255, 255, 0.4);
  border-radius: 8px;
  color: #252a31;
  cursor: pointer;
  display: inline-flex;
  height: 42px;
  justify-content: center;
  position: fixed;
  right: 24px;
  top: 24px;
  transition:
    opacity 0.18s ease,
    transform 0.18s ease,
    background-color 0.18s ease;
  width: 42px;
}

.cover-lightbox-enter-from .cover-lightbox-close,
.cover-lightbox-leave-to .cover-lightbox-close {
  opacity: 0;
  transform: translateY(-4px);
}

.cover-lightbox-close:hover {
  background: #ffffff;
}

@keyframes spin {
  to {
    transform: rotate(360deg);
  }
}

@media (max-width: 1280px) {
  .app-shell {
    --content-width: 92vw;
  }
}

@media (max-width: 1100px) {
  .app-shell {
    padding: 18px;
    --content-width: 100%;
  }

  .topbar {
    flex-direction: column;
  }

  .workspace {
    grid-template-columns: 1fr;
  }

  .metadata-row {
    grid-template-columns: 1fr;
  }

  .preview {
    min-height: 360px;
  }

  .cover-lightbox {
    padding: 24px;
  }

  .cover-lightbox-image {
    max-height: 78vh;
    max-width: 88vw;
  }
}
</style>
