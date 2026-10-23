export interface AppError {
  code: string;
  message: string;
}

export interface MusicFile {
  path: string;
  fileName: string;
  artist: string | null;
}

export interface AlbumMetadataSuggestion {
  albumTitle: string | null;
  albumArtist: string | null;
  albumYear: number | null;
  coverPath?: string | null;
}

export interface TrackMetadata {
  number: number;
  title: string;
  artist: string;
  targetFileName?: string;
}

export interface ApplyMetadataRequest {
  folderPath: string;
  /** Exact files to write, paired by index with `tracks`. */
  filePaths: string[];
  albumTitle: string;
  albumArtist: string;
  albumYear?: number | null;
  /** Empty string keeps the existing embedded artwork. */
  coverPath: string;
  renameFolder: boolean;
  tracks: TrackMetadata[];
}

export interface UpdatedFile {
  originalFileName: string;
  fileName: string;
  path: string;
  title: string;
  artist: string;
}

export interface ApplyMetadataResult {
  updatedCount: number;
  folderPath: string;
  coverPath: string | null;
  files: UpdatedFile[];
  warning?: string | null;
}

export interface CoverPreviewImage {
  mimeType: string;
  data: number[];
}

export interface AlignedItem {
  file: MusicFile | null;
  track: TrackMetadata | null;
}
