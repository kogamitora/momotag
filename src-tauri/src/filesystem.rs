use crate::models::MusicFile;
use lofty::file::TaggedFileExt;
use lofty::prelude::Accessor;
use std::cmp::Ordering;
use std::fs;
use std::path::Path;

const SUPPORTED_AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "m4a", "ogg", "opus", "aiff", "aif", "ape", "wv",
];

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
        .map(|path| music_file_from_path(&path))
        .collect::<Vec<_>>();

    files.sort_by(|left, right| natural_cmp(&left.file_name, &right.file_name));
    Ok(files)
}

pub(crate) fn music_file_from_path(path: &Path) -> MusicFile {
    let artist = lofty::read_from_path(path).ok().and_then(|tagged_file| {
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
}

pub(crate) fn is_supported_audio_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|extension| {
            SUPPORTED_AUDIO_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
        })
        .unwrap_or(false)
}

/// Natural ordering: digit runs compare numerically, other runs compare
/// case-insensitively. "Track 2" < "Track 10", "1-02" < "1-10".
fn natural_cmp(left: &str, right: &str) -> Ordering {
    let mut a = left.chars().peekable();
    let mut b = right.chars().peekable();

    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return left.cmp(right),
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let na = take_digits(&mut a);
                let nb = take_digits(&mut b);
                let ta = na.trim_start_matches('0');
                let tb = nb.trim_start_matches('0');
                let ord = ta
                    .len()
                    .cmp(&tb.len())
                    .then_with(|| ta.cmp(tb))
                    .then_with(|| na.len().cmp(&nb.len()));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                let ord = x.to_lowercase().cmp(y.to_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                a.next();
                b.next();
            }
        }
    }
}

fn take_digits(iter: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    let mut digits = String::new();
    while let Some(&ch) = iter.peek() {
        if !ch.is_ascii_digit() {
            break;
        }
        digits.push(ch);
        iter.next();
    }
    digits
}

#[cfg(test)]
mod tests {
    use super::natural_cmp;

    #[test]
    fn sorts_embedded_numbers_naturally() {
        let mut names = vec!["Track 10.flac", "Track 2.flac", "Track 1.flac", "track 3.flac"];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            names,
            vec!["Track 1.flac", "Track 2.flac", "track 3.flac", "Track 10.flac"]
        );
    }

    #[test]
    fn sorts_zero_padded_and_multi_disc_names() {
        let mut names = vec!["2-01.flac", "1-10.flac", "1-02.flac", "10.flac", "9.flac"];
        names.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(
            names,
            vec!["1-02.flac", "1-10.flac", "2-01.flac", "9.flac", "10.flac"]
        );
    }
}
