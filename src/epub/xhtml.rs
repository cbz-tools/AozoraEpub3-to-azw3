//! Extract semantic information from source EPUB XHTML documents.
//!
//! This includes document styles, writing-mode hints, links, and Aozora
//! semantic elements. KF8-specific XHTML rewriting belongs to `kf8::rawml`.

use std::collections::HashSet;
use std::io::Cursor;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use quick_xml::name::{Namespace, ResolveResult};
use quick_xml::reader::NsReader;

use super::css::{validate_kf8_css, validate_kf8_inline_style};
use super::navigation::has_token;
use super::opf::{ManifestItem, attr, local_name_ref};
use crate::book::{Layout, PageProgression, SemanticDocument, Styles, WritingMode};
use crate::css::inline_style_href_with_occupied_hrefs;
use crate::error::{Error, Result};
use crate::xhtml::path::normalize_path_lossy as normalize_path;
use crate::xhtml::path::{is_external_reference, resolve_path};
use crate::xhtml::scan::{
    html_local_name_is_text as html_local_name_is, html_tag_end,
    html_tag_name_range_with_leading_space as html_tag_name_range,
};

fn combined_xhtml_semantics(source: &str) -> Result<(SemanticDocument, Option<WritingMode>)> {
    let mut reader = Reader::from_reader(Cursor::new(source.as_bytes()));
    let mut buffer = Vec::new();
    let mut result = SemanticDocument::default();
    let mut html_mode = None;
    let mut body_mode = None;
    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Start(event) | Event::Empty(event) => {
                let event_name = event.name();
                let name = local_name_ref(event_name.as_ref());
                if name.eq_ignore_ascii_case("body")
                    && attr(&event, "type").is_some_and(|value| has_token(&value, "cover"))
                {
                    result.is_cover = true;
                }
                if name.eq_ignore_ascii_case("img") || name.eq_ignore_ascii_case("image") {
                    if let Some(href) = attr(&event, "src").or_else(|| attr(&event, "href")) {
                        result.image_references.push(href);
                    }
                }
                if name.eq_ignore_ascii_case("html") && html_mode.is_none() {
                    html_mode = root_attribute_writing_mode(&event);
                } else if name.eq_ignore_ascii_case("body") && body_mode.is_none() {
                    body_mode = root_attribute_writing_mode(&event);
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    Ok((result, html_mode.or(body_mode)))
}

#[allow(dead_code)]
pub(super) fn parse_xhtml_semantics(source: &str) -> Result<SemanticDocument> {
    combined_xhtml_semantics(source).map(|(semantic, _)| semantic)
}

const MATHML_NAMESPACE: &[u8] = b"http://www.w3.org/1998/Math/MathML";

/// Reject MathML before XHTML is transported into KF8 RawML. KF8 has no
/// reviewed MathML semantic projection, so preserving the source or relying
/// on a fallback image would silently change the publication contract.
pub(super) fn reject_mathml(source: &str) -> Result<()> {
    let mut reader = NsReader::from_reader(Cursor::new(source.as_bytes()));
    let mut buffer = Vec::new();
    loop {
        let (namespace, event) = reader.read_resolved_event_into(&mut buffer)?;
        match event {
            Event::Start(event) | Event::Empty(event)
                if event.name().local_name().as_ref() == b"math"
                    && matches!(
                        namespace,
                        ResolveResult::Bound(Namespace(uri)) if uri == MATHML_NAMESPACE
                    ) =>
            {
                return Err(Error::UnsupportedEpub(
                    "MathML is unsupported by the KF8 projection".to_owned(),
                ));
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    Ok(())
}

/// Reject executable scripting and forms in spine content before KF8 output.
/// Non-JavaScript script data blocks are inert metadata and remain harmless.
pub(super) fn reject_scripting(source: &str) -> Result<()> {
    let mut cursor = 0usize;
    while cursor < source.len() {
        let Some(relative) = source[cursor..].find('<') else {
            break;
        };
        let start = cursor + relative;
        if source[start..].starts_with("<!--") {
            cursor = source[start + 4..]
                .find("-->")
                .map_or(source.len(), |end| start + 4 + end + 3);
            continue;
        }
        let Some(tag_end) = html_tag_end(source, start) else {
            break;
        };
        let Some((name_start, name_end, closing)) = html_tag_name_range(source, start, tag_end)
        else {
            cursor = tag_end + 1;
            continue;
        };
        if closing {
            cursor = tag_end + 1;
            continue;
        }
        let name = &source[name_start..name_end];
        if html_local_name_is(name, "form") {
            return Err(Error::UnsupportedEpub(
                "G2-24/G8 scripting boundary: HTML form in spine content is unsupported".to_owned(),
            ));
        }
        if html_local_name_is(name, "script") {
            let has_external_reference = tag_attribute(source, start, tag_end, "src")?.is_some()
                || tag_attribute(source, start, tag_end, "href")?.is_some()
                || tag_attribute(source, start, tag_end, "xlink:href")?.is_some();
            if has_external_reference {
                return Err(Error::UnsupportedEpub(
                    "G2-24/G8 scripting boundary: external script reference in spine content is unsupported"
                        .to_owned(),
                ));
            }
            let script_type = tag_attribute(source, start, tag_end, "type")?;
            if !is_script_data_block(script_type.as_deref()) {
                return Err(Error::UnsupportedEpub(
                    "G2-24/G8 scripting boundary: inline script in spine content is unsupported"
                        .to_owned(),
                ));
            }
            cursor = if source[..tag_end].trim_end().ends_with('/') {
                tag_end + 1
            } else {
                raw_text_end(source, tag_end, "script").unwrap_or(source.len())
            };
            continue;
        }
        cursor = tag_end + 1;
    }
    Ok(())
}

/// Reject audio/video playback in spine content before KF8 lowering. The
/// current path has no reviewed playback projection, so preserving the
/// element or relying on fallback content would silently change the reading
/// experience. This intentionally inspects the XHTML element rather than the
/// manifest, allowing unused media resources to remain packageable.
#[allow(dead_code)]
pub(super) fn reject_media_playback(source: &str) -> Result<()> {
    let mut cursor = 0usize;
    while cursor < source.len() {
        let Some(relative) = source[cursor..].find('<') else {
            break;
        };
        let start = cursor + relative;
        if source[start..].starts_with("<!--") {
            cursor = source[start + 4..]
                .find("-->")
                .map_or(source.len(), |end| start + 4 + end + 3);
            continue;
        }
        let Some(tag_end) = html_tag_end(source, start) else {
            break;
        };
        let Some((name_start, name_end, closing)) = html_tag_name_range(source, start, tag_end)
        else {
            cursor = tag_end + 1;
            continue;
        };
        let name = &source[name_start..name_end];
        if !closing && html_local_name_is(name, "audio") {
            return Err(Error::UnsupportedEpub(
                "G6-23/G6-25 media boundary: audio playback in spine content is unsupported"
                    .to_owned(),
            ));
        }
        if !closing && html_local_name_is(name, "video") {
            return Err(Error::UnsupportedEpub(
                "G6-24/G6-25 media boundary: video playback in spine content is unsupported"
                    .to_owned(),
            ));
        }
        if !closing
            && (html_local_name_is(name, "script") || html_local_name_is(name, "style"))
            && !source[..tag_end].trim_end().ends_with('/')
        {
            cursor = raw_text_end(source, tag_end, name).unwrap_or(source.len());
            continue;
        }
        cursor = tag_end + 1;
    }
    Ok(())
}

/// Run the scripting and media boundary checks in one raw-HTML scan.
///
/// The scripting check retains precedence over media playback, including when
/// the media element appears earlier in source order. Styles are scanned for
/// scripting in the same way as the standalone scripting check, while media
/// checks ignore style raw text as the standalone media check does.
pub(super) fn reject_scripting_and_media_playback(source: &str) -> Result<()> {
    // Keep one main document scan. Media skips style/script raw text, while
    // scripting inspects each style body through a bounded local scan.
    let mut cursor = 0usize;
    let mut scripting_skip_until = 0usize;
    let mut media_violation = None;
    while let Some(tag) = next_raw_scan_tag(source, &mut cursor) {
        let name = tag.name;
        let scripting_active = tag.start >= scripting_skip_until;
        if scripting_active && !tag.closing && html_local_name_is(name, "form") {
            return Err(Error::UnsupportedEpub(
                "G2-24/G8 scripting boundary: HTML form in spine content is unsupported".to_owned(),
            ));
        }
        if !tag.closing && html_local_name_is(name, "audio") {
            media_violation.get_or_insert(MediaViolation::Audio);
        } else if !tag.closing && html_local_name_is(name, "video") {
            media_violation.get_or_insert(MediaViolation::Video);
        }

        if !tag.closing && html_local_name_is(name, "script") {
            if scripting_active {
                let has_external_reference = tag_attribute(source, tag.start, tag.end, "src")?
                    .is_some()
                    || tag_attribute(source, tag.start, tag.end, "href")?.is_some()
                    || tag_attribute(source, tag.start, tag.end, "xlink:href")?.is_some();
                if has_external_reference {
                    return Err(Error::UnsupportedEpub(
                        "G2-24/G8 scripting boundary: external script reference in spine content is unsupported"
                            .to_owned(),
                    ));
                }
                let script_type = tag_attribute(source, tag.start, tag.end, "type")?;
                if !is_script_data_block(script_type.as_deref()) {
                    return Err(Error::UnsupportedEpub(
                        "G2-24/G8 scripting boundary: inline script in spine content is unsupported"
                            .to_owned(),
                    ));
                }
            }
            cursor = if tag.self_closing {
                tag.end + 1
            } else {
                raw_text_end(source, tag.end, "script").unwrap_or(source.len())
            };
            continue;
        }

        if !tag.closing && html_local_name_is(name, "style") && !tag.self_closing {
            let (style_body_end, style_raw_end) = closing_tag(source, tag.end + 1, "style")
                .map_or((source.len(), source.len()), |(start, end)| {
                    (start, end + 1)
                });
            let scripting_body_start = if scripting_active {
                tag.end + 1
            } else {
                scripting_skip_until.max(tag.end + 1)
            };
            if scripting_body_start < style_body_end
                && let Some(skip_until) =
                    scan_scripting_style_body(source, scripting_body_start, style_body_end)?
            {
                scripting_skip_until = skip_until;
            }
            cursor = style_raw_end;
            continue;
        }

        cursor = tag.end + 1;
    }
    match media_violation {
        Some(MediaViolation::Audio) => Err(Error::UnsupportedEpub(
            "G6-23/G6-25 media boundary: audio playback in spine content is unsupported".to_owned(),
        )),
        Some(MediaViolation::Video) => Err(Error::UnsupportedEpub(
            "G6-24/G6-25 media boundary: video playback in spine content is unsupported".to_owned(),
        )),
        None => Ok(()),
    }
}

struct RawScanTag<'a> {
    start: usize,
    end: usize,
    name: &'a str,
    closing: bool,
    self_closing: bool,
}

fn next_raw_scan_tag<'a>(source: &'a str, cursor: &mut usize) -> Option<RawScanTag<'a>> {
    while *cursor < source.len() {
        let relative = source[*cursor..].find('<')?;
        let start = *cursor + relative;
        if source[start..].starts_with("<!--") {
            *cursor = source[start + 4..]
                .find("-->")
                .map_or(source.len(), |end| start + 4 + end + 3);
            continue;
        }
        let tag_end = html_tag_end(source, start)?;
        let Some((name_start, name_end, closing)) = html_tag_name_range(source, start, tag_end)
        else {
            *cursor = tag_end + 1;
            continue;
        };
        let self_closing = source[..tag_end].trim_end().ends_with('/');
        return Some(RawScanTag {
            start,
            end: tag_end,
            name: &source[name_start..name_end],
            closing,
            self_closing,
        });
    }
    None
}

fn scan_scripting_style_body(
    source: &str,
    body_start: usize,
    body_end: usize,
) -> Result<Option<usize>> {
    let mut cursor = body_start;
    while cursor < body_end {
        let Some(relative) = source[cursor..body_end].find('<') else {
            return Ok(None);
        };
        let start = cursor + relative;
        if source[start..].starts_with("<!--") {
            cursor = source[start + 4..]
                .find("-->")
                .map_or(source.len(), |end| start + 4 + end + 3);
            if cursor > body_end {
                return Ok(Some(cursor));
            }
            continue;
        }
        let Some(tag_end) = html_tag_end(source, start) else {
            return Ok(Some(source.len()));
        };
        let Some((name_start, name_end, closing)) = html_tag_name_range(source, start, tag_end)
        else {
            cursor = tag_end + 1;
            if cursor > body_end {
                return Ok(Some(cursor));
            }
            continue;
        };
        if closing {
            cursor = tag_end + 1;
            if cursor > body_end {
                return Ok(Some(cursor));
            }
            continue;
        }
        let name = &source[name_start..name_end];
        if html_local_name_is(name, "form") {
            return Err(Error::UnsupportedEpub(
                "G2-24/G8 scripting boundary: HTML form in spine content is unsupported".to_owned(),
            ));
        }
        if html_local_name_is(name, "script") {
            let has_external_reference = tag_attribute(source, start, tag_end, "src")?.is_some()
                || tag_attribute(source, start, tag_end, "href")?.is_some()
                || tag_attribute(source, start, tag_end, "xlink:href")?.is_some();
            if has_external_reference {
                return Err(Error::UnsupportedEpub(
                    "G2-24/G8 scripting boundary: external script reference in spine content is unsupported"
                        .to_owned(),
                ));
            }
            let script_type = tag_attribute(source, start, tag_end, "type")?;
            if !is_script_data_block(script_type.as_deref()) {
                return Err(Error::UnsupportedEpub(
                    "G2-24/G8 scripting boundary: inline script in spine content is unsupported"
                        .to_owned(),
                ));
            }
            cursor = if source[..tag_end].trim_end().ends_with('/') {
                tag_end + 1
            } else {
                raw_text_end(source, tag_end, "script").unwrap_or(source.len())
            };
        } else {
            cursor = tag_end + 1;
        }
        if cursor > body_end {
            return Ok(Some(cursor));
        }
    }
    Ok(None)
}

#[derive(Clone, Copy)]
enum MediaViolation {
    Audio,
    Video,
}

/// Reject package-local generic resources used as `<object data>` targets.
/// KF8 can serialize the bytes, but the current resource-reference path has
/// no faithful projection for a generic object target. Keeping the original
/// EPUB href would therefore create a successful-looking but unreachable
/// object rather than a safe conversion.
#[allow(dead_code)]
pub(super) fn reject_unsupported_generic_object(
    source: &str,
    document_href: &str,
    manifest: &[ManifestItem],
) -> Result<()> {
    let mut reader = Reader::from_reader(Cursor::new(source.as_bytes()));
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Start(event) | Event::Empty(event)
                if local_name_ref(event.name().as_ref()).eq_ignore_ascii_case("object") =>
            {
                let Some(target) = attr(&event, "data") else {
                    buffer.clear();
                    continue;
                };
                let Some(resolved) = resolve_path(document_href, &target) else {
                    buffer.clear();
                    continue;
                };
                let Some(item) = manifest
                    .iter()
                    .find(|item| normalize_path(&item.href) == resolved)
                else {
                    buffer.clear();
                    continue;
                };
                if !is_object_projectable_media_type(&item.media_type) {
                    return Err(Error::UnsupportedEpub(format!(
                        "G6-22 generic non-image object resource {} is unsupported by the KF8 projection",
                        item.id
                    )));
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    Ok(())
}

fn is_object_projectable_media_type(media_type: &str) -> bool {
    let media_type = media_type.to_ascii_lowercase();
    media_type.starts_with("image/")
        || media_type.starts_with("font/")
        || matches!(
            media_type.as_str(),
            "application/font-sfnt"
                | "application/vnd.ms-opentype"
                | "application/x-font-opentype"
                | "application/x-font-ttf"
                | "application/xhtml+xml"
                | "text/html"
                | "text/css"
        )
}

/// Reject package-local `srcset` candidates before KF8 lowering. The current
/// asset path projects the fallback `src`/`href`, but does not select or
/// rewrite responsive image candidates; accepting a package-local candidate
/// would therefore silently change the publication semantics. A picture with
/// only its fallback image remains supported.
#[allow(dead_code)]
pub(super) fn reject_unsupported_srcset(source: &str) -> Result<()> {
    let mut reader = Reader::from_reader(Cursor::new(source.as_bytes()));
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Start(event) | Event::Empty(event)
                if matches!(
                    local_name_ref(event.name().as_ref())
                        .to_ascii_lowercase()
                        .as_str(),
                    "img" | "source"
                ) =>
            {
                if let Some(srcset) = attr(&event, "srcset")
                    && srcset_has_package_local_candidate(&srcset)
                {
                    return Err(Error::UnsupportedEpub(
                        "G3-19 picture/source srcset with package-local candidate is unsupported by the KF8 projection".to_owned(),
                    ));
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    Ok(())
}

/// Reject unsupported `srcset` candidates and generic object targets in one
/// quick_xml Reader pass. A srcset violation remains dominant over an object
/// violation, while Reader errors retain the original srcset-pass precedence.
pub(super) fn reject_unsupported_srcset_and_generic_object(
    source: &str,
    document_href: &str,
    manifest: &[ManifestItem],
) -> Result<()> {
    let mut reader = Reader::from_reader(Cursor::new(source.as_bytes()));
    let mut buffer = Vec::new();
    let mut object_violation = None;
    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Start(event) | Event::Empty(event) => {
                let event_name = event.name();
                let name = local_name_ref(event_name.as_ref());
                if (name.eq_ignore_ascii_case("img") || name.eq_ignore_ascii_case("source"))
                    && attr(&event, "srcset")
                        .is_some_and(|srcset| srcset_has_package_local_candidate(&srcset))
                {
                    return Err(Error::UnsupportedEpub(
                        "G3-19 picture/source srcset with package-local candidate is unsupported by the KF8 projection".to_owned(),
                    ));
                }
                if name.eq_ignore_ascii_case("object") && object_violation.is_none() {
                    let Some(target) = attr(&event, "data") else {
                        buffer.clear();
                        continue;
                    };
                    let Some(resolved) = resolve_path(document_href, &target) else {
                        buffer.clear();
                        continue;
                    };
                    let Some(item) = manifest
                        .iter()
                        .find(|item| normalize_path(&item.href) == resolved)
                    else {
                        buffer.clear();
                        continue;
                    };
                    if !is_object_projectable_media_type(&item.media_type) {
                        object_violation = Some(Error::UnsupportedEpub(format!(
                            "G6-22 generic non-image object resource {} is unsupported by the KF8 projection",
                            item.id
                        )));
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    match object_violation {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

fn srcset_has_package_local_candidate(srcset: &str) -> bool {
    let mut candidate_start = 0;
    let mut data_url_target = starts_with_data_url(&srcset[candidate_start..]);
    let mut target_complete = !data_url_target;

    for (index, byte) in srcset.bytes().enumerate() {
        if data_url_target && !target_complete && byte.is_ascii_whitespace() {
            target_complete = true;
        }

        if byte == b',' && (!data_url_target || target_complete) {
            if srcset_candidate_is_package_local(&srcset[candidate_start..index]) {
                return true;
            }
            candidate_start = index + 1;
            data_url_target = starts_with_data_url(&srcset[candidate_start..]);
            target_complete = !data_url_target;
        }
    }

    srcset_candidate_is_package_local(&srcset[candidate_start..])
}

fn starts_with_data_url(candidate: &str) -> bool {
    candidate
        .trim_start()
        .get(..5)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("data:"))
}

fn srcset_candidate_is_package_local(candidate: &str) -> bool {
    let Some(target) = candidate.split_whitespace().next() else {
        return false;
    };
    let target = target.trim_matches(['\'', '"']);
    !target.is_empty() && !is_external_reference(target)
}

fn is_script_data_block(script_type: Option<&str>) -> bool {
    let Some(script_type) = script_type.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };
    !matches!(
        script_type.to_ascii_lowercase().as_str(),
        "text/javascript"
            | "application/javascript"
            | "text/ecmascript"
            | "application/ecmascript"
            | "module"
    )
}

/// Validate viewport metadata in a document-local XHTML transport channel.
/// Viewport declarations are not publication-wide metadata and therefore do
/// not participate in original-resolution/EXTH 126 projection.
pub(super) fn validate_viewport(source: &str) -> Result<()> {
    let mut reader = Reader::from_reader(Cursor::new(source.as_bytes()));
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Start(event) | Event::Empty(event)
                if local_name_ref(event.name().as_ref()).eq_ignore_ascii_case("meta")
                    && attr(&event, "name")
                        .is_some_and(|value| value.trim().eq_ignore_ascii_case("viewport")) =>
            {
                validate_viewport_attributes(&event)?;
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    Ok(())
}

fn validate_viewport_attributes(event: &BytesStart<'_>) -> Result<()> {
    let name_count = event
        .attributes()
        .flatten()
        .filter(|attribute| local_name_ref(attribute.key.as_ref()).eq_ignore_ascii_case("name"))
        .count();
    if name_count != 1 {
        return Err(viewport_error(
            "viewport must have exactly one name attribute",
        ));
    }
    let content = event.attributes().flatten().find_map(|attribute| {
        (local_name_ref(attribute.key.as_ref()).eq_ignore_ascii_case("content")).then(|| {
            attribute
                .unescape_value()
                .ok()
                .map(|value| value.into_owned())
        })
    });
    let content = content
        .flatten()
        .ok_or_else(|| viewport_error("viewport must have exactly one content attribute"))?;
    let content_count = event
        .attributes()
        .flatten()
        .filter(|attribute| local_name_ref(attribute.key.as_ref()).eq_ignore_ascii_case("content"))
        .count();
    if content_count != 1 {
        return Err(viewport_error(
            "viewport must have exactly one content attribute",
        ));
    }

    let normalized = content
        .replace('=', " = ")
        .chars()
        .map(|character| {
            if matches!(character, ',' | ';') || character.is_ascii_whitespace() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    let tokens = normalized.split_whitespace().collect::<Vec<_>>();
    if tokens.is_empty() || tokens.len() % 3 != 0 {
        return Err(viewport_error(
            "viewport content has a malformed dimension token",
        ));
    }
    let mut width_seen = false;
    let mut height_seen = false;
    for token in tokens.chunks_exact(3) {
        let key = token[0].to_ascii_lowercase();
        let value = token[2].to_ascii_lowercase();
        if token[1] != "=" || key.is_empty() || value.is_empty() {
            return Err(viewport_error(
                "viewport content has a malformed dimension token",
            ));
        }
        if key == "width" || key == "height" {
            let valid_value = ((key == "width" && value == "device-width")
                || (key == "height" && value == "device-height"))
                || value
                    .parse::<f64>()
                    .ok()
                    .is_some_and(|dimension| dimension.is_finite() && dimension > 0.0);
            if !valid_value {
                return Err(viewport_error(
                    "viewport dimensions must be positive numbers or device dimensions",
                ));
            }
            let seen = if key == "width" {
                &mut width_seen
            } else {
                &mut height_seen
            };
            if *seen {
                return Err(viewport_error("viewport dimensions must not be duplicated"));
            }
            *seen = true;
        }
    }
    if !width_seen || !height_seen {
        return Err(viewport_error(
            "viewport must provide both width and height dimensions",
        ));
    };
    Ok(())
}

fn viewport_error(detail: &str) -> Error {
    Error::UnsupportedEpub(format!("unsupported fixed-page viewport: {detail}"))
}

pub(super) fn infer_layout(
    styles: &Styles,
    page_progression: PageProgression,
    primary_writing_mode: Option<WritingMode>,
    document_writing_modes: &[Option<WritingMode>],
) -> Layout {
    let mut layout = Layout {
        page_progression,
        ..Layout::default()
    };
    if let Some(writing_mode) =
        primary_writing_mode.or_else(|| dominant_writing_mode(document_writing_modes))
    {
        layout.writing_mode = writing_mode;
    } else {
        layout.writing_mode = css_fallback_writing_mode(styles);
    }
    layout.direction = styles
        .computed
        .iter()
        .filter(|style| !selector_is_root_only(&style.selector) && style.direction.is_some())
        .find_map(|style| style.direction)
        .or_else(|| styles.computed.iter().find_map(|style| style.direction))
        .unwrap_or_default();
    layout
}

fn css_fallback_writing_mode(styles: &Styles) -> WritingMode {
    // A nested `.vrtl` rule describes the dominant body/title context even
    // when a title or navigation document also supplies an `html.hltr` rule.
    // Root-only html selectors are therefore a fallback, not an override for
    // body-context declarations.
    let writing_mode = styles
        .computed
        .iter()
        .filter(|style| !selector_is_root_only(&style.selector) && style.writing_mode.is_some())
        .find_map(|style| style.writing_mode)
        .or_else(|| styles.computed.iter().find_map(|style| style.writing_mode));
    writing_mode.unwrap_or_default()
}

fn dominant_writing_mode(document_writing_modes: &[Option<WritingMode>]) -> Option<WritingMode> {
    let mut counts = [0usize; 3];
    for writing_mode in document_writing_modes.iter().flatten() {
        counts[writing_mode_index(*writing_mode)] += 1;
    }
    let max = *counts.iter().max()?;
    if max == 0 || counts.iter().filter(|count| **count == max).count() != 1 {
        return None;
    }
    Some(
        match counts.iter().position(|count| *count == max).unwrap() {
            0 => WritingMode::HorizontalTb,
            1 => WritingMode::VerticalRl,
            _ => WritingMode::VerticalLr,
        },
    )
}

fn writing_mode_index(writing_mode: WritingMode) -> usize {
    match writing_mode {
        WritingMode::HorizontalTb => 0,
        WritingMode::VerticalRl => 1,
        WritingMode::VerticalLr => 2,
    }
}

#[allow(dead_code)]
pub(super) fn document_root_writing_mode(source: &str) -> Option<WritingMode> {
    combined_xhtml_semantics(source)
        .ok()
        .and_then(|(_, writing_mode)| writing_mode)
}

pub(super) fn parse_xhtml_semantics_and_document_root_writing_mode(
    source: &str,
) -> Result<(SemanticDocument, Option<WritingMode>)> {
    combined_xhtml_semantics(source)
}

fn root_attribute_writing_mode(event: &BytesStart<'_>) -> Option<WritingMode> {
    let style = attr(event, "style")
        .unwrap_or_default()
        .to_ascii_lowercase();
    style_value(&style, "writing-mode")
        .or_else(|| style_value(&style, "-webkit-writing-mode"))
        .or_else(|| style_value(&style, "-epub-writing-mode"))
        .and_then(|value| parse_writing_mode(&value))
        .or_else(|| {
            attr(event, "class")?.split_whitespace().find_map(|class| {
                match class.to_ascii_lowercase().as_str() {
                    "hltr" => Some(WritingMode::HorizontalTb),
                    "vrtl" => Some(WritingMode::VerticalRl),
                    "vltr" => Some(WritingMode::VerticalLr),
                    _ => None,
                }
            })
        })
}

pub(super) fn is_primary_writing_mode_meta(event: &BytesStart<'_>) -> bool {
    [attr(event, "property"), attr(event, "name")]
        .into_iter()
        .flatten()
        .any(|value| {
            value.split_whitespace().any(|token| {
                token
                    .rsplit(':')
                    .next()
                    .is_some_and(|local| local.eq_ignore_ascii_case("primary-writing-mode"))
            })
        })
}

pub(super) fn parse_writing_mode(value: &str) -> Option<WritingMode> {
    match value.trim().to_ascii_lowercase().as_str() {
        "horizontal-tb" | "horizontal-lr" => Some(WritingMode::HorizontalTb),
        "vertical-rl" => Some(WritingMode::VerticalRl),
        "vertical-lr" => Some(WritingMode::VerticalLr),
        _ => None,
    }
}

pub(super) fn selector_is_root_only(selector: &str) -> bool {
    let selectors = selector
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let mut has_selector = false;
    let all_root_only = selectors.fold(true, |all_root_only, selector| {
        has_selector = true;
        let selector = selector.to_ascii_lowercase();
        let root_only = selector.strip_prefix("html").is_some_and(|remainder| {
            remainder.is_empty()
                || (remainder
                    .chars()
                    .next()
                    .is_some_and(|character| matches!(character, '.' | '#' | ':' | '['))
                    && !remainder.chars().any(|character| {
                        character.is_whitespace() || matches!(character, '>' | '+' | '~')
                    }))
        });
        all_root_only && root_only
    });
    has_selector && all_root_only
}

#[derive(Debug, Clone)]
pub(super) struct DocumentStyle {
    pub(super) reference: String,
    pub(super) resource_href: Option<String>,
    pub(super) inline_source: Option<String>,
}

/// Discover a document's stylesheet inputs in source order. Inline styles get
/// synthetic, document-local hrefs so the existing CSS graph can treat them as
/// ordinary scoped resources without adding a manifest-level resource.
pub(super) fn document_styles_with_occupied_hrefs(
    source: &str,
    document_href: &str,
    occupied_hrefs: &mut HashSet<String>,
) -> Result<Vec<DocumentStyle>> {
    let mut result = Vec::new();
    let mut seen_links = HashSet::new();
    let mut inline_index = 0usize;
    let mut cursor = 0usize;
    while cursor < source.len() {
        let Some(relative) = source[cursor..].find('<') else {
            break;
        };
        let start = cursor + relative;
        if source[start..].starts_with("<!--") {
            cursor = source[start + 4..]
                .find("-->")
                .map_or(source.len(), |end| start + 4 + end + 3);
            continue;
        }
        let Some(tag_end) = html_tag_end(source, start) else {
            return Err(Error::InvalidXhtmlCss(
                "malformed XHTML tag while scanning CSS resources".to_owned(),
            ));
        };
        let Some((name_start, name_end, closing)) = html_tag_name_range(source, start, tag_end)
        else {
            cursor = tag_end + 1;
            continue;
        };
        if closing {
            cursor = tag_end + 1;
            continue;
        }
        let name = &source[name_start..name_end];
        if let Some(style) = tag_attribute(source, start, tag_end, "style")? {
            validate_kf8_inline_style(&style)?;
        }
        if html_local_name_is(name, "script") {
            if source[..tag_end].trim_end().ends_with('/') {
                cursor = tag_end + 1;
                continue;
            }
            cursor = raw_text_end(source, tag_end, "script").unwrap_or(source.len());
            continue;
        }
        if html_local_name_is(name, "style") {
            let self_closing = source[..tag_end].trim_end().ends_with('/');
            if self_closing {
                cursor = tag_end + 1;
                continue;
            }
            let Some((close_start, close_end)) = closing_tag(source, tag_end + 1, "style") else {
                return Err(Error::InvalidXhtmlCss(
                    "inline style element has no closing tag".to_owned(),
                ));
            };
            validate_kf8_css(&source[tag_end + 1..close_start])?;
            let (resource_href, reference) =
                inline_style_href_with_occupied_hrefs(document_href, inline_index, occupied_hrefs);
            occupied_hrefs.insert(normalize_path(&resource_href));
            result.push(DocumentStyle {
                reference,
                resource_href: Some(resource_href),
                inline_source: Some(source[tag_end + 1..close_start].to_owned()),
            });
            inline_index += 1;
            cursor = close_end + 1;
            continue;
        }
        if html_local_name_is(name, "link")
            && tag_attribute(source, start, tag_end, "rel")?
                .is_some_and(|value| is_stylesheet_rel(&value))
        {
            if let Some(href) = tag_attribute(source, start, tag_end, "href")? {
                if seen_links.insert(href.clone()) {
                    result.push(DocumentStyle {
                        reference: href,
                        resource_href: None,
                        inline_source: None,
                    });
                }
            }
        }
        cursor = tag_end + 1;
    }
    Ok(result)
}

pub(super) fn unique_resource_id(base_id: &str, occupied_ids: &HashSet<String>) -> String {
    let mut resource_id = base_id.to_owned();
    let mut collision_index = 0usize;
    while occupied_ids.contains(&resource_id) {
        collision_index += 1;
        resource_id = format!("{base_id}-collision-{collision_index:04}");
    }
    resource_id
}

fn tag_attribute(
    source: &str,
    start: usize,
    tag_end: usize,
    wanted: &str,
) -> Result<Option<String>> {
    let (_, mut cursor, _) = html_tag_name_range(source, start, tag_end)
        .ok_or_else(|| Error::InvalidXhtmlCss("malformed XHTML tag attributes".to_owned()))?;
    let bytes = source.as_bytes();
    let mut result = None;
    while cursor < tag_end {
        while cursor < tag_end && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= tag_end || bytes[cursor] == b'/' {
            break;
        }
        let name_start = cursor;
        while cursor < tag_end
            && !bytes[cursor].is_ascii_whitespace()
            && !matches!(bytes[cursor], b'=' | b'/' | b'>')
        {
            cursor += source[cursor..].chars().next().map_or(1, char::len_utf8);
        }
        let name_end = cursor;
        if name_start == name_end {
            cursor += 1;
            continue;
        }
        while cursor < tag_end && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b'=') {
            if source[name_start..name_end].eq_ignore_ascii_case(wanted) {
                return Err(Error::InvalidXhtmlCss(format!(
                    "XHTML {wanted} attribute has no value"
                )));
            }
            while cursor < tag_end && !bytes[cursor].is_ascii_whitespace() && bytes[cursor] != b'/'
            {
                cursor += source[cursor..].chars().next().map_or(1, char::len_utf8);
            }
            continue;
        }
        cursor += 1;
        while cursor < tag_end && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let (value_start, value_end) = if matches!(bytes.get(cursor), Some(b'"') | Some(b'\'')) {
            let quote = bytes[cursor];
            let value_start = cursor + 1;
            let Some(relative_end) = source[value_start..tag_end].find(quote as char) else {
                return Err(Error::InvalidXhtmlCss(
                    "XHTML attribute has an unterminated quoted value".to_owned(),
                ));
            };
            let value_end = value_start + relative_end;
            cursor = value_end + 1;
            (value_start, value_end)
        } else {
            let value_start = cursor;
            while cursor < tag_end && !bytes[cursor].is_ascii_whitespace() {
                cursor += source[cursor..].chars().next().map_or(1, char::len_utf8);
            }
            (value_start, cursor)
        };
        if source[name_start..name_end].eq_ignore_ascii_case(wanted) {
            if result.is_some() && wanted.eq_ignore_ascii_case("style") {
                return Err(Error::InvalidXhtmlCss(
                    "XHTML element has duplicate style attributes".to_owned(),
                ));
            }
            result = Some(source[value_start..value_end].to_owned());
        }
    }
    Ok(result)
}

fn is_stylesheet_rel(value: &str) -> bool {
    value
        .split_whitespace()
        .any(|token| token.eq_ignore_ascii_case("stylesheet"))
}

fn raw_text_end(source: &str, tag_end: usize, name: &str) -> Option<usize> {
    closing_tag(source, tag_end + 1, name).map(|(_, end)| end + 1)
}

fn closing_tag(source: &str, start: usize, name: &str) -> Option<(usize, usize)> {
    let bytes = source.as_bytes();
    let mut cursor = start;
    let mut quote = None;
    let mut block_comment = false;
    let mut line_comment = false;
    let mut html_comment = false;
    while cursor < bytes.len() {
        let byte = bytes[cursor];
        if html_comment {
            if byte == b'-'
                && bytes.get(cursor + 1) == Some(&b'-')
                && bytes.get(cursor + 2) == Some(&b'>')
            {
                html_comment = false;
                cursor += 3;
            } else {
                cursor = advance_raw_text_char(source, cursor);
            }
            continue;
        }
        if block_comment {
            if byte == b'*' && bytes.get(cursor + 1) == Some(&b'/') {
                block_comment = false;
                cursor += 2;
            } else {
                cursor = advance_raw_text_char(source, cursor);
            }
            continue;
        }
        if line_comment {
            if byte == b'\r' || byte == b'\n' {
                line_comment = false;
            }
            cursor = advance_raw_text_char(source, cursor);
            continue;
        }
        if let Some(delimiter) = quote {
            if byte == b'\\' {
                cursor = advance_raw_text_char(source, cursor);
                if cursor < bytes.len() {
                    cursor = advance_raw_text_char(source, cursor);
                }
            } else {
                if byte == delimiter {
                    quote = None;
                }
                cursor = advance_raw_text_char(source, cursor);
            }
            continue;
        }
        if byte == b'<'
            && bytes.get(cursor + 1) == Some(&b'!')
            && bytes.get(cursor + 2) == Some(&b'-')
            && bytes.get(cursor + 3) == Some(&b'-')
        {
            html_comment = true;
            cursor += 4;
            continue;
        }
        if byte == b'/' && bytes.get(cursor + 1) == Some(&b'*') {
            block_comment = true;
            cursor += 2;
            continue;
        }
        if name.eq_ignore_ascii_case("script")
            && byte == b'/'
            && bytes.get(cursor + 1) == Some(&b'/')
        {
            line_comment = true;
            cursor += 2;
            continue;
        }
        if matches!(byte, b'\'' | b'"' | b'`') {
            quote = Some(byte);
            cursor = advance_raw_text_char(source, cursor);
            continue;
        }
        if byte == b'<' && bytes.get(cursor + 1) == Some(&b'/') {
            let close_start = cursor;
            let tag_end = html_tag_end(source, close_start)?;
            if let Some((name_start, name_end, closing)) =
                html_tag_name_range(source, close_start, tag_end)
            {
                if closing && html_local_name_is(&source[name_start..name_end], name) {
                    return Some((close_start, tag_end));
                }
            }
            cursor = tag_end + 1;
            continue;
        }
        cursor = advance_raw_text_char(source, cursor);
    }
    None
}

fn advance_raw_text_char(source: &str, cursor: usize) -> usize {
    source
        .get(cursor..)
        .and_then(|remaining| remaining.chars().next())
        .map_or(source.len(), |character| cursor + character.len_utf8())
}

fn style_value(style: &str, property: &str) -> Option<String> {
    style.split(';').find_map(|part| {
        let (name, value) = part.split_once(':')?;
        (name.trim() == property).then(|| value.trim().to_owned())
    })
}
