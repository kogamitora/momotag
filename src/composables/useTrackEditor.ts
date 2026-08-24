import { ref, watch, type Ref } from "vue";
import type { MusicFile, TrackMetadata } from "../types";
import { inferTitleFromFileName, sanitizeFileName } from "../utils/fileNames";

export function useTrackEditor(tracklistText: Ref<string>) {
  const files = ref<MusicFile[]>([]);
  const tracks = ref<TrackMetadata[]>([]);
  const targetFileNames = ref<string[]>([]);
  const editedTargetFileNames = ref<boolean[]>([]);
  const pendingDeleteIndex = ref<number | null>(null);

  watch([files, tracks], () => syncTargetFileNames(false), { deep: true });

  function syncTargetFileNames(resetEdits: boolean) {
    const max = Math.max(files.value.length, tracks.value.length);
    targetFileNames.value = Array.from({ length: max }, (_, index) =>
      !resetEdits && editedTargetFileNames.value[index]
        ? targetFileNames.value[index] || ""
        : defaultTargetFileName(index),
    );
    editedTargetFileNames.value = Array.from({ length: max }, (_, index) =>
      resetEdits ? false : Boolean(editedTargetFileNames.value[index]),
    );
  }

  function displayTargetFileName(index: number): string {
    if (editedTargetFileNames.value[index]) return targetFileNames.value[index] ?? "";
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

  function handleTargetFileNameInput(index: number, value: string) {
    targetFileNames.value[index] = value;
    editedTargetFileNames.value[index] = true;
  }

  function handleTrackInput(index: number, field: "title" | "artist", value: string) {
    ensureTrackAt(index);
    tracks.value[index][field] = value;
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
        artist: existing?.artist || file.artist || "",
      };
    });
  }

  function ensureTrackAt(index: number) {
    while (tracks.value.length <= index) tracks.value.push(createManualTrack(tracks.value.length));
  }

  function createManualTrack(index: number): TrackMetadata {
    return {
      number: index + 1,
      title: files.value[index] ? inferTitleFromFileName(files.value[index].fileName) : "",
      artist: "",
    };
  }

  function removeTrack(index: number) {
    if (!tracks.value[index]) return;

    // Remove the in-memory file/track pair together. This only excludes the
    // file from the current write batch; it never deletes the source file.
    if (files.value[index]) files.value.splice(index, 1);
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

  function resetTrackEditor() {
    files.value = [];
    tracks.value = [];
    targetFileNames.value = [];
    editedTargetFileNames.value = [];
    pendingDeleteIndex.value = null;
  }

  return {
    files,
    tracks,
    targetFileNames,
    editedTargetFileNames,
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
  };
}
