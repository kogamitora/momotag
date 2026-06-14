export interface MusicFile {
  path: string;
  fileName: string;
}

export interface TrackMetadata {
  number: number;
  title: string;
  artist: string;
  targetFileName?: string;
}

export interface ApplyMetadataRequest {
  folderPath: string;
  albumTitle: string;
  albumArtist: string;
  albumYear?: number | null;
  coverPath: string;
  tracks: TrackMetadata[];
}

export interface UpdatedFile {
  originalFileName: string;
  fileName: string;
  title: string;
  artist: string;
}

export interface ApplyMetadataResult {
  updatedCount: number;
  folderPath: string;
  coverPath: string;
  files: UpdatedFile[];
}

export interface CoverPreviewImage {
  mimeType: string;
  data: number[];
}
