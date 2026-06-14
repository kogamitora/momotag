use crate::audio::{apply_metadata, read_cover_preview, scan_music_files};
use crate::models::{
    ApplyMetadataRequest, ApplyMetadataResult, CoverPreviewImage, MusicFile, TrackMetadata,
};
use crate::parser::parse_tracklist_text;

#[tauri::command]
pub fn scan_album_folder(folder_path: String) -> Result<Vec<MusicFile>, String> {
    scan_music_files(&folder_path)
}

#[tauri::command]
pub fn parse_tracklist(tracklist_text: String) -> Result<Vec<TrackMetadata>, String> {
    parse_tracklist_text(&tracklist_text)
}

#[tauri::command]
pub fn load_cover_preview(cover_path: String) -> Result<CoverPreviewImage, String> {
    read_cover_preview(&cover_path)
}

#[tauri::command]
pub fn apply_album_metadata(request: ApplyMetadataRequest) -> Result<ApplyMetadataResult, String> {
    apply_metadata(
        &request.folder_path,
        &request.album_title,
        &request.album_artist,
        request.album_year,
        &request.cover_path,
        &request.tracks,
    )
}
