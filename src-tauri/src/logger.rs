use std::fs::{self, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_LOG_FILE_BYTES: u64 = 5 * 1024 * 1024;
/// Windows Media Foundation (`mf.dll`) rejects FLAC files whose metadata header
/// before the first audio frame exceeds 4 MiB (`4,194,304` bytes).
const WMF_FLAC_HEADER_LIMIT_BYTES: u64 = 4 * 1024 * 1024;

static LOG_LOCK: Mutex<()> = Mutex::new(());

pub(crate) fn log_dir() -> PathBuf {
    if let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") {
        PathBuf::from(local_app_data).join("momotag").join("logs")
    } else if let Some(home) = std::env::var_os("USERPROFILE").or_else(|| std::env::var_os("HOME")) {
        PathBuf::from(home).join(".momotag").join("logs")
    } else {
        std::env::temp_dir().join("momotag").join("logs")
    }
}

pub(crate) fn log_file_path() -> PathBuf {
    log_dir().join("momotag.log")
}

pub(crate) fn init_logger() {
    let path = log_file_path();
    log_info(
        "app",
        format!(
            "=== momotag v{} started (os={}, arch={}, log={}) ===",
            env!("CARGO_PKG_VERSION"),
            std::env::consts::OS,
            std::env::consts::ARCH,
            path.display()
        ),
    );
}

pub(crate) fn log_info(scope: &str, message: impl AsRef<str>) {
    write_log_line("INFO", scope, message.as_ref());
}

pub(crate) fn log_warn(scope: &str, message: impl AsRef<str>) {
    write_log_line("WARN", scope, message.as_ref());
}

pub(crate) fn log_error(scope: &str, message: impl AsRef<str>) {
    write_log_line("ERROR", scope, message.as_ref());
}

fn write_log_line(level: &str, scope: &str, message: &str) {
    let Ok(_guard) = LOG_LOCK.lock() else {
        return;
    };

    let dir = log_dir();
    if fs::create_dir_all(&dir).is_err() {
        return;
    }

    let path = dir.join("momotag.log");
    if let Ok(meta) = fs::metadata(&path) {
        if meta.len() >= MAX_LOG_FILE_BYTES {
            let prev = dir.join("momotag.prev.log");
            let _ = fs::remove_file(&prev);
            let _ = fs::rename(&path, &prev);
        }
    }

    let timestamp = format_timestamp();
    let sanitized = message.replace('\r', " ").replace('\n', " | ");
    let line = format!("[{timestamp}] [{level}] [{scope}] {sanitized}\n");

    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) {
        let _ = file.write_all(line.as_bytes());
    }
}

/// Inspect a FLAC file's binary structure after tag writing and log block layout,
/// audio offset, Windows Media Foundation 4 MiB header limit check, and first
/// audio frame sync bytes (`0xFFF8` / `0xFFF9`).
pub(crate) fn log_flac_diagnostics(path: &Path) {
    let is_flac = path
        .extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| ext.eq_ignore_ascii_case("flac"));
    if !is_flac {
        return;
    }

    let mut file = match fs::File::open(path) {
        Ok(f) => f,
        Err(err) => {
            log_warn(
                "flac_check",
                format!("could not open {} for post-write check: {err}", path.display()),
            );
            return;
        }
    };

    let file_size = file.metadata().map(|m| m.len()).unwrap_or(0);
    let mut magic = [0u8; 4];
    if file.read_exact(&mut magic).is_err() || &magic != b"fLaC" {
        log_error(
            "flac_check",
            format!(
                "file={} (size={file_size}) missing 'fLaC' magic header ({magic:?})",
                path.display()
            ),
        );
        return;
    }

    let mut pos: u64 = 4;
    let mut blocks_summary = Vec::new();
    let mut blocks_count = 0usize;
    loop {
        let mut header = [0u8; 4];
        if let Err(err) = file.read_exact(&mut header) {
            log_error(
                "flac_check",
                format!(
                    "file={} truncated while reading metadata block header at offset {pos}: {err}",
                    path.display()
                ),
            );
            return;
        }
        let is_last = (header[0] & 0x80) != 0;
        let block_type = header[0] & 0x7F;
        let length =
            ((header[1] as u64) << 16) | ((header[2] as u64) << 8) | (header[3] as u64);
        let type_name = match block_type {
            0 => "STREAMINFO",
            1 => "PADDING",
            2 => "APPLICATION",
            3 => "SEEKTABLE",
            4 => "VORBIS_COMMENT",
            5 => "CUESHEET",
            6 => "PICTURE",
            _ => "UNKNOWN",
        };
        blocks_summary.push(format!("{type_name}({length}B)"));
        pos += 4 + length;
        blocks_count += 1;

        if file.seek(SeekFrom::Start(pos)).is_err() {
            log_error(
                "flac_check",
                format!(
                    "file={} failed to seek to end of block {type_name} at {pos}",
                    path.display()
                ),
            );
            return;
        }

        if is_last || blocks_count > 64 {
            break;
        }
    }

    let mut sync = [0u8; 4];
    let sync_ok = file.read_exact(&mut sync).is_ok()
        && sync[0] == 0xFF
        && (sync[1] & 0xFE) == 0xF8;
    let sync_hex = format!("{:02x}{:02x}{:02x}{:02x}", sync[0], sync[1], sync[2], sync[3]);

    if pos > WMF_FLAC_HEADER_LIMIT_BYTES {
        log_warn(
            "flac_check",
            format!(
                "file={} size={file_size}B audio_offset={pos}B EXCEEDS Windows Media Foundation 4MiB limit ({WMF_FLAC_HEADER_LIMIT_BYTES}B -> 0xC00D36C4)! blocks=[{}] sync=0x{sync_hex} (valid={sync_ok})",
                path.display(),
                blocks_summary.join(", ")
            ),
        );
    } else if !sync_ok {
        log_error(
            "flac_check",
            format!(
                "file={} size={file_size}B audio_offset={pos}B INVALID first audio frame sync=0x{sync_hex}! blocks=[{}]",
                path.display(),
                blocks_summary.join(", ")
            ),
        );
    } else {
        log_info(
            "flac_check",
            format!(
                "file={} size={file_size}B audio_offset={pos}B (<4MiB OK) sync=0x{sync_hex} OK blocks=[{}]",
                path.display(),
                blocks_summary.join(", ")
            ),
        );
    }
}

fn format_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = now.as_secs();
    let millis = now.subsec_millis();

    let days = (total_secs / 86_400) as i64;
    let rem = total_secs % 86_400;
    let hour = rem / 3600;
    let minute = (rem % 3600) / 60;
    let second = rem % 60;

    let (year, month, day) = civil_from_days(days);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}.{millis:03}Z")
}

/// Howard Hinnant's civil_from_days algorithm (converts days since 1970-01-01 to Y-M-D).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = (yoe as i64) + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    (year, m, d)
}

#[cfg(test)]
mod tests {
    use super::{civil_from_days, log_file_path, log_info};

    #[test]
    fn converts_epoch_days_correctly() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(20749), (2026, 10, 23));
    }

    #[test]
    fn writes_to_log_file_without_panicking() {
        log_info("test", "diagnostic logger unit test entry");
        assert!(log_file_path().exists());
    }
}
