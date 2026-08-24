<script setup lang="ts">
import { computed, ref } from "vue";
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
  applyAlbumMetadata,
  chooseAlbumFolder,
  scanAlbumFolder,
  suggestAlbumMetadata,
} from "./api/musicTags";
import { detectLocale, labels, localeNames, type Locale } from "./i18n";
import { isAppError } from "./api/tauriClient";
import { useTrackEditor } from "./composables/useTrackEditor";
import { useCoverPreview } from "./composables/useCoverPreview";
import { useTracklistParser } from "./composables/useTracklistParser";
import TrackTable from "./components/TrackTable.vue";
import AlbumPreview from "./components/AlbumPreview.vue";
import StatusBar from "./components/StatusBar.vue";

type StatusKind = "idle" | "info" | "success" | "error";

const locale = ref<Locale>(detectLocale());
const albumTitle = ref("");
const albumArtist = ref("");
const albumYear = ref("");
const folderPath = ref("");
const tracklistText = ref("");
const statusKind = ref<StatusKind>("idle");
const statusText = ref("");
const isBusy = ref(false);
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

async function pickFolder() {
  const selected = await chooseAlbumFolder();
  if (!selected) return;

  // A folder is a new editing session. Do not carry over tracks, metadata,
  // cover state, or manually edited target names from the previous album.
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

    files.value = scannedFiles;
    syncManualTracksFromFiles();
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
  } catch {
    // Ignore metadata hints when the files do not expose readable album tags.
  }
}

function resetWorkspace() {
  folderRequestId += 1;
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
      artist: file.artist,
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
              <button class="icon-button" type="button" :disabled="isBusy" @click="pickFolder">
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

        <AlbumPreview
          :visible="hasAlbumPreview"
          :cover-preview="coverPreview"
          :title="albumTitle"
          :meta="albumPreviewMeta"
          :preview-title="text.previewCover"
          :cover-alt="text.previewCover"
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
