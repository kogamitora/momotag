#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    Vocal,
    Lyrics,
    Music,
    Arrange,
    Mix,
    Mastering,
    Source,
    Other,
}

pub fn match_role(word: &str) -> Option<Role> {
    let cleaned = word
        .trim()
        .trim_matches(['.', ':', '：', ',', '&', '/', '／', '、', '・'])
        .to_lowercase();

    match cleaned.as_str() {
        "vocal" | "vocals" | "vo" | "vo." | "voice" | "歌" | "歌唱" | "ボーカル" | "cv" | "cv." => {
            Some(Role::Vocal)
        }
        "lyrics" | "lyric" | "words" | "作詞" | "作词" | "詞" | "词" => Some(Role::Lyrics),
        "music" | "compose" | "composer" | "composition" | "作曲" | "曲" => Some(Role::Music),
        "arrange" | "arrangement" | "arranger" | "arr" | "arr." | "編曲" | "编曲" => {
            Some(Role::Arrange)
        }
        "mix" | "mixing" | "mixed" | "混音" => Some(Role::Mix),
        "mastering" | "master" | "マスタリング" => Some(Role::Mastering),
        "from" | "原曲" | "収録" | "出典" => Some(Role::Source),
        "illust" | "illustration" | "movie" | "video" | "guitar" | "bass" | "piano" | "drums" => {
            Some(Role::Other)
        }
        _ => None,
    }
}

pub fn is_role_delimiter(ch: char) -> bool {
    matches!(ch, ',' | '&' | '+' | '/' | '／' | '、' | '・' | ' ' | '\t' | '　')
}
