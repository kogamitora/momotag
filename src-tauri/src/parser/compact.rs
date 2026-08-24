use super::{infer_repeated_artist_seed, is_heading, parse_rest};
use crate::models::TrackMetadata;

pub(super) fn parse_contiguous_tracks(text: &str) -> Vec<TrackMetadata> {
    let body = text
        .lines()
        .filter(|line| !is_heading(line.trim()))
        .collect::<Vec<_>>()
        .join(" ");
    let chunks = split_by_sequential_numbers(&body);
    let artist_seed = infer_repeated_artist_seed(&chunks);

    chunks
        .into_iter()
        .filter_map(|(number, rest)| {
            parse_rest(&rest, artist_seed.as_deref()).map(|(title, artist)| TrackMetadata {
                number,
                title,
                artist,
                target_file_name: None,
            })
        })
        .collect()
}

fn split_by_sequential_numbers(text: &str) -> Vec<(u32, String)> {
    let mut result = Vec::new();
    let mut expected = 1_u32;
    let Some(mut current_start) = find_track_number(text, expected, 0) else {
        return result;
    };

    loop {
        let current_width = number_width(text, current_start, expected);
        let content_start = current_start + current_width;
        let next_number = expected + 1;
        let next_start = find_track_number(text, next_number, content_start);
        let end = next_start.unwrap_or(text.len());
        let rest = text[content_start..end]
            .trim()
            .trim_start_matches(['.', ')', ':', '-', ' ', '\t'])
            .trim()
            .to_string();

        if !rest.is_empty() {
            result.push((expected, rest));
        }

        let Some(start) = next_start else { break };
        current_start = start;
        expected = next_number;
    }

    result
}

fn find_track_number(text: &str, number: u32, from: usize) -> Option<usize> {
    let padded = format!("{number:02}");
    text.get(from..)
        .and_then(|slice| slice.find(&padded).map(|offset| from + offset))
}

fn number_width(text: &str, start: usize, number: u32) -> usize {
    let padded = format!("{number:02}");
    if text[start..].starts_with(&padded) {
        padded.len()
    } else {
        number.to_string().len()
    }
}
