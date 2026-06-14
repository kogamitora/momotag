use serde::{Deserialize, Serialize};

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
