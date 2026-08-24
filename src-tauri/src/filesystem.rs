use crate::models::MusicFile;
use lofty::file::TaggedFileExt;
use lofty::prelude::Accessor;
use std::fs;
use std::path::Path;

const SUPPORTED_AUDIO_EXTENSIONS: &[&str] = &["mp3", "flac", "wav"];

pub(crate) fn scan_music_files(folder_path: &str) -> Result<Vec<MusicFile>, String> {
    let folder = Path::new(folder_path);
    if !folder.is_dir() {
        return Err("Selected album folder does not exist.".to_string());
    }

    let mut files = fs::read_dir(folder)
        .map_err(|err| format!("Could not read album folder: {err}"))?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && is_supported_audio_file(path))
        .map(|path| {
            let artist = lofty::read_from_path(&path).ok().and_then(|tagged_file| {
                tagged_file
                    .primary_tag()
                    .and_then(|tag| tag.artist())
                    .map(|value| value.trim().to_string())
                    .filter(|value| !value.is_empty())
            });

            MusicFile {
                file_name: path
                    .file_name()
                    .map(|name| name.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.display().to_string()),
                path: path.display().to_string(),
                artist,
            }
        })
        .collect::<Vec<_>>();

    files.sort_by(|left, right| natural_music_order(left).cmp(&natural_music_order(right)));
    Ok(files)
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
