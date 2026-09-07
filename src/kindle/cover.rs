//! Lower source cover binaries into the representations expected by Kindle.
//!
//! Full-size cover lowering is required and may return a conversion error;
//! library thumbnail generation is separate and best-effort. Cover navigation
//! and XHTML suppression remain the responsibility of `normalize`.

use std::io::Cursor;

use image::codecs::jpeg::JpegEncoder;
use image::{DynamicImage, ImageFormat, ImageReader, RgbImage};

use crate::error::{Error, Result};

use super::KindleResource;

const LIBRARY_THUMBNAIL_MAX_WIDTH: u32 = 330;
const LIBRARY_THUMBNAIL_MAX_HEIGHT: u32 = 470;
const LIBRARY_THUMBNAIL_JPEG_QUALITY: u8 = 80;
const COVER_JPEG_QUALITY: u8 = 100;

/// Prepare the native cover representation required by the KF8 output.
///
/// PNG covers are decoded, flattened onto white, and encoded as RGB JPEG;
/// failures are returned because a selected full-size PNG must not silently
/// remain in the output. JPEG covers are passed through unchanged. This
/// required path is intentionally separate from the best-effort thumbnail path.
pub(crate) fn prepare_cover_resource(
    resources: &mut [KindleResource],
    cover_resource_id: Option<&str>,
) -> Result<()> {
    let Some(cover) =
        cover_resource_id.and_then(|id| resources.iter_mut().find(|resource| resource.id == id))
    else {
        return Ok(());
    };
    if !cover.media_type.eq_ignore_ascii_case("image/png") {
        return Ok(());
    }
    let data = convert_png_cover_to_jpeg(&cover.data)?;
    cover.data = data;
    cover.media_type = "image/jpeg".to_owned();
    Ok(())
}

fn convert_png_cover_to_jpeg(data: &[u8]) -> Result<Vec<u8>> {
    let reader = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .map_err(|error| Error::Output(format!("cover format detection failed: {error}")))?;
    if reader.format() != Some(ImageFormat::Png) {
        return Err(Error::Output(
            "selected PNG cover is not PNG data".to_owned(),
        ));
    }
    let image = reader
        .decode()
        .map_err(|error| Error::Output(format!("PNG cover decode failed: {error}")))?;
    let (width, height) = (image.width(), image.height());
    let rgba = image.to_rgba8();
    let mut rgb = RgbImage::new(width, height);
    for (source, destination) in rgba.pixels().zip(rgb.pixels_mut()) {
        let alpha = u16::from(source[3]);
        for channel in 0..3 {
            let foreground = u32::from(source[channel]);
            destination[channel] =
                ((foreground * u32::from(alpha) + 255 * u32::from(255 - alpha) + 127) / 255) as u8;
        }
    }
    let mut encoded = Vec::new();
    JpegEncoder::new_with_quality(&mut encoded, COVER_JPEG_QUALITY)
        .encode_image(&DynamicImage::ImageRgb8(rgb))
        .map_err(|error| Error::Output(format!("PNG cover JPEG encode failed: {error}")))?;
    Ok(encoded)
}

/// Lower the Aozora cover to a standalone Kindle Library thumbnail.
///
/// This intentionally returns `None` for missing, unsupported, malformed, or
/// unencodable input. The full source cover remains in the output in all of
/// those cases, but EXTH 202/129 are omitted rather than pointing at a record
/// that is not a valid thumbnail.
pub(crate) fn generate_library_thumbnail(resource: &KindleResource) -> Option<Vec<u8>> {
    if !matches!(
        resource.media_type.to_ascii_lowercase().as_str(),
        "image/jpeg" | "image/png"
    ) {
        return None;
    }
    let reader = ImageReader::new(Cursor::new(&resource.data))
        .with_guessed_format()
        .ok()?;
    if !matches!(reader.format(), Some(ImageFormat::Jpeg | ImageFormat::Png)) {
        return None;
    }
    let image = reader.decode().ok()?;
    let (width, height) = thumbnail_dimensions(image.width(), image.height());
    let resized = image.resize_exact(width, height, image::imageops::FilterType::Lanczos3);
    let rgba = resized.to_rgba8();
    let mut rgb = RgbImage::new(width, height);
    for (source, destination) in rgba.pixels().zip(rgb.pixels_mut()) {
        let alpha = u16::from(source[3]);
        for channel in 0..3 {
            let foreground = u32::from(source[channel]);
            destination[channel] =
                ((foreground * u32::from(alpha) + 255 * u32::from(255 - alpha) + 127) / 255) as u8;
        }
    }
    let mut encoded = Vec::new();
    JpegEncoder::new_with_quality(&mut encoded, LIBRARY_THUMBNAIL_JPEG_QUALITY)
        .encode_image(&DynamicImage::ImageRgb8(rgb))
        .ok()?;
    Some(encoded)
}

fn thumbnail_dimensions(width: u32, height: u32) -> (u32, u32) {
    if width <= LIBRARY_THUMBNAIL_MAX_WIDTH && height <= LIBRARY_THUMBNAIL_MAX_HEIGHT {
        return (width.max(1), height.max(1));
    }
    let width64 = u64::from(width);
    let height64 = u64::from(height);
    let max_width64 = u64::from(LIBRARY_THUMBNAIL_MAX_WIDTH);
    let max_height64 = u64::from(LIBRARY_THUMBNAIL_MAX_HEIGHT);
    if width64 * max_height64 > height64 * max_width64 {
        (
            LIBRARY_THUMBNAIL_MAX_WIDTH,
            (height64 * max_width64 / width64).max(1) as u32,
        )
    } else {
        (
            (width64 * max_height64 / height64).max(1) as u32,
            LIBRARY_THUMBNAIL_MAX_HEIGHT,
        )
    }
}
