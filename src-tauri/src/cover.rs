use crate::logger::{log_error, log_info};
use crate::models::CoverPreviewImage;
use image::codecs::jpeg::JpegEncoder;
use image::imageops::FilterType;
use image::ColorType;
use lofty::picture::{Picture, PictureType};
use reqwest::header::CONTENT_TYPE;
use std::fs;
use std::io::Cursor;
use std::io::Read;
use std::path::Path;
use std::time::Duration;

const SUPPORTED_COVER_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp"];
const MAX_COVER_BYTES: usize = 20 * 1024 * 1024;
/// Windows Media Foundation (`mf.dll`) rejects FLAC files whose total metadata
/// header exceeds 4 MiB (`4,194,304` bytes) with `MF_E_UNSUPPORTED_BYTESTREAM_TYPE`.
/// Keeping embedded cover art under ~2.5 MiB guarantees safe playback across
/// Windows Media Player and hardware/software decoders while the full-size
/// cover file is still saved on disk in the album folder.
const MAX_EMBEDDED_COVER_BYTES: usize = 2_500_000;

pub(crate) struct CoverImageData {
    pub(crate) data: Vec<u8>,
    pub(crate) mime_type: String,
    pub(crate) extension: String,
}

pub(crate) fn read_cover_preview(cover_path: &str) -> Result<CoverPreviewImage, String> {
    let cover_image = read_cover_image(cover_path)?;
    Ok(CoverPreviewImage {
        mime_type: cover_image.mime_type,
        data: cover_image.data,
    })
}

pub(crate) fn read_cover_image(cover_path: &str) -> Result<CoverImageData, String> {
    let source = cover_path.trim();
    if source.is_empty() {
        return Err("Selected cover image does not exist.".to_string());
    }

    if is_http_url(source) {
        return download_cover_image(source);
    }

    let path = Path::new(source);
    if !path.is_file() {
        log_error("cover", format!("local cover file does not exist: {source}"));
        return Err("Selected cover image does not exist.".to_string());
    }

    let mime_type = cover_mime_type_from_path(path)?;
    let extension = cover_extension_from_mime_type(&mime_type)
        .or_else(|| cover_extension_from_path(path))
        .ok_or_else(|| "Cover image must be JPG, PNG, or WEBP.".to_string())?;
    let data = fs::read(path).map_err(|err| {
        log_error("cover", format!("failed to read local cover {source}: {err}"));
        format!("Could not read cover image: {err}")
    })?;
    validate_cover_size(data.len())?;

    log_info(
        "cover",
        format!(
            "loaded local cover path={source} mime={mime_type} size={}B",
            data.len()
        ),
    );

    Ok(CoverImageData {
        data,
        mime_type,
        extension,
    })
}

fn download_cover_image(url: &str) -> Result<CoverImageData, String> {
    log_info("cover", format!("downloading remote cover url={url}"));
    let response = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(20))
        .user_agent("momotag-cover-loader")
        .build()
        .map_err(|err| format!("Could not prepare image download: {err}"))?
        .get(url)
        .send()
        .map_err(|err| {
            log_error("cover", format!("remote cover request failed url={url}: {err}"));
            format!("Could not download cover image: {err}")
        })?
        .error_for_status()
        .map_err(|err| {
            log_error("cover", format!("remote cover HTTP status error url={url}: {err}"));
            format!("Could not download cover image: {err}")
        })?;

    if response
        .content_length()
        .is_some_and(|length| length > MAX_COVER_BYTES as u64)
    {
        return Err("Cover image is too large.".to_string());
    }

    let header_mime_type = response
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string);
    // Do not rely on Content-Length: servers may omit or falsify it. Read at
    // most one byte beyond the configured limit so oversized responses are
    // rejected without buffering an unbounded payload in memory.
    let mut limited_response = response.take(MAX_COVER_BYTES as u64 + 1);
    let mut data = Vec::with_capacity(MAX_COVER_BYTES.min(1024 * 1024));
    limited_response
        .read_to_end(&mut data)
        .map_err(|err| format!("Could not read downloaded cover image: {err}"))?;
    validate_cover_size(data.len())?;

    let mime_type = header_mime_type
        .filter(|value| cover_extension_from_mime_type(value).is_some())
        .or_else(|| infer_cover_mime_type_from_bytes(&data))
        .or_else(|| {
            cover_extension_from_url(url)
                .map(|extension| cover_mime_type_from_extension(&extension).to_string())
                .filter(|value| !value.is_empty())
        })
        .ok_or_else(|| "Cover image must be JPG, PNG, or WEBP.".to_string())?;
    let extension = cover_extension_from_mime_type(&mime_type)
        .ok_or_else(|| "Cover image must be JPG, PNG, or WEBP.".to_string())?;

    log_info(
        "cover",
        format!(
            "downloaded remote cover url={url} mime={mime_type} size={}B",
            data.len()
        ),
    );

    Ok(CoverImageData {
        data,
        mime_type,
        extension,
    })
}

fn validate_cover_size(size: usize) -> Result<(), String> {
    if size == 0 {
        return Err("Cover image is empty.".to_string());
    }
    if size > MAX_COVER_BYTES {
        return Err("Cover image is too large.".to_string());
    }
    Ok(())
}

fn cover_mime_type_from_path(path: &Path) -> Result<String, String> {
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .ok_or_else(|| "Cover image has no extension.".to_string())?;
    let mime_type = cover_mime_type_from_extension(&extension);
    if mime_type.is_empty() {
        Err("Cover image must be JPG, PNG, or WEBP.".to_string())
    } else {
        Ok(mime_type.to_string())
    }
}

fn cover_mime_type_from_extension(extension: &str) -> &'static str {
    match extension {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        _ => "",
    }
}

fn cover_extension_from_mime_type(mime_type: &str) -> Option<String> {
    match mime_type {
        "image/jpeg" => Some("jpg".to_string()),
        "image/png" => Some("png".to_string()),
        "image/webp" => Some("webp".to_string()),
        _ => None,
    }
}

fn cover_extension_from_path(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| SUPPORTED_COVER_EXTENSIONS.contains(&value.as_str()))
}

fn cover_extension_from_url(url: &str) -> Option<String> {
    let without_fragment = url.split('#').next().unwrap_or(url);
    let without_query = without_fragment
        .split('?')
        .next()
        .unwrap_or(without_fragment);
    Path::new(without_query)
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .filter(|value| SUPPORTED_COVER_EXTENSIONS.contains(&value.as_str()))
}

fn infer_cover_mime_type_from_bytes(data: &[u8]) -> Option<String> {
    if data.starts_with(&[0xff, 0xd8, 0xff]) {
        return Some("image/jpeg".to_string());
    }
    if data.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png".to_string());
    }
    if data.len() >= 12 && &data[..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        return Some("image/webp".to_string());
    }
    None
}

pub(crate) fn cover_picture(data: &[u8]) -> Result<Picture, String> {
    let embedded_bytes = optimize_cover_for_embedding(data)?;
    let mut cursor = Cursor::new(embedded_bytes.as_slice());
    let mut picture =
        Picture::from_reader(&mut cursor).map_err(|err| format!("Invalid cover image: {err}"))?;
    picture.set_pic_type(PictureType::CoverFront);
    Ok(picture)
}

fn optimize_cover_for_embedding(data: &[u8]) -> Result<Vec<u8>, String> {
    let mime = infer_cover_mime_type_from_bytes(data).unwrap_or_else(|| "unknown".to_string());
    let is_webp = mime == "image/webp";
    if !is_webp && data.len() <= MAX_EMBEDDED_COVER_BYTES {
        log_info(
            "cover::embed",
            format!(
                "using original cover for embedding without re-encoding (mime={mime}, size={}B <= {MAX_EMBEDDED_COVER_BYTES}B)",
                data.len()
            ),
        );
        return Ok(data.to_vec());
    }

    let decoded = image::load_from_memory(data).map_err(|err| {
        log_error("cover::embed", format!("failed to decode cover for optimization: {err}"));
        format!("Invalid cover image: {err}")
    })?;

    for &(max_dim, quality) in &[
        (0u32, 90u8),
        (0u32, 82u8),
        (1600u32, 88u8),
        (1400u32, 82u8),
        (1000u32, 80u8),
    ] {
        let candidate = if max_dim > 0 && (decoded.width() > max_dim || decoded.height() > max_dim)
        {
            decoded.resize(max_dim, max_dim, FilterType::Lanczos3)
        } else {
            decoded.clone()
        };
        let rgb = candidate.to_rgb8();
        let mut buf = Vec::new();
        let mut encoder = JpegEncoder::new_with_quality(&mut buf, quality);
        encoder
            .encode(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                ColorType::Rgb8.into(),
            )
            .map_err(|err| format!("Invalid cover image: {err}"))?;
        if buf.len() <= MAX_EMBEDDED_COVER_BYTES {
            log_info(
                "cover::embed",
                format!(
                    "optimized cover for embedding: original={}B ({mime}, {}x{}) -> embedded={}B (image/jpeg, {}x{}, quality={quality})",
                    data.len(),
                    decoded.width(),
                    decoded.height(),
                    buf.len(),
                    rgb.width(),
                    rgb.height()
                ),
            );
            return Ok(buf);
        }
    }

    log_error(
        "cover::embed",
        format!(
            "cover image remains > {MAX_EMBEDDED_COVER_BYTES}B even after downscaling (original={}B)",
            data.len()
        ),
    );
    Err("Cover image is too large.".to_string())
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("https://") || value.starts_with("http://")
}

#[cfg(test)]
mod tests {
    use super::{
        cover_extension_from_url, infer_cover_mime_type_from_bytes, optimize_cover_for_embedding,
        validate_cover_size, MAX_EMBEDDED_COVER_BYTES,
    };
    use image::{ImageBuffer, ImageFormat, Rgb};
    use std::io::Cursor;

    #[test]
    fn detects_supported_image_signatures() {
        assert_eq!(
            infer_cover_mime_type_from_bytes(&[0xff, 0xd8, 0xff]),
            Some("image/jpeg".to_string())
        );
        assert_eq!(
            infer_cover_mime_type_from_bytes(b"\x89PNG\r\n\x1a\n"),
            Some("image/png".to_string())
        );
        assert_eq!(
            infer_cover_mime_type_from_bytes(b"RIFF0000WEBP"),
            Some("image/webp".to_string())
        );
    }

    #[test]
    fn extracts_extension_without_query_or_fragment() {
        assert_eq!(
            cover_extension_from_url("https://example.test/COVER.JPEG?size=large#x"),
            Some("jpeg".to_string())
        );
        assert_eq!(
            cover_extension_from_url("https://example.test/file.gif"),
            None
        );
    }

    #[test]
    fn enforces_cover_size_limits() {
        assert!(validate_cover_size(1).is_ok());
        assert!(validate_cover_size(0).is_err());
        assert!(validate_cover_size(20 * 1024 * 1024 + 1).is_err());
    }

    #[test]
    fn compresses_oversized_png_for_embedding() {
        let img = ImageBuffer::from_fn(1200, 1200, |x, y| {
            let r = (x.wrapping_mul(2654435761) ^ y.wrapping_mul(2246822519)) as u8;
            let g = (x.wrapping_mul(3266489917) ^ y.wrapping_mul(668265263)) as u8;
            let b = (x ^ y).wrapping_mul(374761393) as u8;
            Rgb([r, g, b])
        });
        let mut png_bytes = Vec::new();
        img.write_to(&mut Cursor::new(&mut png_bytes), ImageFormat::Png)
            .unwrap();
        assert!(png_bytes.len() > MAX_EMBEDDED_COVER_BYTES);

        let optimized = optimize_cover_for_embedding(&png_bytes).unwrap();
        assert!(optimized.len() <= MAX_EMBEDDED_COVER_BYTES);
        assert_eq!(
            infer_cover_mime_type_from_bytes(&optimized),
            Some("image/jpeg".to_string())
        );
    }
}
