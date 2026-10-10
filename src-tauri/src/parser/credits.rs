use super::lexicon::{is_role_delimiter, match_role, Role};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreditSegment {
    pub roles: Vec<Role>,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedCreditLine {
    pub prefix: Option<String>,
    pub credits: Vec<CreditSegment>,
}

pub fn parse_credit_line(line: &str) -> Option<ParsedCreditLine> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut labels = Vec::new();

    // Iterate through all colon occurrences (half-width ':' or full-width '：')
    for (colon_idx, ch) in trimmed.char_indices() {
        if ch != ':' && ch != '：' {
            continue;
        }

        let colon_len = ch.len_utf8();
        let preceding = &trimmed[..colon_idx];

        // Step backwards from colon to find a chain of (word, delimiter) where word is a valid role
        let mut words_rev = Vec::new();
        let mut curr_end = colon_idx;

        // Skip whitespace between last role and colon
        while curr_end > 0 {
            let prev_ch = preceding[..curr_end].chars().next_back().unwrap();
            if prev_ch.is_whitespace() {
                curr_end -= prev_ch.len_utf8();
            } else {
                break;
            }
        }

        let mut candidate_start = curr_end;

        while curr_end > 0 {
            // Find end of preceding word
            let mut word_start = curr_end;
            while word_start > 0 {
                let prev_ch = preceding[..word_start].chars().next_back().unwrap();
                if is_word_char(prev_ch) {
                    word_start -= prev_ch.len_utf8();
                } else {
                    break;
                }
            }

            if word_start == curr_end {
                break;
            }

            let word = &preceding[word_start..curr_end];
            let Some(role) = match_role(word) else {
                break;
            };

            words_rev.push((word_start, role));
            candidate_start = word_start;
            curr_end = word_start;

            // Check if there is a role delimiter before this word
            while curr_end > 0 {
                let prev_ch = preceding[..curr_end].chars().next_back().unwrap();
                if is_role_delimiter(prev_ch) {
                    curr_end -= prev_ch.len_utf8();
                } else {
                    break;
                }
            }
        }

        if !words_rev.is_empty() {
            words_rev.reverse();
            let roles: Vec<Role> = words_rev.into_iter().map(|(_, r)| r).collect();
            labels.push((candidate_start, colon_idx + colon_len, roles));
        }
    }

    if labels.is_empty() {
        return None;
    }

    // Sort labels by start index and eliminate overlaps
    labels.sort_by_key(|l| l.0);
    let mut non_overlapping = Vec::new();
    let mut last_end = 0;
    for label in labels {
        if label.0 >= last_end {
            last_end = label.1;
            non_overlapping.push(label);
        }
    }

    if non_overlapping.is_empty() {
        return None;
    }

    let first_start = non_overlapping[0].0;
    let prefix = if first_start > 0 {
        let p = trimmed[..first_start]
            .trim()
            .trim_end_matches(['-', '–', '—', '/', '／'])
            .trim();
        if !p.is_empty() {
            Some(p.to_string())
        } else {
            None
        }
    } else {
        None
    };

    let mut credits = Vec::new();
    for i in 0..non_overlapping.len() {
        let (_, val_start, ref roles) = non_overlapping[i];
        let val_end = if i + 1 < non_overlapping.len() {
            non_overlapping[i + 1].0
        } else {
            trimmed.len()
        };

        let val = trimmed[val_start..val_end].trim();
        let cleaned = clean_credit_value(val);
        if !cleaned.is_empty() {
            credits.push(CreditSegment {
                roles: roles.clone(),
                value: cleaned,
            });
        }
    }

    if credits.is_empty() {
        return None;
    }

    Some(ParsedCreditLine { prefix, credits })
}

pub fn select_artist_from_credits(credits: &[CreditSegment]) -> Option<String> {
    // 1. Source artist (From: Album / Artist)
    for c in credits {
        if c.roles.contains(&Role::Source) {
            if let Some(artist) = extract_artist_from_source(&c.value) {
                return Some(artist);
            }
        }
    }

    // 2. Vocal
    for c in credits {
        if c.roles.contains(&Role::Vocal) && !c.value.is_empty() {
            return Some(c.value.clone());
        }
    }

    // 3. Lyrics
    for c in credits {
        if c.roles.contains(&Role::Lyrics) && !c.value.is_empty() {
            return Some(c.value.clone());
        }
    }

    // 4. Music (Composer - for instrumental music, composer is the author)
    for c in credits {
        if c.roles.contains(&Role::Music) && !c.value.is_empty() {
            return Some(c.value.clone());
        }
    }

    // 5. Arrange
    for c in credits {
        if c.roles.contains(&Role::Arrange) && !c.value.is_empty() {
            return Some(c.value.clone());
        }
    }

    // 6. Other / first credit
    credits.first().map(|c| c.value.clone())
}

fn extract_artist_from_source(val: &str) -> Option<String> {
    let slash = val.find('/').or_else(|| val.find('／'))?;
    let artist = val[slash + 1..].trim();
    if !artist.is_empty() {
        Some(artist.to_string())
    } else {
        None
    }
}

fn clean_credit_value(val: &str) -> String {
    val.trim()
        .trim_matches([';', '；', '　', '-', '–', '—'])
        .trim()
        .to_string()
}

fn is_word_char(ch: char) -> bool {
    ch.is_alphanumeric() || matches!(ch, '\u{4E00}'..='\u{9FFF}' | '\u{3040}'..='\u{309F}' | '\u{30A0}'..='\u{30FF}')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_multi_role_credit_lines() {
        let line = "music,arrange,mix:Toccoyaki　music,lyric:桃寝ちのい";
        let parsed = parse_credit_line(line).unwrap();
        assert_eq!(parsed.credits.len(), 2);
        assert_eq!(parsed.credits[0].roles, vec![Role::Music, Role::Arrange, Role::Mix]);
        assert_eq!(parsed.credits[0].value, "Toccoyaki");
        assert_eq!(parsed.credits[1].roles, vec![Role::Music, Role::Lyrics]);
        assert_eq!(parsed.credits[1].value, "桃寝ちのい");
    }

    #[test]
    fn preserves_names_containing_colons() {
        let line = "music : ちろ:D　lyric : もちうつね";
        let parsed = parse_credit_line(line).unwrap();
        assert_eq!(parsed.credits.len(), 2);
        assert_eq!(parsed.credits[0].value, "ちろ:D");
        assert_eq!(parsed.credits[1].value, "もちうつね");
    }

    #[test]
    fn extracts_prefix_title() {
        let line = "Dear Next Song - Music&Lyrics: y0c1e";
        let parsed = parse_credit_line(line).unwrap();
        assert_eq!(parsed.prefix.as_deref(), Some("Dear Next Song"));
        assert_eq!(parsed.credits[0].value, "y0c1e");
    }
}
