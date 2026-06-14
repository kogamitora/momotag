use crate::models::{
    ApplyMetadataResult, CoverPreviewImage, MusicFile, TrackMetadata, UpdatedFile,
};
use lofty::config::WriteOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::{Picture, PictureType};
use lofty::prelude::Accessor;
use lofty::tag::items::Timestamp;
use lofty::tag::{ItemKey, Tag};
use reqwest::header::CONTENT_TYPE;
use std::collections::HashSet;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::time::Duration;

const SUPPORTED_AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "wav"];
const SUPPORTED_COVER_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp"];
const MAX_COVER_BYTES: usize = 20 * 1024 * 1024;

struct CoverImageData {
    data: Vec<u8>,
    mime_type: String,
    extension: String,
}

pub fn scan_music_files(folder_path: &str) -> Result<Vec<MusicFile>, String> {
    let folder = Path::new(folder_path);
    if !folder.is_dir() {
        return Err("Selected album folder does not exist.".to_string());
    }

    let mut files = fs::read_dir(folder)
        .map_err(|err| format!("Could not read album folder: {err}"))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_supported_audio_file(path))
        .map(|path| MusicFile {
            file_name: path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| path.display().to_string()),
            path: path.display().to_string(),
        })
        .collect::<Vec<_>>();

    files.sort_by(|left, right| natural_music_order(left).cmp(&natural_music_order(right)));
    Ok(files)
}

pub fn apply_metadata(
    folder_path: &str,
    album_title: &str,
    album_artist: &str,
    album_year: Option<u16>,
    cover_path: &str,
    tracks: &[TrackMetadata],
) -> Result<ApplyMetadataResult, String> {
    let album_title = album_title.trim();
    let album_artist = album_artist.trim();
    if album_title.is_empty() {
        return Err("Album title is empty.".to_string());
    }

    let files = scan_music_files(folder_path)?;
    if files.len() != tracks.len() {
        return Err(format!(
            "Track count mismatch: found {} music files, but parsed {} tracks.",
            files.len(),
            tracks.len()
        ));
    }

    let cover_image = read_cover_image(cover_path)?;

    validate_tracks(tracks)?;
    let target_folder_path = build_album_folder_rename_path(folder_path, album_title)?;
    let rename_plan = build_rename_plan(folder_path, &files, tracks)?;
    let copied_cover_path = copy_cover_to_album_folder(folder_path, &cover_image)?;
    let picture = cover_picture(&cover_image.data)?;
    let total_tracks = tracks.len() as u32;
    let mut updated_files = Vec::with_capacity(files.len());

    for ((file, target_path), track) in rename_plan.iter().zip(tracks.iter()) {
        write_file_metadata(
            file,
            album_title,
            album_artist,
            album_year,
            track,
            total_tracks,
            picture.clone(),
        )
        .map_err(|err| format!("Failed to update {}: {err}", file.file_name))?;

        let original_path = Path::new(&file.path);
        if original_path != target_path {
            fs::rename(original_path, target_path)
                .map_err(|err| format!("Failed to rename {}: {err}", file.file_name))?;
        }

        updated_files.push(UpdatedFile {
            original_file_name: file.file_name.clone(),
            file_name: target_path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| target_path.display().to_string()),
            title: track.title.clone(),
            artist: track.artist.clone(),
        });
    }

    let final_cover_path = target_folder_path.join(
        copied_cover_path
            .file_name()
            .ok_or_else(|| "Copied cover path has no file name.".to_string())?,
    );
    rename_album_folder(folder_path, &target_folder_path)?;

    Ok(ApplyMetadataResult {
        updated_count: updated_files.len(),
        folder_path: target_folder_path.display().to_string(),
        cover_path: final_cover_path.display().to_string(),
        files: updated_files,
    })
}

fn write_file_metadata(
    file: &MusicFile,
    album_title: &str,
    album_artist: &str,
    album_year: Option<u16>,
    track: &TrackMetadata,
    total_tracks: u32,
    picture: Picture,
) -> Result<(), String> {
    let mut tagged_file =
        lofty::read_from_path(&file.path).map_err(|err| format!("could not read tags: {err}"))?;
    let tag_type = tagged_file.primary_tag_type();

    if tagged_file.primary_tag_mut().is_none() {
        tagged_file.insert_tag(Tag::new(tag_type));
    }

    let tag = tagged_file
        .primary_tag_mut()
        .ok_or_else(|| "could not create a writable primary tag".to_string())?;

    tag.set_album(album_title.to_string());
    if !album_artist.is_empty() {
        tag.insert_text(ItemKey::AlbumArtist, album_artist.to_string());
    }
    if let Some(year) = album_year {
        tag.set_date(Timestamp {
            year,
            month: None,
            day: None,
            hour: None,
            minute: None,
            second: None,
        });
    }
    tag.set_title(track.title.clone());
    tag.set_artist(track.artist.clone());
    tag.set_track(track.number);
    tag.set_track_total(total_tracks);

    while !tag.pictures().is_empty() {
        tag.remove_picture(0);
    }
    tag.push_picture(picture);

    tagged_file
        .save_to_path(&file.path, WriteOptions::default())
        .map_err(|err| format!("could not write tags: {err}"))
}

pub fn read_cover_preview(cover_path: &str) -> Result<CoverPreviewImage, String> {
    let cover_image = read_cover_image(cover_path)?;

    Ok(CoverPreviewImage {
        mime_type: cover_image.mime_type,
        data: cover_image.data,
    })
}

fn validate_tracks(tracks: &[TrackMetadata]) -> Result<(), String> {
    for track in tracks {
        if track.title.trim().is_empty() {
            return Err(format!("Track {:02} title is empty.", track.number));
        }

        if track.artist.trim().is_empty() {
            return Err(format!("Track {:02} artist is empty.", track.number));
        }
    }

    Ok(())
}

fn build_rename_plan(
    folder_path: &str,
    files: &[MusicFile],
    tracks: &[TrackMetadata],
) -> Result<Vec<(MusicFile, PathBuf)>, String> {
    let folder = Path::new(folder_path);
    let mut seen_targets = HashSet::new();
    let mut plan = Vec::with_capacity(files.len());

    for (file, track) in files.iter().zip(tracks.iter()) {
        let extension = Path::new(&file.file_name)
            .extension()
            .and_then(|value| value.to_str())
            .ok_or_else(|| format!("{} has no file extension.", file.file_name))?;
        let target_file_name = target_file_name(file, track, extension)?;
        let target_path = folder.join(target_file_name);
        let key = target_path.to_string_lossy().to_lowercase();

        if !seen_targets.insert(key) {
            return Err(format!(
                "Duplicate target file name for track {:02}.",
                track.number
            ));
        }

        let original_path = Path::new(&file.path);
        if target_path.exists() && original_path != target_path {
            return Err(format!(
                "Cannot rename {} because {} already exists.",
                file.file_name,
                target_path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| target_path.display().to_string())
            ));
        }

        plan.push((file.clone(), target_path));
    }

    Ok(plan)
}

fn target_file_name(
    file: &MusicFile,
    track: &TrackMetadata,
    extension: &str,
) -> Result<String, String> {
    let Some(custom_name) = track
        .target_file_name
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(format!(
            "{:02} {}.{}",
            track.number,
            sanitize_file_name(&track.title),
            extension
        ));
    };

    if custom_name.chars().any(|ch| {
        matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*') || ch.is_control()
    }) {
        return Err(format!(
            "{} contains characters that cannot be used in a file name.",
            custom_name
        ));
    }

    let trimmed = custom_name.trim_matches(['.', ' ']);
    if trimmed.is_empty() {
        return Err(format!("Target file name for {} is empty.", file.file_name));
    }

    if Path::new(trimmed).extension().is_some() {
        Ok(trimmed.to_string())
    } else {
        Ok(format!("{trimmed}.{extension}"))
    }
}

fn sanitize_file_name(title: &str) -> String {
    let sanitized = title
        .chars()
        .map(|ch| match ch {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => ' ',
            ch if ch.is_control() => ' ',
            ch => ch,
        })
        .collect::<String>();
    let collapsed = sanitized.split_whitespace().collect::<Vec<_>>().join(" ");
    let trimmed = collapsed.trim_matches(['.', ' ']).trim();

    if trimmed.is_empty() {
        "Untitled".to_string()
    } else {
        trimmed.chars().take(160).collect()
    }
}

fn build_album_folder_rename_path(folder_path: &str, album_title: &str) -> Result<PathBuf, String> {
    let folder = Path::new(folder_path);
    let parent = folder
        .parent()
        .ok_or_else(|| "Could not find the parent folder for the album folder.".to_string())?;
    let target_name = sanitize_file_name(album_title);
    let target = parent.join(target_name);

    if paths_equivalent(folder, &target) {
        return Ok(target);
    }

    if target.exists() {
        return Err(format!(
            "Cannot rename album folder because {} already exists.",
            target.display()
        ));
    }

    Ok(target)
}

fn rename_album_folder(folder_path: &str, target_folder_path: &Path) -> Result<(), String> {
    let folder = Path::new(folder_path);
    if paths_equivalent(folder, target_folder_path) {
        return Ok(());
    }

    fs::rename(folder, target_folder_path)
        .map_err(|err| format!("Failed to rename album folder: {err}"))
}

fn paths_equivalent(left: &Path, right: &Path) -> bool {
    if left == right {
        return true;
    }

    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left), Ok(right)) => left == right,
        _ => false,
    }
}

fn copy_cover_to_album_folder(
    folder_path: &str,
    cover_image: &CoverImageData,
) -> Result<PathBuf, String> {
    let target = Path::new(folder_path).join(format!("cover.{}", cover_image.extension));
    fs::write(&target, &cover_image.data)
        .map_err(|err| format!("Could not copy cover image into album folder: {err}"))?;
    Ok(target)
}

fn read_cover_image(cover_path: &str) -> Result<CoverImageData, String> {
    let source = cover_path.trim();
    if source.is_empty() {
        return Err("Selected cover image does not exist.".to_string());
    }

    if is_http_url(source) {
        return download_cover_image(source);
    }

    let path = Path::new(source);
    if !path.is_file() {
        return Err("Selected cover image does not exist.".to_string());
    }

    let mime_type = cover_mime_type_from_path(path)?;
    let extension = cover_extension_from_mime_type(&mime_type)
        .or_else(|| cover_extension_from_path(path))
        .ok_or_else(|| "Cover image must be JPG, PNG, or WEBP.".to_string())?;
    let data = fs::read(path).map_err(|err| format!("Could not read cover image: {err}"))?;
    validate_cover_size(data.len())?;

    Ok(CoverImageData {
        data,
        mime_type,
        extension,
    })
}

fn download_cover_image(url: &str) -> Result<CoverImageData, String> {
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("momotag-cover-loader")
        .build()
        .map_err(|err| format!("Could not prepare image download: {err}"))?
        .get(url)
        .send()
        .map_err(|err| format!("Could not download cover image: {err}"))?
        .error_for_status()
        .map_err(|err| format!("Could not download cover image: {err}"))?;

    let header_mime_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    let data = response
        .bytes()
        .map_err(|err| format!("Could not read downloaded cover image: {err}"))?
        .to_vec();
    validate_cover_size(data.len())?;

    let mime_type = header_mime_type
        .filter(|value| cover_extension_from_mime_type(value).is_some())
        .or_else(|| infer_cover_mime_type_from_bytes(&data))
        .or_else(|| {
            cover_extension_from_url(url)
                .map(|extension| cover_mime_type_from_extension(&extension).to_string())
                .filter(|value| !value.is_empty())
        })
        .ok_or_else(|| "Cover image must be JPG, PNG, or WEBP.".to_string())?;
    let extension = cover_extension_from_mime_type(&mime_type)
        .ok_or_else(|| "Cover image must be JPG, PNG, or WEBP.".to_string())?;

    Ok(CoverImageData {
        data,
        mime_type,
        extension,
    })
}

fn validate_cover_size(size: usize) -> Result<(), String> {
    if size == 0 {
        return Err("Cover image is empty.".to_string());
    }

    if size > MAX_COVER_BYTES {
        return Err("Cover image is too large.".to_string());
    }

    Ok(())
}

fn cover_mime_type_from_path(path: &Path) -> Result<String, String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| "Cover image has no extension.".to_string())?;

    let mime_type = cover_mime_type_from_extension(&extension);
    if mime_type.is_empty() {
        Err("Cover image must be JPG, PNG, or WEBP.".to_string())
    } else {
        Ok(mime_type.to_string())
    }
}

fn cover_mime_type_from_extension(extension: &str) -> &'static str {
    match extension {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        _ => "",
    }
}

fn cover_extension_from_mime_type(mime_type: &str) -> Option<String> {
    match mime_type {
        "image/jpeg" => Some("jpg".to_string()),
        "image/png" => Some("png".to_string()),
        "image/webp" => Some("webp".to_string()),
        _ => None,
    }
}

fn cover_extension_from_path(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| SUPPORTED_COVER_EXTENSIONS.contains(&value.as_str()))
}

fn cover_extension_from_url(url: &str) -> Option<String> {
    let without_fragment = url.split('#').next().unwrap_or(url);
    let without_query = without_fragment
        .split('?')
        .next()
        .unwrap_or(without_fragment);

    Path::new(without_query)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| SUPPORTED_COVER_EXTENSIONS.contains(&value.as_str()))
}

fn infer_cover_mime_type_from_bytes(data: &[u8]) -> Option<String> {
    if data.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("image/jpeg".to_string());
    }

    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png".to_string());
    }

    if data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        return Some("image/webp".to_string());
    }

    None
}

fn cover_picture(data: &[u8]) -> Result<Picture, String> {
    let mut cursor = Cursor::new(data);
    let mut picture =
        Picture::from_reader(&mut cursor).map_err(|err| format!("Invalid cover image: {err}"))?;
    picture.set_pic_type(PictureType::CoverFront);
    Ok(picture)
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("https://") || value.starts_with("http://")
}

fn is_supported_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|extension| {
            SUPPORTED_AUDIO_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        })
        .unwrap_or(false)
}

fn natural_music_order(file: &MusicFile) -> (u32, String) {
    let stem = Path::new(&file.file_name)
        .file_stem()
        .map(|value| value.to_string_lossy())
        .unwrap_or_default();

    let leading_number = stem
        .chars()
        .take_while(|ch| ch.is_ascii_digit())
        .collect::<String>()
        .parse::<u32>()
        .unwrap_or(u32::MAX);

    (leading_number, file.file_name.to_lowercase())
}
