use crate::audio::apply_metadata;
use crate::cover::read_cover_preview;
use crate::filesystem::scan_music_files;
use crate::metadata::suggest_album_metadata as read_album_metadata_suggestion;
use crate::models::{
    AlbumMetadataSuggestion, AppError, ApplyMetadataRequest, ApplyMetadataResult,
    CoverPreviewImage, MusicFile, TrackMetadata,
};
use crate::parser::parse_tracklist_text;

#[tauri::command]
pub fn scan_album_folder(folder_path: String) -> Result<Vec<MusicFile>, AppError> {
    scan_music_files(&folder_path).map_err(AppError::from_message)
}

#[tauri::command]
pub fn suggest_album_metadata(folder_path: String) -> Result<AlbumMetadataSuggestion, AppError> {
    let files = scan_music_files(&folder_path).map_err(AppError::from_message)?;
    read_album_metadata_suggestion(&files).map_err(AppError::from_message)
}

#[tauri::command]
pub fn parse_tracklist(tracklist_text: String) -> Result<Vec<TrackMetadata>, AppError> {
    parse_tracklist_text(&tracklist_text).map_err(AppError::from_message)
}

#[tauri::command]
pub fn load_cover_preview(cover_path: String) -> Result<CoverPreviewImage, AppError> {
    read_cover_preview(&cover_path).map_err(AppError::from_message)
}

#[tauri::command]
pub fn apply_album_metadata(
    request: ApplyMetadataRequest,
) -> Result<ApplyMetadataResult, AppError> {
    apply_metadata(&request).map_err(AppError::from_message)
}

#[tauri::command]
pub fn align_tracks(
    files: Vec<MusicFile>,
    tracks: Vec<TrackMetadata>,
) -> Result<Vec<crate::models::AlignedItem>, AppError> {
    Ok(crate::aligner::align_files_and_tracks(&files, &tracks))
}
