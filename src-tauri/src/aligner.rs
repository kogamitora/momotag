use crate::models::{AlignedItem, MusicFile, TrackMetadata};
use regex::Regex;

const GAP_PENALTY: i32 = -25;

/// Align physical files with parsed tracks using the Needleman-Wunsch sequence alignment algorithm.
/// Preserves sequential order, handles insertions/deletions (gaps), and scores number and title similarity.
pub fn align_files_and_tracks(
    files: &[MusicFile],
    tracks: &[TrackMetadata],
) -> Vec<AlignedItem> {
    let m = files.len();
    let n = tracks.len();

    if m == 0 && n == 0 {
        return Vec::new();
    }
    if m == 0 {
        return tracks
            .iter()
            .map(|t| AlignedItem {
                file: None,
                track: Some(t.clone()),
            })
            .collect();
    }
    if n == 0 {
        return files
            .iter()
            .map(|f| AlignedItem {
                file: Some(f.clone()),
                track: None,
            })
            .collect();
    }

    // dp[i][j] stores the best alignment score between files[..i] and tracks[..j]
    let mut dp = vec![vec![0i32; n + 1]; m + 1];

    for (i, row) in dp.iter_mut().enumerate().skip(1) {
        row[0] = i as i32 * GAP_PENALTY;
    }
    for (j, cell) in dp[0].iter_mut().enumerate().skip(1) {
        *cell = j as i32 * GAP_PENALTY;
    }

    // Needleman-Wunsch dynamic programming recurrence.
    // 1-based index loops are intentionally preserved here for clarity and safety,
    // as calculating cell (i, j) requires simultaneous immutable reads of previous
    // matrix cells (i-1, j-1), (i-1, j), and (i, j-1).
    for i in 1..=m {
        for j in 1..=n {
            let match_score = calculate_pair_score(&files[i - 1], &tracks[j - 1]);
            let diag = dp[i - 1][j - 1] + match_score;
            let up = dp[i - 1][j] + GAP_PENALTY;
            let left = dp[i][j - 1] + GAP_PENALTY;

            dp[i][j] = diag.max(up).max(left);
        }
    }

    // Traceback
    let mut i = m;
    let mut j = n;
    let mut aligned = Vec::new();

    while i > 0 || j > 0 {
        if i > 0 && j > 0 {
            let match_score = calculate_pair_score(&files[i - 1], &tracks[j - 1]);
            if dp[i][j] == dp[i - 1][j - 1] + match_score {
                aligned.push(AlignedItem {
                    file: Some(files[i - 1].clone()),
                    track: Some(tracks[j - 1].clone()),
                });
                i -= 1;
                j -= 1;
                continue;
            }
        }

        if i > 0 && dp[i][j] == dp[i - 1][j] + GAP_PENALTY {
            aligned.push(AlignedItem {
                file: Some(files[i - 1].clone()),
                track: None,
            });
            i -= 1;
        } else if j > 0 {
            aligned.push(AlignedItem {
                file: None,
                track: Some(tracks[j - 1].clone()),
            });
            j -= 1;
        } else {
            break;
        }
    }

    aligned.reverse();
    aligned
}

fn calculate_pair_score(file: &MusicFile, track: &TrackMetadata) -> i32 {
    let file_num = extract_file_track_number(&file.file_name);
    let title_sim = calculate_title_similarity(&file.file_name, &track.title);

    let num_score = match file_num {
        Some(num) if num == track.number => 60,
        Some(_) => -40,
        None => 0,
    };

    let title_score = if title_sim >= 0.3 {
        (title_sim * 60.0) as i32
    } else {
        -20
    };

    let total = num_score + title_score;

    // Critical guard: if there is no matching track number and title similarity is negligible,
    // the score must be strictly worse than two gaps (2 * GAP_PENALTY = -50).
    // Otherwise, Needleman-Wunsch falsely pairs unrelated items instead of inserting gaps.
    if file_num != Some(track.number) && title_sim < 0.25 {
        return (-60).min(total);
    }

    total
}

fn extract_file_track_number(file_name: &str) -> Option<u32> {
    let stem = std::path::Path::new(file_name)
        .file_stem()?
        .to_str()?;

    let re = Regex::new(
        r"(?i)(?:^|[\s._\-\(\[])(?:tr(?:ack)?\.?\s*)?0*(\d{1,3})(?:[\s._\-\]\)]|$)",
    )
    .ok()?;

    for cap in re.captures_iter(stem) {
        if let Some(m) = cap.get(1) {
            if let Ok(num) = m.as_str().parse::<u32>() {
                return Some(num);
            }
        }
    }
    None
}

fn calculate_title_similarity(file_name: &str, track_title: &str) -> f32 {
    let stem = std::path::Path::new(file_name)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(file_name);

    let clean_f = normalize_string(stem);
    let clean_t = normalize_string(track_title);

    if clean_f.is_empty() || clean_t.is_empty() {
        return 0.0;
    }

    if clean_f.contains(&clean_t) || clean_t.contains(&clean_f) {
        return 1.0;
    }

    // Also check Romaji transliteration
    let romaji_f = kana_to_romaji(&clean_f);
    let romaji_t = kana_to_romaji(&clean_t);

    if romaji_f.contains(&romaji_t) || romaji_t.contains(&romaji_f) {
        return 0.9;
    }

    // Bigram Dice coefficient
    dice_coefficient(&romaji_f, &romaji_t)
}

fn normalize_string(s: &str) -> String {
    s.chars()
        .filter(|ch| ch.is_alphanumeric() || is_cjk(*ch))
        .collect::<String>()
        .to_lowercase()
}

fn is_cjk(ch: char) -> bool {
    matches!(ch, '\u{4E00}'..='\u{9FFF}' | '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}')
}

fn dice_coefficient(a: &str, b: &str) -> f32 {
    let chars_a: Vec<char> = a.chars().collect();
    let chars_b: Vec<char> = b.chars().collect();

    if chars_a.len() < 2 || chars_b.len() < 2 {
        return if a == b { 1.0 } else { 0.0 };
    }

    let mut bigrams_a = std::collections::HashSet::new();
    for window in chars_a.windows(2) {
        bigrams_a.insert((window[0], window[1]));
    }

    let mut matches = 0;
    for window in chars_b.windows(2) {
        if bigrams_a.contains(&(window[0], window[1])) {
            matches += 1;
        }
    }

    (2.0 * matches as f32) / (chars_a.len() - 1 + chars_b.len() - 1) as f32
}

/// Convert common Hiragana and Katakana to Romaji
pub fn kana_to_romaji(text: &str) -> String {
    let mut result = String::new();
    for ch in text.chars() {
        let romaji = match ch {
            'あ' | 'ア' => "a", 'い' | 'イ' => "i", 'う' | 'ウ' => "u", 'え' | 'エ' => "e", 'お' | 'オ' => "o",
            'か' | 'カ' => "ka", 'き' | 'キ' => "ki", 'く' | 'ク' => "ku", 'け' | 'ケ' => "ke", 'こ' | 'コ' => "ko",
            'さ' | 'サ' => "sa", 'し' | 'シ' => "shi", 'す' | 'ス' => "su", 'せ' | 'セ' => "se", 'そ' | 'ソ' => "so",
            'た' | 'タ' => "ta", 'ち' | 'チ' => "chi", 'つ' | 'ツ' => "tsu", 'て' | 'テ' => "te", 'と' | 'ト' => "to",
            'な' | 'ナ' => "na", 'に' | 'ニ' => "ni", 'ぬ' | 'ヌ' => "nu", 'ね' | 'ネ' => "ne", 'の' | 'ノ' => "no",
            'は' | 'ハ' => "ha", 'ひ' | 'ヒ' => "hi", 'ふ' | 'フ' => "fu", 'へ' | 'ヘ' => "he", 'ほ' | 'ホ' => "ho",
            'ま' | 'マ' => "ma", 'み' | 'ミ' => "mi", 'む' | 'ム' => "mu", 'め' | 'メ' => "me", 'も' | 'モ' => "mo",
            'や' | 'ヤ' => "ya", 'ゆ' | 'ユ' => "yu", 'よ' | 'ヨ' => "yo",
            'ら' | 'ラ' => "ra", 'り' | 'リ' => "ri", 'る' | 'ル' => "ru", 'れ' | 'レ' => "re", 'ろ' | 'ロ' => "ro",
            'わ' | 'ワ' => "wa", 'を' | 'ヲ' => "wo", 'ん' | 'ン' => "n",
            'が' | 'ガ' => "ga", 'ぎ' | 'ギ' => "gi", 'ぐ' | 'グ' => "gu", 'げ' | 'ゲ' => "ge", 'ご' | 'ゴ' => "go",
            'ざ' | 'ザ' => "za", 'じ' | 'ジ' => "ji", 'ず' | 'ズ' => "zu", 'ぜ' | 'ゼ' => "ze", 'ぞ' | 'ゾ' => "zo",
            'だ' | 'ダ' => "da", 'ぢ' | 'ヂ' => "ji", 'づ' | 'ヅ' => "zu", 'で' | 'デ' => "de", 'ど' | 'ド' => "do",
            'ば' | 'バ' => "ba", 'び' | 'ビ' => "bi", 'ぶ' | 'ブ' => "bu", 'べ' | 'ベ' => "be", 'ぼ' | 'ボ' => "bo",
            'ぱ' | 'パ' => "pa", 'ぴ' | 'ピ' => "pi", 'ぷ' | 'プ' => "pu", 'ぺ' | 'ペ' => "pe", 'ぽ' | 'ポ' => "po",
            _ => {
                result.push(ch);
                continue;
            }
        };
        result.push_str(romaji);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn isolates_extra_files_without_shifting_tracks() {
        let files = vec![
            MusicFile {
                path: "00 crossfade.flac".to_string(),
                file_name: "00 crossfade.flac".to_string(),
                artist: None,
            },
            MusicFile {
                path: "01 title_a.flac".to_string(),
                file_name: "01 title_a.flac".to_string(),
                artist: None,
            },
            MusicFile {
                path: "02 title_b.flac".to_string(),
                file_name: "02 title_b.flac".to_string(),
                artist: None,
            },
        ];

        let tracks = vec![
            TrackMetadata {
                number: 1,
                title: "title_a".to_string(),
                artist: "Artist".to_string(),
                target_file_name: None,
            },
            TrackMetadata {
                number: 2,
                title: "title_b".to_string(),
                artist: "Artist".to_string(),
                target_file_name: None,
            },
        ];

        let aligned = align_files_and_tracks(&files, &tracks);
        assert_eq!(aligned.len(), 3);
        // 00 crossfade is matched with None (gap)
        assert_eq!(aligned[0].file.as_ref().unwrap().file_name, "00 crossfade.flac");
        assert!(aligned[0].track.is_none());
        // Track 1 matches 01 title_a
        assert_eq!(aligned[1].file.as_ref().unwrap().file_name, "01 title_a.flac");
        assert_eq!(aligned[1].track.as_ref().unwrap().number, 1);
        // Track 2 matches 02 title_b
        assert_eq!(aligned[2].file.as_ref().unwrap().file_name, "02 title_b.flac");
        assert_eq!(aligned[2].track.as_ref().unwrap().number, 2);
    }

    #[test]
    fn matches_romaji_filename_with_kana_title() {
        let files = vec![
            MusicFile {
                path: "01 Himitsu no Telepath.flac".to_string(),
                file_name: "01 Himitsu no Telepath.flac".to_string(),
                artist: None,
            },
        ];

        let tracks = vec![
            TrackMetadata {
                number: 1,
                title: "ヒミツのテレパス".to_string(),
                artist: "irucaice".to_string(),
                target_file_name: None,
            },
        ];

        let aligned = align_files_and_tracks(&files, &tracks);
        assert_eq!(aligned[0].file.as_ref().unwrap().file_name, "01 Himitsu no Telepath.flac");
        assert_eq!(aligned[0].track.as_ref().unwrap().title, "ヒミツのテレパス");
    }

    #[test]
    fn does_not_force_pair_unrelated_file_and_track() {
        let files = vec![
            MusicFile {
                path: "bonus_talk.wav".to_string(),
                file_name: "bonus_talk.wav".to_string(),
                artist: None,
            },
        ];
        let tracks = vec![
            TrackMetadata {
                number: 1,
                title: "弱くなれ！".to_string(),
                artist: "呆".to_string(),
                target_file_name: None,
            },
        ];

        let aligned = align_files_and_tracks(&files, &tracks);
        assert_eq!(aligned.len(), 2);
        // Neither item should be paired together; each is in its own row with a gap
        assert!(aligned.iter().all(|item| item.file.is_none() || item.track.is_none()));
        assert!(aligned.iter().any(|item| item.file.as_ref().map(|f| &f.file_name) == Some(&"bonus_talk.wav".to_string())));
        assert!(aligned.iter().any(|item| item.track.as_ref().map(|t| &t.title) == Some(&"弱くなれ！".to_string())));
    }
}
