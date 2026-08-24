use crate::models::{AlbumMetadataSuggestion, MusicFile, TrackMetadata};
use lofty::config::WriteOptions;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::picture::Picture;
use lofty::prelude::Accessor;
use lofty::tag::items::Timestamp;
use lofty::tag::{ItemKey, Tag};

pub(crate) fn write_file_metadata(
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

pub(crate) fn suggest_album_metadata(
    files: &[MusicFile],
) -> Result<AlbumMetadataSuggestion, String> {
    let mut suggestion = AlbumMetadataSuggestion {
        album_title: None,
        album_artist: None,
        album_year: None,
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
