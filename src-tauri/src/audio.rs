use crate::cover::{cover_picture, read_cover_image, CoverImageData};
use crate::filesystem::scan_music_files;
use crate::metadata::write_file_metadata;
use crate::models::{ApplyMetadataResult, MusicFile, TrackMetadata, UpdatedFile};
use crate::transaction::MutationJournal;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

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
    let folder = Path::new(folder_path.trim());
    if !folder.is_dir() {
        return Err("Selected album folder does not exist.".to_string());
    }
    if cover_path.trim().is_empty() {
        return Err("Selected cover image does not exist.".to_string());
    }
    if tracks.is_empty() {
        return Err("Tracklist is empty.".to_string());
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
    let mut journal = MutationJournal::begin(folder_path, &files)?;

    match apply_metadata_mutations(
        folder_path,
        album_title,
        album_artist,
        album_year,
        &cover_image,
        &target_folder_path,
        &rename_plan,
        tracks,
        &mut journal,
    ) {
        Ok(result) => {
            journal.commit();
            Ok(result)
        }
        Err(error) => {
            let rollback_error = journal.rollback();
            if let Some(rollback_error) = rollback_error {
                Err(format!("{error} (rollback incomplete: {rollback_error})"))
            } else {
                Err(error)
            }
        }
    }
}

fn apply_metadata_mutations(
    folder_path: &str,
    album_title: &str,
    album_artist: &str,
    album_year: Option<u16>,
    cover_image: &CoverImageData,
    target_folder_path: &Path,
    rename_plan: &[(MusicFile, PathBuf)],
    tracks: &[TrackMetadata],
    journal: &mut MutationJournal,
) -> Result<ApplyMetadataResult, String> {
    let cover_target = Path::new(folder_path).join(format!("cover.{}", cover_image.extension));
    let previous_cover = if cover_target.exists() {
        let backup = journal.backup_path("cover.previous");
        fs::copy(&cover_target, &backup)
            .map_err(|err| format!("Could not prepare existing cover backup: {err}"))?;
        Some(backup)
    } else {
        None
    };
    fs::write(&cover_target, &cover_image.data)
        .map_err(|err| format!("Could not copy cover image into album folder: {err}"))?;
    journal.record_cover(cover_target.clone(), previous_cover);

    let picture = cover_picture(&cover_image.data)?;
    let total_tracks = tracks.len() as u32;
    let mut updated_files = Vec::with_capacity(rename_plan.len());
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
        let original_path = Path::new(&file.path).to_path_buf();
        if original_path != *target_path {
            fs::rename(&original_path, target_path)
                .map_err(|err| format!("Failed to rename {}: {err}", file.file_name))?;
            journal.record_rename(original_path, target_path.clone());
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

    rename_album_folder(folder_path, target_folder_path)?;
    if !paths_equivalent(Path::new(folder_path), target_folder_path) {
        journal.record_folder_rename(target_folder_path);
    }
    let final_cover_path = target_folder_path.join(
        cover_target
            .file_name()
            .ok_or_else(|| "Copied cover path has no file name.".to_string())?,
    );
    Ok(ApplyMetadataResult {
        updated_count: updated_files.len(),
        folder_path: target_folder_path.display().to_string(),
        cover_path: final_cover_path.display().to_string(),
        files: updated_files,
    })
}

fn validate_tracks(tracks: &[TrackMetadata]) -> Result<(), String> {
    let mut numbers = HashSet::new();
    for track in tracks {
        if track.number == 0 || !numbers.insert(track.number) {
            return Err(format!(
                "Track number {:02} is duplicated or invalid.",
                track.number
            ));
        }
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
    if trimmed == "." || trimmed == ".." {
        return Err(format!(
            "Target file name for {} is invalid.",
            file.file_name
        ));
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

#[cfg(test)]
mod tests {
    use super::{sanitize_file_name, target_file_name, validate_tracks};
    use crate::models::{MusicFile, TrackMetadata};

    fn track(number: u32, title: &str) -> TrackMetadata {
        TrackMetadata {
            number,
            title: title.to_string(),
            artist: "Artist".to_string(),
            target_file_name: None,
        }
    }

    #[test]
    fn rejects_duplicate_track_numbers() {
        let tracks = vec![track(1, "One"), track(1, "Two")];
        assert!(validate_tracks(&tracks).is_err());
    }

    #[test]
    fn sanitizes_windows_file_name_characters() {
        assert_eq!(sanitize_file_name("  A<>:\"/B  "), "A B");
        assert_eq!(sanitize_file_name("..."), "Untitled");
    }

    #[test]
    fn appends_original_extension_to_custom_name() {
        let file = MusicFile {
            path: "C:\\album\\01.flac".to_string(),
            file_name: "01.flac".to_string(),
            artist: None,
        };
        let mut metadata = track(1, "One");
        metadata.target_file_name = Some("custom".to_string());
        assert_eq!(
            target_file_name(&file, &metadata, "flac").unwrap(),
            "custom.flac"
        );
    }
}
