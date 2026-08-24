import { open } from "@tauri-apps/plugin-dialog";
import type {
  AlbumMetadataSuggestion,
  ApplyMetadataRequest,
  ApplyMetadataResult,
  CoverPreviewImage,
  MusicFile,
  TrackMetadata,
} from "../types";
import { parseTracklistLocally } from "../utils/trackParser";
import { invokeCommand, isTauriUnavailableError } from "./tauriClient";

export async function chooseAlbumFolder(): Promise<string | null> {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Select album folder",
  });

  return typeof selected === "string" ? selected : null;
}

export async function chooseCoverImage(): Promise<string | null> {
  const selected = await open({
    directory: false,
    multiple: false,
    title: "Select album cover",
    filters: [
      {
        name: "Image",
        extensions: ["jpg", "jpeg", "png", "webp"],
      },
    ],
  });

  return typeof selected === "string" ? selected : null;
}

export function scanAlbumFolder(folderPath: string): Promise<MusicFile[]> {
  return invokeCommand("scan_album_folder", { folderPath });
}

export function suggestAlbumMetadata(
  folderPath: string,
): Promise<AlbumMetadataSuggestion> {
  return invokeCommand("suggest_album_metadata", { folderPath });
}

export function loadCoverPreview(
  coverPath: string,
): Promise<CoverPreviewImage> {
  return invokeCommand("load_cover_preview", { coverPath });
}

export function parseTracklist(tracklistText: string): Promise<TrackMetadata[]> {
  return invokeCommand<TrackMetadata[]>("parse_tracklist", { tracklistText }).catch((error) => {
    if (isTauriUnavailableError(error)) {
      return parseTracklistLocally(tracklistText);
    }

    throw error;
  });
}

export function applyAlbumMetadata(
  request: ApplyMetadataRequest,
): Promise<ApplyMetadataResult> {
  return invokeCommand("apply_album_metadata", { request });
}
