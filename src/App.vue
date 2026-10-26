<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { listen } from "@tauri-apps/api/event";
import { check } from "@tauri-apps/plugin-updater";
import { isTauri } from "@tauri-apps/api/core";
import {
  FolderOpen,
  ImagePlus,
  Languages,
  LoaderCircle,
  Plus,
  RotateCcw,
  Save,
  X,
} from "lucide-vue-next";
import {
  alignTracks,
  applyAlbumMetadata,
  chooseAlbumFolder,
  scanAlbumFolder,
  parseTracklist,
  suggestAlbumMetadata,
} from "./api/musicTags";
import { detectLocale, labels, localeNames, type Locale } from "./i18n";
import { isAppError } from "./api/tauriClient";
import type { MusicFile, TrackMetadata } from "./types";
import { useTrackEditor } from "./composables/useTrackEditor";
import { useCoverPreview } from "./composables/useCoverPreview";
import { useTracklistParser } from "./composables/useTracklistParser";
import TrackTable from "./components/TrackTable.vue";
import AlbumPreview from "./components/AlbumPreview.vue";
import StatusBar from "./components/StatusBar.vue";

type StatusKind = "idle" | "info" | "success" | "warning" | "error";

const locale = ref<Locale>(detectLocale());
const albumTitle = ref("");
const albumArtist = ref("");
const albumYear = ref("");
const folderPath = ref("");
const tracklistText = ref("");
const statusKind = ref<StatusKind>("idle");
const statusText = ref("");
const isBusy = ref(false);
const isCheckingUpdates = ref(false);
const isTauriApp = isTauri();
let trayUnlisten: (() => void) | null = null;
const renameFolder = ref(false);
let folderRequestId = 0;
const {
  files,
  tracks,
  pendingDeleteIndex,
  syncTargetFileNames,
  displayTargetFileName,
  handleTargetFileNameInput,
  handleTrackInput,
  addTrack,
  syncManualTracksFromFiles,
  removeTrack,
  requestRemoveTrack,
  cancelRemoveTrack,
  resetTrackEditor,
} = useTrackEditor(tracklistText);

const text = computed(() => labels[locale.value]);
const { showParsingStatus, resetParser } = useTracklistParser({
  tracklistText,
  tracks,
  files,
  syncManualTracksFromFiles,
  syncTargetFileNames,
  onError: (error) => setStatus("error", stringifyError(error)),
  onClearStatus: () => {
    statusKind.value = "idle";
    statusText.value = "";
  },
});
const {
  coverPath,
  coverPreview,
  isCoverPreviewOpen,
  isCoverLoading,
  pickCover,
  handleCoverSourceChange,
  openCoverPreview,
  closeCoverPreview,
  resetCoverPreview,
} = useCoverPreview((error) => setStatus("error", stringifyError(error)));
const languageOptions = computed(() =>
  (Object.keys(localeNames) as Locale[]).map((key) => ({
    key,
    label: localeNames[key],
  })),
);
async function checkForUpdates(silent = false) {
  if (!isTauriApp || isCheckingUpdates.value) return;
  isCheckingUpdates.value = true;
  try {
    const update = await check();
    if (!update) {
      if (!silent) setStatus("success", text.value.upToDate);
      return;
    }
    const accepted = window.confirm(`${text.value.updateAvailable} (${update.version})\n\n${update.body ?? ""}`);
    if (!accepted) {
      await update.close();
      return;
    }
    setStatus("info", text.value.downloadingUpdate);
    await update.downloadAndInstall();
  } catch (error) {
    if (!silent) {
      setStatus("error", `${text.value.updateCheckFailed}: ${error instanceof Error ? error.message : String(error)}`);
    }
  } finally {
    isCheckingUpdates.value = false;
  }
}

const validPairs = computed(() => {
  const pairs: { file: MusicFile; track: TrackMetadata; index: number }[] = [];
  const len = Math.min(files.value.length, tracks.value.length);
  for (let i = 0; i < len; i++) {
    const f = files.value[i];
    const t = tracks.value[i];
    if (f && f.path && t && t.number > 0 && t.title.trim()) {
      pairs.push({ file: f, track: t, index: i });
    }
  }
  return pairs;
});

const countMessage = computed(() => {
  const fileCount = files.value.filter((f) => f && f.path).length;
  const trackCount = tracks.value.filter((t) => t && t.title.trim()).length;
  return `${text.value.fileCount}: ${fileCount} / ${text.value.trackCount}: ${trackCount}`;
});
const countState = computed(() => {
  const fileCount = files.value.filter((f) => f && f.path).length;
  const trackCount = tracks.value.filter((t) => t && t.title.trim()).length;
  return fileCount > 0 && trackCount > 0 && fileCount === trackCount ? "ok" : "error";
});
const showCountMessage = computed(() => files.value.length > 0 || tracks.value.length > 0);
const displayStatusText = computed(() => statusText.value || (showCountMessage.value ? countMessage.value : ""));
const displayStatusKind = computed<StatusKind>(() => {
  if (statusText.value) return statusKind.value;
  return countState.value === "ok" ? "success" : "error";
});
const hasInvalidTracks = computed(() =>
  tracks.value.some(
    (track) =>
      track.number > 0 &&
      (!track.title.trim() || (!track.artist.trim() && !albumArtist.value.trim())),
  ),
);
const hasUnpairedTracks = computed(() =>
  tracks.value.some((track, index) => {
    if (!track || track.number === 0 || !track.title.trim()) return false;
    const file = files.value[index];
    return !file || !file.path;
  }),
);
const hasUnpairedFiles = computed(() =>
  files.value.some((file, index) => {
    if (!file || !file.path) return false;
    const track = tracks.value[index];
    return !track || track.number === 0 || !track.title.trim();
  }),
);
const hasInvalidTargetFiles = computed(() =>
  validPairs.value.some((p) => !displayTargetFileName(p.index).trim()),
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
function handleCoverPaste(event: ClipboardEvent) {
  const pasted = event.clipboardData?.getData("text")?.trim();
  if (
    pasted &&
    (pasted.startsWith("http://") ||
      pasted.startsWith("https://") ||
      pasted.startsWith("/") ||
      /^[a-zA-Z]:[\\/]/.test(pasted))
  ) {
    // We assign the clipboard text to v-model ourselves. Prevent the browser
    // from also inserting it at the caret, which would duplicate the URL.
    event.preventDefault();
    coverPath.value = pasted;
    handleCoverSourceChange();
  }
}

const hasAlbumPreview = computed(() =>
  Boolean(coverPreview.value || isCoverLoading.value || albumTitle.value.trim() || albumPreviewMeta.value),
);
onMounted(() => {
  if (!isTauriApp) return;
  void checkForUpdates(true);
  void listen("tray://check-for-updates", () => {
    void checkForUpdates();
  }).then((unlisten) => {
    trayUnlisten = unlisten;
  });
});
onBeforeUnmount(() => {
  trayUnlisten?.();
  trayUnlisten = null;
});
const canSubmit = computed(
  () =>
    Boolean(
      folderPath.value.trim() &&
        albumTitle.value.trim() &&
        !hasInvalidAlbumYear.value &&
        validPairs.value.length > 0 &&
        !hasInvalidTracks.value &&
        !hasUnpairedTracks.value &&
        !hasUnpairedFiles.value &&
        !hasInvalidTargetFiles.value &&
        !isBusy.value,
    ),
);

async function pickFolder() {
  const selected = await chooseAlbumFolder();
  if (!selected) return;

  // Selecting another album starts a fresh editing session. In particular,
  // never align the new files against the previous album's tracks or cover.
  resetWorkspace();
  folderPath.value = selected;
  await refreshFiles(selected, folderRequestId);
}

async function refreshFiles(
  selectedFolder = folderPath.value,
  requestId = folderRequestId,
) {
  if (!selectedFolder) return;

  isBusy.value = true;
  try {
    const scannedFiles = await scanAlbumFolder(selectedFolder);
    if (requestId !== folderRequestId || folderPath.value !== selectedFolder) return;

    const hasTracklist = Boolean(tracklistText.value.trim());
    let validTracks = tracks.value.filter((t) => t && t.title.trim());

    if (validTracks.length === 0 && hasTracklist) {
      try {
        const parsed = await parseTracklist(tracklistText.value.trim());
        if (parsed.length > 0) {
          validTracks = parsed;
        }
      } catch {
        // Ignore parsing errors here; parser handles reporting.
      }
    }

    if (validTracks.length > 0) {
      const aligned = await alignTracks(scannedFiles, validTracks);
      files.value = aligned.map((a) => a.file ?? { fileName: "-", path: "", artist: null });
      tracks.value = aligned.map((a) => a.track ?? { number: 0, title: "", artist: "" });
    } else {
      files.value = scannedFiles;
      syncManualTracksFromFiles();
    }

    syncTargetFileNames(true);
    await applyAlbumSuggestions(selectedFolder, requestId);
  } catch (error) {
    if (requestId !== folderRequestId || folderPath.value !== selectedFolder) return;

    files.value = [];
    syncManualTracksFromFiles();
    syncTargetFileNames(true);
    setStatus("error", stringifyError(error));
  } finally {
    if (requestId === folderRequestId) isBusy.value = false;
  }
}

async function applyAlbumSuggestions(selectedFolder: string, requestId: number) {
  try {
    const suggestion = await suggestAlbumMetadata(selectedFolder);
    if (requestId !== folderRequestId || folderPath.value !== selectedFolder) return;

    if (!albumTitle.value.trim() && suggestion.albumTitle) {
      albumTitle.value = suggestion.albumTitle;
    }

    if (!albumArtist.value.trim() && suggestion.albumArtist) {
      albumArtist.value = suggestion.albumArtist;
    }

    if (!albumYear.value.trim() && suggestion.albumYear) {
      albumYear.value = String(suggestion.albumYear);
    }

    if (!coverPath.value.trim() && suggestion.coverPath) {
      coverPath.value = suggestion.coverPath;
      await handleCoverSourceChange();
    }
  } catch {
    // Ignore metadata hints when the files do not expose readable album tags.
  }
}

function resetWorkspace() {
  folderRequestId += 1;
  // Invalidate any in-flight scan and release the UI lock immediately. A
  // stale request's finally block must not be responsible for clearing it.
  isBusy.value = false;
  albumTitle.value = "";
  albumArtist.value = "";
  albumYear.value = "";
  folderPath.value = "";
  resetCoverPreview();
  tracklistText.value = "";
  resetParser();
  resetTrackEditor();
  statusKind.value = "idle";
  statusText.value = "";
}

async function submit() {
  if (!canSubmit.value) {
    setStatus(
      "error",
      hasInvalidAlbumYear.value
        ? text.value.invalidYear
        : hasUnpairedTracks.value
          ? text.value.unpairedTracks
          : hasUnpairedFiles.value
            ? text.value.unpairedFiles
          : showCountMessage.value
            ? countMessage.value
            : text.value.emptyPreview,
    );
    return;
  }

  isBusy.value = true;
  setStatus("info", text.value.updating);

  try {
    const pairs = validPairs.value;
    const result = await applyAlbumMetadata({
      folderPath: folderPath.value,
      filePaths: pairs.map((p) => p.file.path),
      albumTitle: albumTitle.value,
      albumArtist: albumArtist.value,
      albumYear: albumYearNumber.value,
      coverPath: coverPath.value.trim(),
      renameFolder: renameFolder.value,
      tracks: pairs.map((p) => ({
        targetFileName: displayTargetFileName(p.index).trim(),
        number: p.track.number,
        title: p.track.title.trim(),
        artist: p.track.artist.trim(),
      })),
    });
    if (result.warning) {
      setStatus(
        "warning",
        `${text.value.success}: ${result.updatedCount} (${text.value.partialSuccess})`,
      );
    } else {
      setStatus("success", `${text.value.success}: ${result.updatedCount}`);
    }
    folderPath.value = result.folderPath;
    if (result.coverPath) {
      coverPath.value = result.coverPath;
    }
    files.value = result.files.map((file) => ({
      fileName: file.fileName,
      path: file.path,
      artist: file.artist,
    }));
    tracks.value = pairs.map((p, idx) => ({
      ...p.track,
      artist: result.files[idx]?.artist || p.track.artist,
      targetFileName: result.files[idx]?.fileName,
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
  const payload = isAppError(error) ? error : null;
  const message = payload?.message ?? (error instanceof Error ? error.message : String(error));
  switch (payload?.code) {
    case "tracklist_empty":
      return text.value.errorTracklistEmpty;
    case "tracks_undetected":
      return text.value.errorTracksUndetected;
    case "cover_missing":
      return text.value.errorCoverMissing;
    case "cover_type":
      return text.value.errorCoverType;
    case "cover_empty":
      return text.value.errorCoverEmpty;
    case "cover_too_large":
      return text.value.errorCoverTooLarge;
    case "cover_no_extension":
      return text.value.errorCoverNoExtension;
    case "cover_read":
      return text.value.errorCoverRead;
    case "cover_download":
      return text.value.errorCoverDownload;
    case "cover_copy":
      return text.value.errorCoverCopy;
    case "cover_invalid":
      return text.value.errorCoverInvalid;
  }

  // Keep compatibility with older native binaries that still return strings.
  if (message === "Tracklist is empty.") return text.value.errorTracklistEmpty;
  if (message === "Could not detect any tracks from the album content.") return text.value.errorTracksUndetected;
  if (message === "Selected cover image does not exist.") return text.value.errorCoverMissing;
  if (message === "Cover image must be JPG, PNG, or WEBP.") return text.value.errorCoverType;
  if (message === "Cover image is empty.") return text.value.errorCoverEmpty;
  if (message === "Cover image is too large.") return text.value.errorCoverTooLarge;
  if (message === "Cover image has no extension.") return text.value.errorCoverNoExtension;
  if (message.startsWith("Could not read cover image:")) return text.value.errorCoverRead;
  if (message.startsWith("Could not download cover image:") || message.startsWith("Could not read downloaded cover image:")) return text.value.errorCoverDownload;
  if (message.startsWith("Could not copy cover image into album folder:")) return text.value.errorCoverCopy;
  if (message.startsWith("Invalid cover image:")) return text.value.errorCoverInvalid;

  return message;
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
              <div class="inline-flex items-center gap-2">
                <span>{{ text.cover }}</span>
                <span v-if="isCoverLoading" class="subtle-status">
                  <LoaderCircle class="spin" :size="13" aria-hidden="true" />
                  {{ text.loadingCover }}
                </span>
              </div>
              <button class="icon-button" type="button" :disabled="isCoverLoading" @click="pickCover">
                <ImagePlus :size="18" aria-hidden="true" />
                <span>{{ text.chooseCover }}</span>
              </button>
            </div>
            <div class="relative mt-2.5">
              <input
                v-model="coverPath"
                class="text-input w-full"
                :class="{ 'pr-8': isCoverLoading }"
                type="text"
                :placeholder="text.coverSourcePlaceholder"
                @paste="handleCoverPaste"
                @keydown.enter.prevent="handleCoverSourceChange"
                @change="handleCoverSourceChange"
                @blur="handleCoverSourceChange"
              />
              <div
                v-if="isCoverLoading"
                class="pointer-events-none absolute right-2.5 top-1/2 -translate-y-1/2 text-accent-dark"
              >
                <LoaderCircle class="spin" :size="15" aria-hidden="true" />
              </div>
            </div>
          </section>

          <section class="picker-panel">
            <div class="panel-header">
              <span>{{ text.folder }}</span>
              <button class="icon-button" type="button" :disabled="isBusy" @click="pickFolder">
                <FolderOpen :size="18" aria-hidden="true" />
                <span>{{ text.chooseFolder }}</span>
              </button>
            </div>
            <p class="path-text">{{ folderPath || text.noFolder }}</p>
            <label class="checkbox-row">
              <input v-model="renameFolder" type="checkbox" :disabled="isBusy" />
              <span>{{ text.renameFolder }}</span>
            </label>
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

        <AlbumPreview
          :visible="hasAlbumPreview"
          :cover-preview="coverPreview"
          :is-loading-cover="isCoverLoading"
          :title="albumTitle"
          :meta="albumPreviewMeta"
          :preview-title="text.previewCover"
          :cover-alt="text.previewCover"
          :loading-cover-text="text.loadingCover"
          @open-cover="openCoverPreview"
        />

        <TrackTable
          :files="files"
          :tracks="tracks"
          :text="text"
          :pending-delete-index="pendingDeleteIndex"
          :display-target-file-name="displayTargetFileName"
          @target-file-name-input="(index, value) => handleTargetFileNameInput(index, value)"
          @track-input="(index, field, value) => handleTrackInput(index, field, value)"
          @request-remove="requestRemoveTrack"
          @remove="removeTrack"
          @cancel-remove="cancelRemoveTrack"
        />

        <StatusBar :kind="displayStatusKind" :message="displayStatusText" />
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
