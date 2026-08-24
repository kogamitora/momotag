use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
}

impl AppError {
    pub fn from_message(message: String) -> Self {
        let code = if message == "Tracklist is empty." {
            "tracklist_empty"
        } else if message == "Could not detect any tracks from the album content." {
            "tracks_undetected"
        } else if message == "Selected cover image does not exist." {
            "cover_missing"
        } else if message == "Cover image must be JPG, PNG, or WEBP." {
            "cover_type"
        } else if message == "Cover image is empty." {
            "cover_empty"
        } else if message == "Cover image is too large." {
            "cover_too_large"
        } else if message == "Cover image has no extension." {
            "cover_no_extension"
        } else if message.starts_with("Could not read cover image:") {
            "cover_read"
        } else if message.starts_with("Could not download cover image:")
            || message.starts_with("Could not read downloaded cover image:")
        {
            "cover_download"
        } else if message.starts_with("Could not copy cover image into album folder:") {
            "cover_copy"
        } else if message.starts_with("Invalid cover image:") {
            "cover_invalid"
        } else {
            "operation_failed"
        };

        Self {
            code: code.to_string(),
            message,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackMetadata {
    pub number: u32,
    pub title: String,
    pub artist: String,
    pub target_file_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicFile {
    pub path: String,
    pub file_name: String,
    pub artist: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlbumMetadataSuggestion {
    pub album_title: Option<String>,
    pub album_artist: Option<String>,
    pub album_year: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyMetadataRequest {
    pub folder_path: String,
    pub album_title: String,
    pub album_artist: String,
    pub album_year: Option<u16>,
    pub cover_path: String,
    pub tracks: Vec<TrackMetadata>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdatedFile {
    pub original_file_name: String,
    pub file_name: String,
    pub title: String,
    pub artist: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyMetadataResult {
    pub updated_count: usize,
    pub folder_path: String,
    pub cover_path: String,
    pub files: Vec<UpdatedFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoverPreviewImage {
    pub mime_type: String,
    pub data: Vec<u8>,
}
