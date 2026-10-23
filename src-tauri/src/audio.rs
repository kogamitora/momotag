use crate::cover::{cover_picture, read_cover_image, CoverImageData};
use crate::filesystem::{is_supported_audio_file, music_file_from_path, retry_io};
use crate::logger::{log_error, log_info, log_warn};
use crate::metadata::write_file_metadata;
use crate::models::{
    ApplyMetadataRequest, ApplyMetadataResult, MusicFile, TrackMetadata, UpdatedFile,
};
use crate::transaction::MutationJournal;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub fn apply_metadata(request: &ApplyMetadataRequest) -> Result<ApplyMetadataResult, String> {
    let folder_path = request.folder_path.trim();
    let album_title = request.album_title.trim();
    let album_artist = request.album_artist.trim();
    let tracks = request.tracks.as_slice();

    log_info(
        "audio",
        format!(
            "apply_metadata start: folder={folder_path}, album={album_title:?}, artist={album_artist:?}, year={:?}, files={}, tracks={}, cover={:?}, rename_folder={}",
            request.album_year,
            request.file_paths.len(),
            tracks.len(),
            request.cover_path.trim(),
            request.rename_folder
        ),
    );

    if album_title.is_empty() {
        return Err("Album title is empty.".to_string());
    }
    let folder = Path::new(folder_path);
    if !folder.is_dir() {
        return Err("Selected album folder does not exist.".to_string());
    }
    if tracks.is_empty() {
        return Err("Tracklist is empty.".to_string());
    }

    let files = resolve_requested_files(folder, &request.file_paths)?;
    if files.len() != tracks.len() {
        return Err(format!(
            "Track count mismatch: {} music files selected, but {} tracks provided.",
            files.len(),
            tracks.len()
        ));
    }

    let cover_source_path = request.cover_path.trim();
    let cover_image = match cover_source_path {
        "" => None,
        source => Some(read_cover_image(source)?),
    };
    validate_tracks(tracks, album_artist)?;
    let target_folder_path = if request.rename_folder {
        build_album_folder_rename_path(folder_path, album_title)?
    } else {
        folder.to_path_buf()
    };
    let rename_plan = build_rename_plan(folder_path, &files, tracks)?;
    let mut journal = MutationJournal::begin(folder_path, &files)?;

    let ctx = MutationContext {
        folder_path,
        album_title,
        album_artist,
        album_year: request.album_year,
        cover_source_path,
        cover_image: cover_image.as_ref(),
        target_folder_path: &target_folder_path,
    };

    match apply_metadata_mutations(&ctx, &rename_plan, tracks, &mut journal) {
        Ok(mut result) => {
            if let Some(cleanup_warning) = journal.commit() {
                log_warn("audio", &cleanup_warning);
                result.warning = Some(match result.warning.take() {
                    Some(existing) => format!("{existing} {cleanup_warning}"),
                    None => cleanup_warning,
                });
            }
            log_info(
                "audio",
                format!(
                    "apply_metadata completed: updated={}, folder={}, warning={:?}",
                    result.updated_count, result.folder_path, result.warning
                ),
            );
            Ok(result)
        }
        Err(error) => {
            log_error("audio", format!("apply_metadata failed: {error}"));
            let rollback_error = journal.rollback();
            if let Some(rollback_error) = rollback_error {
                log_error(
                    "audio",
                    format!("apply_metadata rollback incomplete: {rollback_error}"),
                );
                Err(format!("{error} (rollback incomplete: {rollback_error})"))
            } else {
                Err(error)
            }
        }
    }
}

/// Validate the file list sent by the editor: every entry must be an existing,
/// supported audio file located directly inside the album folder, with no
/// duplicates. Order is preserved because it defines the file/track pairing.
fn resolve_requested_files(folder: &Path, file_paths: &[String]) -> Result<Vec<MusicFile>, String> {
    if file_paths.is_empty() {
        return Err("No music files selected.".to_string());
    }

    let canonical_folder =
        fs::canonicalize(folder).map_err(|err| format!("Could not read album folder: {err}"))?;
    let mut seen = HashSet::new();
    let mut files = Vec::with_capacity(file_paths.len());

    for raw in file_paths {
        let path = Path::new(raw.trim());
        if !path.is_file() {
            return Err(format!("Music file no longer exists: {raw}"));
        }
        if !is_supported_audio_file(path) {
            return Err(format!("Unsupported audio file: {raw}"));
        }
        let canonical = fs::canonicalize(path)
            .map_err(|err| format!("Could not read music file {raw}: {err}"))?;
        if canonical.parent() != Some(canonical_folder.as_path()) {
            return Err(format!("Music file is not inside the album folder: {raw}"));
        }
        if !seen.insert(canonical.to_string_lossy().to_lowercase()) {
            return Err(format!("Music file is listed more than once: {raw}"));
        }
        files.push(music_file_from_path(
            &folder.join(canonical.file_name().unwrap_or_default()),
        ));
    }

    Ok(files)
}

struct MutationContext<'a> {
    folder_path: &'a str,
    album_title: &'a str,
    album_artist: &'a str,
    album_year: Option<u16>,
    cover_source_path: &'a str,
    cover_image: Option<&'a CoverImageData>,
    target_folder_path: &'a Path,
}

fn apply_metadata_mutations(
    ctx: &MutationContext<'_>,
    rename_plan: &[(MusicFile, PathBuf)],
    tracks: &[TrackMetadata],
    journal: &mut MutationJournal,
) -> Result<ApplyMetadataResult, String> {
    let mut cover_file_name = None;
    let mut picture = None;
    if let Some(cover_image) = ctx.cover_image {
        let source_path = Path::new(ctx.cover_source_path);
        let cover_target = cover_target_path(ctx.folder_path, source_path, &cover_image.extension);
        let is_same_cover_file =
            source_path.is_file() && paths_equivalent(source_path, &cover_target);
        if !is_same_cover_file {
            let previous_cover = if cover_target.exists() {
                let backup = journal.backup_path("cover.previous");
                retry_io(|| fs::copy(&cover_target, &backup))
                    .map_err(|err| format!("Could not prepare existing cover backup: {err}"))?;
                Some(backup)
            } else {
                None
            };
            // Register the mutation before writing: a failed write may still
            // have truncated or partially replaced the destination.
            journal.record_cover(cover_target.clone(), previous_cover);
            retry_io(|| fs::write(&cover_target, &cover_image.data))
                .map_err(|err| format!("Could not copy cover image into album folder: {err}"))?;
            log_info(
                "audio",
                format!(
                    "wrote folder cover {} ({}B)",
                    cover_target.display(),
                    cover_image.data.len()
                ),
            );
        } else {
            log_info(
                "audio",
                format!(
                    "cover source is already {}, skipping redundant self-overwrite",
                    cover_target.display()
                ),
            );
        }
        cover_file_name = cover_target.file_name().map(|name| name.to_os_string());
        picture = Some(cover_picture(&cover_image.data)?);
    }

    let total_tracks = tracks.len() as u32;
    let mut pending_files = Vec::with_capacity(rename_plan.len());
    for (index, ((file, target_path), track)) in rename_plan.iter().zip(tracks.iter()).enumerate() {
        journal.record_modified(index);
        write_file_metadata(
            file,
            ctx.album_title,
            ctx.album_artist,
            ctx.album_year,
            track,
            total_tracks,
            picture.clone(),
        )
        .map_err(|err| format!("Failed to update {}: {err}", file.file_name))?;
        let original_path = Path::new(&file.path).to_path_buf();
        if original_path != *target_path {
            retry_io(|| fs::rename(&original_path, target_path))
                .map_err(|err| format!("Failed to rename {}: {err}", file.file_name))?;
            journal.record_rename(original_path.clone(), target_path.clone());
            log_info(
                "audio",
                format!(
                    "renamed file {} -> {}",
                    original_path.display(),
                    target_path.display()
                ),
            );
        }
        let file_name = target_path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| target_path.display().to_string());
        let effective_artist = if track.artist.trim().is_empty() {
            ctx.album_artist.to_string()
        } else {
            track.artist.clone()
        };
        pending_files.push((
            file.file_name.clone(),
            file_name,
            track.title.clone(),
            effective_artist,
        ));
    }

    // Folder rename is non-destructive to the audio files: if the OS refuses to
    // rename the outer folder (e.g. open in Explorer or locked by another process),
    // keep the successfully updated audio files in the original folder and surface
    // a warning rather than rolling back all track mutations.
    let (actual_folder_path, warning) =
        match rename_album_folder(ctx.folder_path, ctx.target_folder_path) {
            Ok(()) => {
                if !paths_equivalent(Path::new(ctx.folder_path), ctx.target_folder_path) {
                    journal.record_folder_rename(ctx.target_folder_path);
                    log_info(
                        "audio",
                        format!(
                            "renamed album folder {} -> {}",
                            ctx.folder_path,
                            ctx.target_folder_path.display()
                        ),
                    );
                }
                (ctx.target_folder_path.to_path_buf(), None)
            }
            Err(err) => {
                log_warn(
                    "audio",
                    format!(
                        "album folder rename skipped (tracks kept intact in {}): {err}",
                        ctx.folder_path
                    ),
                );
                (Path::new(ctx.folder_path).to_path_buf(), Some(err))
            }
        };

    let updated_files = pending_files
        .into_iter()
        .map(|(original_file_name, file_name, title, artist)| UpdatedFile {
            original_file_name,
            path: actual_folder_path.join(&file_name).display().to_string(),
            file_name,
            title,
            artist,
        })
        .collect();

    Ok(ApplyMetadataResult {
        updated_count: rename_plan.len(),
        folder_path: actual_folder_path.display().to_string(),
        cover_path: cover_file_name
            .map(|name| actual_folder_path.join(name).display().to_string()),
        files: updated_files,
        warning,
    })
}

fn validate_tracks(tracks: &[TrackMetadata], album_artist: &str) -> Result<(), String> {
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

        if track.artist.trim().is_empty() && album_artist.trim().is_empty() {
            return Err(format!(
                "Track {:02} has no artist and album artist is empty.",
                track.number
            ));
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

    // Renaming must never masquerade as audio transcoding. Keep the source
    // format even when a caller supplies a different extension.
    let base_name = trimmed
        .rsplit_once('.')
        .filter(|(_, supplied_extension)| !supplied_extension.is_empty())
        .map_or(trimmed, |(base, _)| base)
        .trim_end_matches(['.', ' ']);
    if base_name.is_empty() {
        return Err(format!("Target file name for {} is empty.", file.file_name));
    }
    Ok(format!("{base_name}.{extension}"))
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
    if !paths_equivalent(folder, &target) && target.exists() {
        return Err(format!(
            "Cannot rename album folder because {} already exists.",
            target.display()
        ));
    }
    Ok(target)
}

fn cover_target_path(folder_path: &str, source_path: &Path, extension: &str) -> PathBuf {
    let folder = Path::new(folder_path);
    let source_is_in_album_folder = source_path.is_file()
        && source_path
            .parent()
            .is_some_and(|parent| paths_equivalent(parent, folder))
        && source_path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(is_named_cover_file);

    if source_is_in_album_folder {
        source_path.to_path_buf()
    } else {
        folder.join(format!("cover.{extension}"))
    }
}

fn is_named_cover_file(file_name: &str) -> bool {
    let mut parts = file_name.rsplitn(2, '.');
    let extension = parts.next().unwrap_or_default().to_ascii_lowercase();
    let stem = parts.next().unwrap_or_default().to_ascii_lowercase();
    matches!(stem.as_str(), "cover" | "folder" | "front")
        && matches!(extension.as_str(), "jpg" | "jpeg" | "png" | "webp")
}

fn rename_album_folder(folder_path: &str, target_folder_path: &Path) -> Result<(), String> {
    let folder = Path::new(folder_path);
    if paths_equivalent(folder, target_folder_path) {
        return Ok(());
    }

    if target_folder_path.exists() {
        return Err(format!(
            "Cannot rename album folder because {} already exists.",
            target_folder_path.display()
        ));
    }

    retry_io(|| fs::rename(folder, target_folder_path))
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
    use super::{
        cover_target_path, rename_album_folder, sanitize_file_name, target_file_name,
        validate_tracks,
    };
    use crate::models::{MusicFile, TrackMetadata};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

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
        assert!(validate_tracks(&tracks, "Artist").is_err());
    }

    #[test]
    fn allows_empty_track_artist_when_album_artist_present() {
        let mut t = track(1, "One");
        t.artist = String::new();
        assert!(validate_tracks(&[t.clone()], "Album Artist").is_ok());
        assert!(validate_tracks(&[t], "").is_err());
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

    #[test]
    fn custom_file_name_cannot_change_audio_extension() {
        let file = MusicFile {
            path: "C:\\album\\01.flac".to_string(),
            file_name: "01.flac".to_string(),
            artist: None,
        };
        let mut metadata = track(1, "One");
        metadata.target_file_name = Some("custom.mp3".to_string());
        assert_eq!(
            target_file_name(&file, &metadata, "flac").unwrap(),
            "custom.flac"
        );
    }

    #[test]
    fn keeps_existing_named_folder_cover_as_the_cover_target() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let folder = std::env::temp_dir().join(format!(".momotag-test-cover-{stamp}"));
        let source = folder.join("folder.jpeg");
        fs::create_dir_all(&folder).unwrap();
        fs::write(&source, b"cover").unwrap();

        assert_eq!(
            cover_target_path(&folder.display().to_string(), &source, "jpg"),
            source
        );

        let _ = fs::remove_dir_all(&folder);
    }

    #[test]
    fn rename_album_folder_fails_gracefully_when_target_exists() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base = std::env::temp_dir().join(format!(".momotag-test-rename-{stamp}"));
        let src = base.join("Unknown Album");
        let dst = base.join("Target Album");
        fs::create_dir_all(&src).unwrap();
        fs::create_dir_all(&dst).unwrap();

        let res = rename_album_folder(&src.display().to_string(), &dst);
        assert!(res.is_err());
        assert!(src.exists());

        let _ = fs::remove_dir_all(&base);
    }
}
