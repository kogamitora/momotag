use crate::models::TrackMetadata;
use regex::Regex;

pub fn parse_tracklist_text(input: &str) -> Result<Vec<TrackMetadata>, String> {
    let text = normalize_input(input);
    if text.trim().is_empty() {
        return Err("Tracklist is empty.".to_string());
    }

    let line_tracks = parse_line_tracks(&text);
    let block_tracks = parse_block_tracks(&text);
    let stacked_tracks = parse_stacked_tracks(&text);
    let unnumbered_block_tracks = parse_unnumbered_metadata_blocks(&text);
    let tracks = if line_tracks.len() > 1 {
        line_tracks
    } else if block_tracks.len() > 1 {
        block_tracks
    } else if stacked_tracks.len() > 1 {
        stacked_tracks
    } else if unnumbered_block_tracks.len() > 1 {
        unnumbered_block_tracks
    } else {
        parse_contiguous_tracks(&text)
    };

    if tracks.is_empty() {
        return Err("Could not detect any tracks from the album content.".to_string());
    }

    Ok(tracks)
}

fn normalize_input(input: &str) -> String {
    input
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\u{3000}', " ")
}

fn parse_line_tracks(text: &str) -> Vec<TrackMetadata> {
    let line_re = Regex::new(r"^\s*(\d{1,3})[\.\)\]:\s\t]*(.+?)\s*$").unwrap();

    text.lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || is_heading(trimmed) {
                return None;
            }

            let captures = line_re.captures(trimmed)?;
            let number = captures.get(1)?.as_str().parse::<u32>().ok()?;
            let rest = captures.get(2)?.as_str();
            parse_rest(rest, None).map(|(title, artist)| TrackMetadata {
                number,
                title,
                artist,
                target_file_name: None,
            })
        })
        .collect()
}

fn parse_block_tracks(text: &str) -> Vec<TrackMetadata> {
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !is_heading(line))
        .collect::<Vec<_>>();
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
        let has_metadata = block.iter().any(|line| is_metadata_line(line));
        let title = inline_title.or_else(|| {
            block
                .iter()
                .find(|line| !is_metadata_line(line) && parse_number_line(line).is_none())
                .map(|line| clean_value(line))
        });

        if has_metadata {
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

fn parse_stacked_tracks(text: &str) -> Vec<TrackMetadata> {
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !is_heading(line))
        .collect::<Vec<_>>();
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

fn parse_unnumbered_metadata_blocks(text: &str) -> Vec<TrackMetadata> {
    let lines = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !is_heading(line))
        .collect::<Vec<_>>();
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

    infer_repeated_artist_seed(&chunks)
}

fn parse_number_line(line: &str) -> Option<(u32, Option<String>)> {
    let number_re =
        Regex::new(r"(?i)^\s*(?:tr(?:ack)?\.?\s*)?(\d{1,3})(?:[\.\)\]:\-\s\t]+(.*?))?\s*$")
            .unwrap();
    let captures = number_re.captures(line)?;
    let number = captures.get(1)?.as_str().parse::<u32>().ok()?;
    let title = captures
        .get(2)
        .map(|value| clean_value(value.as_str()))
        .filter(|value| !value.is_empty());

    Some((number, title))
}

fn is_metadata_line(line: &str) -> bool {
    let normalized = line.trim().to_lowercase();
    let normalized = normalized.as_str();
    normalized.starts_with("lyrics")
        || normalized.starts_with("music")
        || normalized.starts_with("compose")
        || normalized.starts_with("arrange")
        || normalized.starts_with("from:")
        || normalized.starts_with("from：")
}

fn extract_artist_from_block(lines: &[&str]) -> Option<String> {
    let from_artist = lines.iter().find_map(|line| {
        let trimmed = line.trim();
        let lower = trimmed.to_lowercase();
        if !(lower.starts_with("from:") || lower.starts_with("from：")) {
            return None;
        }

        let slash_index = trimmed.find('/').or_else(|| trimmed.find('／'))?;
        let artist = clean_value(&trimmed[slash_index + 1..]);
        (!artist.is_empty()).then_some(artist)
    });

    from_artist.or_else(|| {
        lines
            .iter()
            .find(|line| is_metadata_line(line) && !line.to_lowercase().starts_with("from"))
            .and_then(|line| extract_credit_from_metadata_line(line))
    })
}

fn extract_credit_from_metadata_line(line: &str) -> Option<String> {
    extract_labeled_credit(line, &["lyrics", "lyric"])
        .or_else(|| extract_labeled_credit(line, &["music", "compose", "composition"]))
        .or_else(|| extract_labeled_credit(line, &["arrangement", "arrange"]))
        .or_else(|| {
            line.rsplit(|ch| ch == ':' || ch == '：')
                .next()
                .map(clean_value)
                .filter(|value| !value.is_empty())
        })
}

fn extract_labeled_credit(line: &str, labels: &[&str]) -> Option<String> {
    let lower = line.to_lowercase();
    let all_labels = [
        "lyrics",
        "lyric",
        "music",
        "compose",
        "composition",
        "arrangement",
        "arrange",
    ];

    labels.iter().find_map(|label| {
        let label_start = lower.find(label)?;
        let value_start = label_start + label.len();
        let value_end = all_labels
            .iter()
            .filter_map(|next_label| {
                lower[value_start..]
                    .find(next_label)
                    .map(|offset| value_start + offset)
            })
            .min()
            .unwrap_or(line.len());
        let value = line[value_start..value_end].trim_start_matches(|ch: char| {
            ch.is_whitespace() || ch == ':' || ch == '：' || ch == '&' || ch == '/'
        });
        let value = clean_value(value);

        (!value.is_empty()).then_some(value)
    })
}

fn parse_contiguous_tracks(text: &str) -> Vec<TrackMetadata> {
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

        let Some(start) = next_start else {
            break;
        };

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

fn parse_rest(rest: &str, artist_seed: Option<&str>) -> Option<(String, String)> {
    let normalized = rest.trim().trim_matches('-').trim();

    if let Some(parsed) = parse_slash_format(normalized) {
        return Some(parsed);
    }

    if let Some(parsed) = parse_metadata_suffix_format(normalized) {
        return Some(parsed);
    }

    if let Some(parsed) = parse_dash_format(normalized) {
        return Some(parsed);
    }

    if let Some(parsed) = parse_column_format(normalized) {
        return Some(parsed);
    }

    if let Some(seed) = artist_seed {
        if let Some(parsed) = parse_with_artist_seed(normalized, seed) {
            return Some(parsed);
        }
    }

    parse_with_artist_marker(normalized)
}

fn parse_slash_format(rest: &str) -> Option<(String, String)> {
    split_once_by(rest, &[" / ", " ／ ", "\t/\t", "\t／\t"])
        .map(|(title, artist)| (clean_value(title), clean_value(artist)))
}

fn parse_metadata_suffix_format(rest: &str) -> Option<(String, String)> {
    split_once_by(rest, &[" - ", " – ", " — ", "\t-\t"]).and_then(|(title, metadata)| {
        if !is_metadata_line(metadata) {
            return None;
        }

        extract_credit_from_metadata_line(metadata)
            .map(|artist| (clean_value(title), artist))
            .filter(|(title, artist)| !title.is_empty() && !artist.is_empty())
    })
}

fn parse_dash_format(rest: &str) -> Option<(String, String)> {
    split_once_by(rest, &["\t-\t", " - ", " – ", " — "])
        .map(|(artist, title)| (clean_value(title), clean_value(artist)))
}

fn parse_column_format(rest: &str) -> Option<(String, String)> {
    if rest.contains('\t') {
        let columns = rest
            .split('\t')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();

        if columns.len() >= 2 {
            return Some((
                clean_value(columns[0]),
                clean_value(&columns[1..].join(" ")),
            ));
        }
    }

    let spaced_columns = Regex::new(r"\s{2,}").unwrap();
    let columns = spaced_columns
        .split(rest)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    if columns.len() >= 2 && find_artist_marker_index(columns[1]).is_some() {
        return Some((
            clean_value(columns[0]),
            clean_value(&columns[1..].join(" ")),
        ));
    }

    None
}

fn split_once_by<'a>(text: &'a str, separators: &[&str]) -> Option<(&'a str, &'a str)> {
    separators.iter().find_map(|separator| {
        text.split_once(separator)
            .filter(|(left, right)| !left.trim().is_empty() && !right.trim().is_empty())
    })
}

fn parse_with_artist_seed(rest: &str, seed: &str) -> Option<(String, String)> {
    let index = rest.rfind(seed)?;
    let title = clean_value(&rest[..index]);
    let artist = clean_value(&rest[index..]);

    if title.is_empty() || artist.is_empty() {
        return None;
    }

    Some((title, artist))
}

fn parse_with_artist_marker(rest: &str) -> Option<(String, String)> {
    let marker_index = find_artist_marker_index(rest)?;

    let prefix = &rest[..marker_index];
    let artist_start =
        ascii_artist_boundary(prefix).or_else(|| prefix.rfind(' ').map(|i| i + 1))?;
    let title = clean_value(&rest[..artist_start]);
    let artist = clean_value(&rest[artist_start..]);

    if title.is_empty() || artist.is_empty() {
        return None;
    }

    Some((title, artist))
}

fn ascii_artist_boundary(text: &str) -> Option<usize> {
    let mut previous_ascii = false;
    let mut previous_index = 0;

    for (index, ch) in text.char_indices() {
        let current_ascii = ch.is_ascii_alphanumeric();
        if current_ascii && !previous_ascii && index > 0 {
            let previous = text[previous_index..index].chars().next()?;
            if !previous.is_ascii() {
                return Some(index);
            }
        }
        previous_ascii = current_ascii;
        previous_index = index;
    }

    None
}

fn infer_repeated_artist_seed(chunks: &[(u32, String)]) -> Option<String> {
    let prefixes = chunks
        .iter()
        .filter_map(|(_, chunk)| prefix_before_artist_marker(chunk))
        .collect::<Vec<_>>();

    if prefixes.len() < 2 {
        return None;
    }

    let suffix = common_suffix(&prefixes).trim().to_string();
    if suffix.chars().count() >= 2 {
        Some(suffix)
    } else {
        None
    }
}

fn prefix_before_artist_marker(text: &str) -> Option<String> {
    let index = find_artist_marker_index(text)?;

    Some(text[..index].to_string())
}

fn find_artist_marker_index(text: &str) -> Option<usize> {
    let lower = text.to_lowercase();
    [" feat.", " feat ", " ft.", " featuring ", " vocal", " vo."]
        .iter()
        .filter_map(|marker| lower.find(marker))
        .min()
}

fn common_suffix(values: &[String]) -> String {
    let mut reversed_suffix = Vec::new();
    let char_iters = values
        .iter()
        .map(|value| value.chars().rev().collect::<Vec<_>>())
        .collect::<Vec<_>>();

    if char_iters.is_empty() {
        return String::new();
    }

    let min_len = char_iters.iter().map(Vec::len).min().unwrap_or(0);
    for index in 0..min_len {
        let current = char_iters[0][index];
        if char_iters.iter().all(|chars| chars[index] == current) {
            reversed_suffix.push(current);
        } else {
            break;
        }
    }

    reversed_suffix.into_iter().rev().collect()
}

fn clean_value(value: &str) -> String {
    value
        .trim()
        .trim_matches('-')
        .trim_matches('/')
        .trim_matches('／')
        .trim()
        .to_string()
}

fn is_heading(line: &str) -> bool {
    let normalized = line.trim().to_lowercase();
    matches!(
        normalized.as_str(),
        "tracklist" | "track list" | "トラックリスト" | "曲目" | "曲目リスト" | "曲目列表"
    )
}

#[cfg(test)]
mod tests {
    use super::parse_tracklist_text;

    #[test]
    fn parses_number_title_artist_stacked_tracklist() {
        let input = "\
01
\u{30d2}\u{30df}\u{30c4}\u{306e}\u{30c6}\u{30ec}\u{30d1}\u{30b9}
irucaice feat. \u{7434}\u{8449}\u{831c}\u{30fb}\u{8475}
02
Twinkle Lights
irucaice feat. \u{7434}\u{8449}\u{831c}\u{30fb}\u{8475}
03
Moon Lights
irucaice feat. \u{7434}\u{8449}\u{831c}\u{30fb}\u{8475}";
        let tracks = parse_tracklist_text(input).unwrap();

        assert_eq!(tracks.len(), 3);
        assert_eq!(tracks[0].number, 1);
        assert_eq!(
            tracks[0].title,
            "\u{30d2}\u{30df}\u{30c4}\u{306e}\u{30c6}\u{30ec}\u{30d1}\u{30b9}"
        );
        assert_eq!(tracks[1].title, "Twinkle Lights");
        assert_eq!(
            tracks[2].artist,
            "irucaice feat. \u{7434}\u{8449}\u{831c}\u{30fb}\u{8475}"
        );
    }

    #[test]
    fn parses_contiguous_japanese_tracklist() {
        let input = "01ヒミツのテレパスirucaice feat. 琴葉茜・葵02Twinkle Lightsirucaice feat. 琴葉茜・葵03Moon Lightsirucaice feat. 琴葉茜・葵";
        let tracks = parse_tracklist_text(input).unwrap();

        assert_eq!(tracks.len(), 3);
        assert_eq!(tracks[0].title, "ヒミツのテレパス");
        assert_eq!(tracks[1].title, "Twinkle Lights");
        assert_eq!(tracks[2].artist, "irucaice feat. 琴葉茜・葵");
    }

    #[test]
    fn parses_artist_dash_title_tracklist() {
        let input = "TRACKLIST\n01\tirucaice feat. 琴葉茜・葵\t-\tヒミツのテレパス";
        let tracks = parse_tracklist_text(input).unwrap();

        assert_eq!(tracks[0].title, "ヒミツのテレパス");
        assert_eq!(tracks[0].artist, "irucaice feat. 琴葉茜・葵");
    }

    #[test]
    fn parses_title_slash_artist_tracklist() {
        let input = "トラックリスト\n01. ヒミツのテレパス / irucaice feat. 琴葉茜・葵";
        let tracks = parse_tracklist_text(input).unwrap();

        assert_eq!(tracks[0].title, "ヒミツのテレパス");
        assert_eq!(tracks[0].artist, "irucaice feat. 琴葉茜・葵");
    }

    #[test]
    fn parses_tabular_number_title_artist_tracklist() {
        let input = "\
01\tヒミツのテレパス\tirucaice feat. 琴葉茜・葵
02\tTwinkle Lights\tirucaice feat. 琴葉茜・葵
03\tMoon Lights\tirucaice feat. 琴葉茜・葵";
        let tracks = parse_tracklist_text(input).unwrap();

        assert_eq!(tracks.len(), 3);
        assert_eq!(tracks[1].number, 2);
        assert_eq!(tracks[1].title, "Twinkle Lights");
        assert_eq!(tracks[1].artist, "irucaice feat. 琴葉茜・葵");
    }

    #[test]
    fn parses_wide_spaced_number_title_artist_tracklist() {
        let input = "\
01  ヒミツのテレパス  irucaice feat. 琴葉茜・葵
02  Twinkle Lights  irucaice feat. 琴葉茜・葵";
        let tracks = parse_tracklist_text(input).unwrap();

        assert_eq!(tracks.len(), 2);
        assert_eq!(tracks[1].title, "Twinkle Lights");
        assert_eq!(tracks[1].artist, "irucaice feat. 琴葉茜・葵");
    }

    #[test]
    fn parses_number_then_combined_title_artist_lines() {
        let input = "\
1

メロン・メロディ・トロイメライirucaice feat.Hatsune Miku
2

Melty Cream Sodairucaice feat.Hatsune Miku
3

メロンクリーム・スプラッシュirucaice feat.Hatsune Miku
4

クロスド・レターirucaice feat.Hatsune Miku
5

Rainy Stepirucaice feat.Hatsune Miku
6

My Lucky Weekendirucaice feat.Hatsune Miku
7

Lucky☆Honey☆Pancakeirucaice feat.Hatsune Miku
8

Streaming Loveirucaice feat.Hatsune Miku
9

なないろハピネスirucaice feat.Hatsune Miku
10

Lovely Kitchen (3R2 Remix)irucaice feat.Hatsune Miku";
        let tracks = parse_tracklist_text(input).unwrap();

        assert_eq!(tracks.len(), 10);
        assert_eq!(tracks[0].title, "メロン・メロディ・トロイメライ");
        assert_eq!(tracks[1].number, 2);
        assert_eq!(tracks[1].title, "Melty Cream Soda");
        assert_eq!(tracks[1].artist, "irucaice feat.Hatsune Miku");
        assert_eq!(tracks[5].title, "My Lucky Weekend");
        assert_eq!(tracks[6].title, "Lucky☆Honey☆Pancake");
        assert_eq!(tracks[9].number, 10);
        assert_eq!(tracks[9].title, "Lovely Kitchen (3R2 Remix)");
    }

    #[test]
    fn parses_labeled_track_blocks_with_metadata_lines() {
        let input = "\
Tr.01
ヌイグルミーツ
Lyrics&Music:はるなば
From：Cunetry☆Century/pomme'tto
Tr.02
LOVE!1/8スケール
Lyrics&Music:はるなば
From：Colory Starry/ななひら
Tr.03
クラブこわい
Lyrics:七条レタス Music:D.watt
From：Apprism/pomme'tto
Tr.07
気づいたら春夏秋冬
Lyrics&Music:かめりあ
From：ばーさす！/かめりあ feat. ななひら";
        let tracks = parse_tracklist_text(input).unwrap();

        assert_eq!(tracks.len(), 4);
        assert_eq!(tracks[0].number, 1);
        assert_eq!(tracks[0].title, "ヌイグルミーツ");
        assert_eq!(tracks[0].artist, "pomme'tto");
        assert_eq!(tracks[1].title, "LOVE!1/8スケール");
        assert_eq!(tracks[1].artist, "ななひら");
        assert_eq!(tracks[3].number, 7);
        assert_eq!(tracks[3].title, "気づいたら春夏秋冬");
        assert_eq!(tracks[3].artist, "かめりあ feat. ななひら");
    }

    #[test]
    fn parses_dotted_number_blocks_with_music_metadata() {
        let input = "\
1.

はっぴーどりーむ☆りあらいず！

Music&Lyrics:Akki

2.

ミラクルキャンディファンシー

Music&Lyrics:5u5h1

3.

ラブりんりんぐ！

Music:雪乃イト Lyrics:C'Na

6.

魔法なんてない世界で

Music:&Lyrics:Akki";
        let tracks = parse_tracklist_text(input).unwrap();

        assert_eq!(tracks.len(), 4);
        assert_eq!(tracks[0].number, 1);
        assert_eq!(tracks[0].title, "はっぴーどりーむ☆りあらいず！");
        assert_eq!(tracks[0].artist, "Akki");
        assert_eq!(tracks[1].title, "ミラクルキャンディファンシー");
        assert_eq!(tracks[1].artist, "5u5h1");
        assert_eq!(tracks[2].title, "ラブりんりんぐ！");
        assert_eq!(tracks[2].artist, "C'Na");
        assert_eq!(tracks[3].number, 6);
        assert_eq!(tracks[3].title, "魔法なんてない世界で");
        assert_eq!(tracks[3].artist, "Akki");
    }

    #[test]
    fn parses_title_dash_music_credit_blocks() {
        let input = "\
Tr.01
Dear Next Song - Music&Lyrics: y0c1e
Tr.02
Girl meets manything - Music&Lyrics: U-ske
Tr.03
いっぱい食べる君が好きだよ - Music&Lyrics: ハム
Tr.04
TODO - Music: D.watt(IOSYS) Lyrics: 七条レタス(IOSYS)
Tr.05
Cheerful Days - Music: 塚越雄一朗(NanosizeMir) Lyrics: 植木直敬、らいね
Tr.06
生きんの大変！ - Music&Lyrics: Neko Hacker";
        let tracks = parse_tracklist_text(input).unwrap();

        assert_eq!(tracks.len(), 6);
        assert_eq!(tracks[0].title, "Dear Next Song");
        assert_eq!(tracks[0].artist, "y0c1e");
        assert_eq!(tracks[1].title, "Girl meets manything");
        assert_eq!(tracks[1].artist, "U-ske");
        assert_eq!(tracks[3].title, "TODO");
        assert_eq!(tracks[3].artist, "七条レタス(IOSYS)");
        assert_eq!(tracks[4].title, "Cheerful Days");
        assert_eq!(tracks[4].artist, "植木直敬、らいね");
        assert_eq!(tracks[5].title, "生きんの大変！");
        assert_eq!(tracks[5].artist, "Neko Hacker");
    }

    #[test]
    fn parses_unnumbered_title_then_music_metadata_blocks() {
        let input = "\
レンダスイッチ
Music&Lyrics:Kijibato
週末のDance Queen
Music:Ray_Oh　Lyrics:めがねこ
トゥルーエンドは君だけ feat.ひのせ
Music:Eno　Lyrics:カシラテ
新作のしあわせはこちら
Music:PandaBoy　Lyrics:畑　亜貴　Arrangement:Toccoyaki
ひといき
Music&Lyrics:x-x";
        let tracks = parse_tracklist_text(input).unwrap();

        assert_eq!(tracks.len(), 5);
        assert_eq!(tracks[0].number, 1);
        assert_eq!(tracks[0].title, "レンダスイッチ");
        assert_eq!(tracks[0].artist, "Kijibato");
        assert_eq!(tracks[1].title, "週末のDance Queen");
        assert_eq!(tracks[1].artist, "めがねこ");
        assert_eq!(tracks[2].title, "トゥルーエンドは君だけ feat.ひのせ");
        assert_eq!(tracks[2].artist, "カシラテ");
        assert_eq!(tracks[3].title, "新作のしあわせはこちら");
        assert_eq!(tracks[3].artist, "畑 亜貴");
        assert_eq!(tracks[4].title, "ひといき");
        assert_eq!(tracks[4].artist, "x-x");
    }
}
