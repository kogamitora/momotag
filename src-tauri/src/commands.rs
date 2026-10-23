use crate::audio::apply_metadata;
use crate::cover::read_cover_preview;
use crate::filesystem::scan_music_files;
use crate::logger::{log_error, log_info};
use crate::metadata::suggest_album_metadata as read_album_metadata_suggestion;
use crate::models::{
    AlbumMetadataSuggestion, AppError, ApplyMetadataRequest, ApplyMetadataResult,
    CoverPreviewImage, MusicFile, TrackMetadata,
};
use crate::parser::parse_tracklist_text;

#[tauri::command]
pub fn scan_album_folder(folder_path: String) -> Result<Vec<MusicFile>, AppError> {
    scan_music_files(&folder_path).map_err(|err| {
        log_error("cmd::scan_album_folder", format!("folder={folder_path}: {err}"));
        AppError::from_message(err)
    })
}

#[tauri::command]
pub fn suggest_album_metadata(folder_path: String) -> Result<AlbumMetadataSuggestion, AppError> {
    let files = scan_music_files(&folder_path).map_err(AppError::from_message)?;
    let mut suggestion = read_album_metadata_suggestion(&files).map_err(AppError::from_message)?;
    if suggestion.cover_path.is_none() {
        suggestion.cover_path = crate::filesystem::find_folder_cover(&folder_path);
    }
    log_info(
        "cmd::suggest_album_metadata",
        format!(
            "folder={folder_path} title={:?} artist={:?} year={:?} cover={:?}",
            suggestion.album_title,
            suggestion.album_artist,
            suggestion.album_year,
            suggestion.cover_path
        ),
    );
    Ok(suggestion)
}

#[tauri::command]
pub fn parse_tracklist(tracklist_text: String) -> Result<Vec<TrackMetadata>, AppError> {
    match parse_tracklist_text(&tracklist_text) {
        Ok(tracks) => {
            log_info(
                "cmd::parse_tracklist",
                format!(
                    "parsed {} tracks from {} chars",
                    tracks.len(),
                    tracklist_text.len()
                ),
            );
            Ok(tracks)
        }
        Err(err) => {
            let preview: String = tracklist_text.chars().take(120).collect();
            log_error(
                "cmd::parse_tracklist",
                format!("parse failed ({err}) for input snippet={preview:?}"),
            );
            Err(AppError::from_message(err))
        }
    }
}

#[tauri::command]
pub fn load_cover_preview(cover_path: String) -> Result<CoverPreviewImage, AppError> {
    read_cover_preview(&cover_path).map_err(|err| {
        log_error("cmd::load_cover_preview", format!("source={cover_path}: {err}"));
        AppError::from_message(err)
    })
}

#[tauri::command]
pub fn apply_album_metadata(
    request: ApplyMetadataRequest,
) -> Result<ApplyMetadataResult, AppError> {
    apply_metadata(&request).map_err(|err| {
        log_error("cmd::apply_album_metadata", &err);
        AppError::from_message(err)
    })
}

#[tauri::command]
pub fn align_tracks(
    files: Vec<MusicFile>,
    tracks: Vec<TrackMetadata>,
) -> Result<Vec<crate::models::AlignedItem>, AppError> {
    let aligned = crate::aligner::align_files_and_tracks(&files, &tracks);
    let paired = aligned
        .iter()
        .filter(|item| item.file.is_some() && item.track.is_some())
        .count();
    log_info(
        "cmd::align_tracks",
        format!(
            "aligned files={} tracks={} -> rows={} (paired={paired})",
            files.len(),
            tracks.len(),
            aligned.len()
        ),
    );
    Ok(aligned)
}
