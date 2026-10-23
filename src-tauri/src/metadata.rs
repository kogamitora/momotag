use crate::filesystem::{is_transient_fs_error, retry_transient};
use crate::logger::{log_error, log_flac_diagnostics, log_info};
use crate::models::{AlbumMetadataSuggestion, MusicFile, TrackMetadata};
use lofty::config::WriteOptions;
use lofty::error::ErrorKind as LoftyErrorKind;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::Picture;
use lofty::prelude::Accessor;
use lofty::tag::items::Timestamp;
use lofty::tag::{ItemKey, Tag};
use std::path::Path;

pub(crate) fn write_file_metadata(
    file: &MusicFile,
    album_title: &str,
    album_artist: &str,
    album_year: Option<u16>,
    track: &TrackMetadata,
    total_tracks: u32,
    picture: Option<Picture>,
) -> Result<(), String> {
    let mut tagged_file = retry_transient(
        || lofty::read_from_path(&file.path),
        |err| matches!(err.kind(), LoftyErrorKind::Io(io_err) if is_transient_fs_error(io_err)),
    )
    .map_err(|err| {
        log_error(
            "metadata",
            format!("could not read tags from {}: {err}", file.path),
        );
        format!("could not read tags: {err}")
    })?;
    let tag_type = tagged_file.primary_tag_type();

    if tagged_file.primary_tag_mut().is_none() {
        tagged_file.insert_tag(Tag::new(tag_type));
    }

    let tag = tagged_file.primary_tag_mut().ok_or_else(|| {
        log_error(
            "metadata",
            format!("could not create writable primary tag for {}", file.path),
        );
        "could not create a writable primary tag".to_string()
    })?;

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
    let effective_artist = if track.artist.trim().is_empty() {
        album_artist
    } else {
        track.artist.as_str()
    };
    tag.set_artist(effective_artist.to_string());
    tag.set_track(track.number);
    tag.set_track_total(total_tracks);

    if let Some(picture) = picture {
        while !tag.pictures().is_empty() {
            tag.remove_picture(0);
        }
        tag.push_picture(picture);
    }

    retry_transient(
        || tagged_file.save_to_path(&file.path, WriteOptions::default()),
        |err| matches!(err.kind(), LoftyErrorKind::Io(io_err) if is_transient_fs_error(io_err)),
    )
    .map_err(|err| {
        log_error(
            "metadata",
            format!("could not write tags to {}: {err}", file.path),
        );
        format!("could not write tags: {err}")
    })?;

    log_info(
        "metadata",
        format!(
            "wrote tags to file={} (track={}/{total_tracks}, title={:?}, artist={:?}, tag_type={tag_type:?})",
            file.path, track.number, track.title, effective_artist
        ),
    );
    log_flac_diagnostics(Path::new(&file.path));

    Ok(())
}

pub(crate) fn suggest_album_metadata(
    files: &[MusicFile],
) -> Result<AlbumMetadataSuggestion, String> {
    let mut suggestion = AlbumMetadataSuggestion {
        album_title: None,
        album_artist: None,
        album_year: None,
        cover_path: None,
    };

    for file in files {
        let tagged_file = match lofty::read_from_path(&file.path) {
            Ok(tagged_file) => tagged_file,
            Err(_) => continue,
        };
        let Some(tag) = tagged_file.primary_tag() else {
            continue;
        };

        if suggestion.album_title.is_none() {
            suggestion.album_title = tag
                .album()
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
        }
        if suggestion.album_artist.is_none() {
            suggestion.album_artist = tag
                .get_string(ItemKey::AlbumArtist)
                .map(|value| value.to_string())
                .or_else(|| tag.artist().map(|value| value.into_owned()))
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty());
        }
        if suggestion.album_year.is_none() {
            suggestion.album_year = tag.date().map(|date| date.year);
        }
        if suggestion.album_title.is_some()
            && suggestion.album_artist.is_some()
            && suggestion.album_year.is_some()
        {
            break;
        }
    }

    Ok(suggestion)
}
