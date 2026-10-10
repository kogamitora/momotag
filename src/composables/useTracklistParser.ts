import { ref, watch, type Ref } from "vue";
import { alignTracks, parseTracklist } from "../api/musicTags";
import type { MusicFile, TrackMetadata } from "../types";

interface TracklistParserOptions {
  tracklistText: Ref<string>;
  tracks: Ref<TrackMetadata[]>;
  files: Ref<MusicFile[]>;
  syncManualTracksFromFiles: () => void;
  syncTargetFileNames: (resetEdits: boolean) => void;
  onError: (error: unknown) => void;
  onClearStatus: () => void;
}

export function useTracklistParser(options: TracklistParserOptions) {
  const isParsing = ref(false);
  const showParsingStatus = ref(false);
  let parseTimer: number | undefined;
  let parsingStatusTimer: number | undefined;
  let parseRequestId = 0;

  watch(options.tracklistText, () => {
    if (parseTimer) window.clearTimeout(parseTimer);
    parseTimer = window.setTimeout(() => void refreshParsedTracks(), 350);
  });

  async function refreshParsedTracks() {
    const content = options.tracklistText.value.trim();
    if (!content) {
      cancelParsing();
      options.syncManualTracksFromFiles();
      options.syncTargetFileNames(true);
      options.onClearStatus();
      return;
    }

    const requestId = ++parseRequestId;
    isParsing.value = true;
    showParsingStatus.value = false;
    clearParsingStatusTimer();
    parsingStatusTimer = window.setTimeout(() => {
      if (isParsing.value && requestId === parseRequestId) showParsingStatus.value = true;
    }, 250);

    try {
      const parsedTracks = await parseTracklist(content);
      if (requestId !== parseRequestId) return;

      const validFiles = options.files.value.filter((f) => f && f.path);
      if (validFiles.length > 0) {
        const aligned = await alignTracks(validFiles, parsedTracks);
        options.files.value = aligned.map((a) => a.file ?? { fileName: "-", path: "", artist: null });
        options.tracks.value = aligned.map((a) => a.track ?? { number: 0, title: "", artist: "" });
      } else {
        options.tracks.value = parsedTracks;
      }

      options.syncTargetFileNames(true);
      options.onClearStatus();
    } catch (error) {
      if (requestId !== parseRequestId) return;
      options.tracks.value = [];
      options.syncTargetFileNames(true);
      options.onError(error);
    } finally {
      if (requestId === parseRequestId) {
        isParsing.value = false;
        showParsingStatus.value = false;
        clearParsingStatusTimer();
      }
    }
  }

  function cancelParsing() {
    parseRequestId += 1;
    isParsing.value = false;
    showParsingStatus.value = false;
    clearParsingStatusTimer();
  }

  function resetParser() {
    if (parseTimer) window.clearTimeout(parseTimer);
    parseTimer = undefined;
    cancelParsing();
  }

  function clearParsingStatusTimer() {
    if (parsingStatusTimer) window.clearTimeout(parsingStatusTimer);
    parsingStatusTimer = undefined;
  }

  return { isParsing, showParsingStatus, resetParser };
}
