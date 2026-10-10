use super::clean_value;
use super::credits::{parse_credit_line, select_artist_from_credits};
use super::normalize_lines;
use crate::models::TrackMetadata;
use regex::Regex;

pub(super) fn is_metadata_line(line: &str) -> bool {
    if parse_credit_line(line).is_some() {
        return true;
    }
    let trimmed = line.trim();
    let lower = trimmed.to_lowercase();
    lower.starts_with("from:")
        || lower.starts_with("from：")
        || lower.starts_with("lyrics")
        || lower.starts_with("music")
        || lower.starts_with("compose")
        || lower.starts_with("arrange")
}

pub(super) fn extract_artist_from_block(lines: &[&str]) -> Option<String> {
    // 1. Check for From: source lines
    for line in lines {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();
        if lower.starts_with("from:") || lower.starts_with("from：") {
            if let Some(slash_idx) = trimmed.find('/').or_else(|| trimmed.find('／')) {
                let artist = clean_value(&trimmed[slash_idx + 1..]);
                if !artist.is_empty() {
                    return Some(artist);
                }
            }
        }
    }

    // 2. Structured credits
    for line in lines {
        if let Some(parsed) = parse_credit_line(line) {
            if let Some(artist) = select_artist_from_credits(&parsed.credits) {
                return Some(artist);
            }
        }
    }

    None
}

pub(super) fn parse_line_tracks(text: &str) -> Vec<TrackMetadata> {
    let line_re = Regex::new(r"^\s*(\d{1,3})[\.\)\]:\s\t]*(.+?)\s*$").unwrap();
    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || super::is_heading(trimmed) {
                return None;
            }
            let captures = line_re.captures(trimmed)?;
            let number = captures.get(1)?.as_str().parse::<u32>().ok()?;
            let rest = captures.get(2)?.as_str();

            // First check if rest has an inline credit prefix like "Title - Music: ..."
            if let Some(parsed_credit) = parse_credit_line(rest) {
                if let Some(prefix_title) = parsed_credit.prefix {
                    let artist = select_artist_from_credits(&parsed_credit.credits).unwrap_or_default();
                    return Some(TrackMetadata {
                        number,
                        title: clean_value(&prefix_title),
                        artist,
                        target_file_name: None,
                    });
                }
            }

            super::parse_rest(rest, None).map(|(title, artist)| TrackMetadata {
                number,
                title,
                artist,
                target_file_name: None,
            })
        })
        .collect()
}

pub(super) fn parse_block_tracks(text: &str) -> Vec<TrackMetadata> {
    let lines = normalize_lines(text);
    let mut tracks = Vec::new();
    let mut index = 0;

    let has_any_metadata = lines.iter().any(|l| is_metadata_line(l));

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

        // If the document overall uses metadata blocks, accept any numbered track even if this specific track only has notes (e.g. 02 mochi)
        if has_any_metadata || block.iter().any(|line| is_metadata_line(line)) {
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
    let lines = normalize_lines(text);
    let artist_seed = infer_repeated_artist_seed_from_lines(&lines);
    let mut tracks = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let Some((number, inline_title)) = parse_number_line(lines[index]) else {
            index += 1;
            continue;
        };
        if let Some(title) = inline_title.as_deref() {
            if let Some((title, artist)) = super::parse_rest(title, artist_seed.as_deref()) {
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
                    if let Some((title, artist)) =
                        super::parse_rest(combined_line, artist_seed.as_deref())
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
    let lines = normalize_lines(text);
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

fn infer_repeated_artist_seed_from_lines(lines: &[&str]) -> Option<String> {
    let chunks = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| parse_number_line(line).is_none())
        .map(|(index, line)| (index as u32, (*line).to_string()))
        .collect::<Vec<_>>();
    super::infer_repeated_artist_seed(&chunks)
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
