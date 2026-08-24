use super::{
    clean_value, extract_artist_from_block, infer_repeated_artist_seed, is_heading,
    is_metadata_line, parse_rest,
};
use crate::models::TrackMetadata;
use regex::Regex;

pub(super) fn parse_line_tracks(text: &str) -> Vec<TrackMetadata> {
    let line_re = Regex::new(r"^\s*(\d{1,3})[\.\)\]:\s\t]*(.+?)\s*$").unwrap();
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || is_heading(trimmed) {
                return None;
            }
            let captures = line_re.captures(trimmed)?;
            let number = captures.get(1)?.as_str().parse::<u32>().ok()?;
            parse_rest(captures.get(2)?.as_str(), None).map(|(title, artist)| TrackMetadata {
                number,
                title,
                artist,
                target_file_name: None,
            })
        })
        .collect()
}

pub(super) fn parse_block_tracks(text: &str) -> Vec<TrackMetadata> {
    let lines = normalized_lines(text);
    let mut tracks = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let Some((number, inline_title)) = parse_number_line(lines[index]) else {
            index += 1;
            continue;
        };
        let next_index = lines[index + 1..]
            .iter()
            .position(|line| parse_number_line(line).is_some())
            .map(|offset| index + 1 + offset)
            .unwrap_or(lines.len());
        let block = &lines[index + 1..next_index];
        let title = inline_title.or_else(|| {
            block
                .iter()
                .find(|line| !is_metadata_line(line) && parse_number_line(line).is_none())
                .map(|line| clean_value(line))
        });
        if block.iter().any(|line| is_metadata_line(line)) {
            if let Some(title) = title.filter(|value| !value.is_empty()) {
                tracks.push(TrackMetadata {
                    number,
                    title,
                    artist: extract_artist_from_block(block).unwrap_or_default(),
                    target_file_name: None,
                });
            }
        }
        index = next_index;
    }
    tracks
}

pub(super) fn parse_stacked_tracks(text: &str) -> Vec<TrackMetadata> {
    let lines = normalized_lines(text);
    let artist_seed = infer_repeated_artist_seed_from_lines(&lines);
    let mut tracks = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let Some((number, inline_title)) = parse_number_line(lines[index]) else {
            index += 1;
            continue;
        };
        if let Some(title) = inline_title.as_deref() {
            if let Some((title, artist)) = parse_rest(title, artist_seed.as_deref()) {
                tracks.push(TrackMetadata {
                    number,
                    title,
                    artist,
                    target_file_name: None,
                });
                index += 1;
                continue;
            }
        }
        if inline_title.is_none() {
            if let Some(combined_line) = lines.get(index + 1) {
                if parse_number_line(combined_line).is_none() {
                    if let Some((title, artist)) = parse_rest(combined_line, artist_seed.as_deref())
                    {
                        tracks.push(TrackMetadata {
                            number,
                            title,
                            artist,
                            target_file_name: None,
                        });
                        index += 2;
                        continue;
                    }
                }
            }
        }
        let has_inline_title = inline_title.is_some();
        let title_index = if has_inline_title { index } else { index + 1 };
        let artist_index = title_index + 1;
        if artist_index >= lines.len() {
            break;
        }
        let title = inline_title.unwrap_or_else(|| lines[title_index].to_string());
        let artist = lines[artist_index].trim();
        if title.trim().is_empty()
            || artist.is_empty()
            || parse_number_line(artist).is_some()
            || (!has_inline_title && parse_number_line(lines[title_index]).is_some())
        {
            index += 1;
            continue;
        }
        tracks.push(TrackMetadata {
            number,
            title: clean_value(&title),
            artist: clean_value(artist),
            target_file_name: None,
        });
        index = artist_index + 1;
    }
    tracks
}

pub(super) fn parse_unnumbered_metadata_blocks(text: &str) -> Vec<TrackMetadata> {
    let lines = normalized_lines(text);
    let mut tracks = Vec::new();
    let mut index = 0;
    while index + 1 < lines.len() {
        let title = lines[index];
        if is_metadata_line(title) || parse_number_line(title).is_some() {
            index += 1;
            continue;
        }
        let mut metadata_end = index + 1;
        while metadata_end < lines.len() && is_metadata_line(lines[metadata_end]) {
            metadata_end += 1;
        }
        if metadata_end == index + 1 {
            index += 1;
            continue;
        }
        let metadata_lines = &lines[index + 1..metadata_end];
        tracks.push(TrackMetadata {
            number: tracks.len() as u32 + 1,
            title: clean_value(title),
            artist: extract_artist_from_block(metadata_lines).unwrap_or_default(),
            target_file_name: None,
        });
        index = metadata_end;
    }
    tracks
}

fn normalized_lines(text: &str) -> Vec<&str> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !is_heading(line))
        .collect()
}

fn infer_repeated_artist_seed_from_lines(lines: &[&str]) -> Option<String> {
    let chunks = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| parse_number_line(line).is_none())
        .map(|(index, line)| (index as u32, (*line).to_string()))
        .collect::<Vec<_>>();
    infer_repeated_artist_seed(&chunks)
}

fn parse_number_line(line: &str) -> Option<(u32, Option<String>)> {
    let number_re =
        Regex::new(r"(?i)^\s*(?:tr(?:ack)?\.?\s*)?(\d{1,3})(?:[\.\)\]:\-\s\t]+(.*?))?\s*$")
            .unwrap();
    let captures = number_re.captures(line.trim())?;
    let number = captures.get(1)?.as_str().parse::<u32>().ok()?;
    let title = captures
        .get(2)
        .map(|value| clean_value(value.as_str()))
        .filter(|value| !value.is_empty());
    Some((number, title))
}
