//! Extract semantic information from source EPUB XHTML documents.
//!
//! This includes document styles, writing-mode hints, links, and Aozora
//! semantic elements. KF8-specific XHTML rewriting belongs to `kf8::rawml`.

use std::collections::HashSet;
use std::io::Cursor;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use super::navigation::has_token;
use super::opf::{attr, local_name_ref};
use crate::book::{Layout, PageProgression, SemanticDocument, Styles, WritingMode};
use crate::error::Result;
use crate::xhtml::path::normalize_path_lossy as normalize_path;
use crate::xhtml::scan::{
    html_local_name_is_text as html_local_name_is, html_tag_end,
    html_tag_name_range_with_leading_space as html_tag_name_range,
};
pub(super) fn parse_xhtml_semantics(source: &str) -> Result<SemanticDocument> {
    let mut reader = Reader::from_reader(Cursor::new(source.as_bytes()));
    let mut buffer = Vec::new();
    let mut result = SemanticDocument::default();
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
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    Ok(result)
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

pub(super) fn document_root_writing_mode(source: &str) -> Option<WritingMode> {
    let mut reader = Reader::from_reader(Cursor::new(source.as_bytes()));
    let mut buffer = Vec::new();
    let mut html_mode = None;
    let mut body_mode = None;
    loop {
        match reader.read_event_into(&mut buffer) {
            Ok(Event::Start(event)) | Ok(Event::Empty(event)) => {
                let event_name = event.name();
                let name = local_name_ref(event_name.as_ref());
                if name.eq_ignore_ascii_case("html") && html_mode.is_none() {
                    html_mode = root_attribute_writing_mode(&event);
                } else if name.eq_ignore_ascii_case("body") && body_mode.is_none() {
                    body_mode = root_attribute_writing_mode(&event);
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return None,
            _ => {}
        }
        buffer.clear();
    }
    html_mode.or(body_mode)
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
) -> Vec<DocumentStyle> {
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
                break;
            };
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
            && tag_attribute(source, start, tag_end, "rel")
                .is_some_and(|value| is_stylesheet_rel(&value))
        {
            if let Some(href) = tag_attribute(source, start, tag_end, "href") {
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
    result
}

pub(super) fn inline_style_href_with_occupied_hrefs(
    document_href: &str,
    index: usize,
    occupied_hrefs: &HashSet<String>,
) -> (String, String) {
    let document_token = inline_document_token(document_href);
    let stem = format!("__inline_css__/doc-{document_token}-style-{index:04}");
    let mut resource_href = format!("{stem}.css");
    let mut collision_index = 0usize;
    while occupied_hrefs.contains(&normalize_path(&resource_href)) {
        collision_index += 1;
        resource_href = format!("{stem}-collision-{collision_index:04}.css");
    }
    let reference = relative_resource_reference(document_href, &resource_href);
    (resource_href, reference)
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

fn inline_document_token(document_href: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let normalized = normalize_path(document_href);
    if normalized.is_empty() {
        return "root".to_owned();
    }
    let mut token = String::with_capacity(normalized.len() * 2);
    for byte in normalized.bytes() {
        token.push(HEX[(byte >> 4) as usize] as char);
        token.push(HEX[(byte & 0x0f) as usize] as char);
    }
    token
}

fn relative_resource_reference(document_href: &str, resource_href: &str) -> String {
    let normalized = normalize_path(document_href);
    let directory = normalized
        .rsplit_once('/')
        .map(|(directory, _)| directory)
        .unwrap_or_default();
    let parent_prefix = "../".repeat(directory.split('/').filter(|part| !part.is_empty()).count());
    format!("{parent_prefix}{resource_href}")
}

fn tag_attribute(source: &str, start: usize, tag_end: usize, wanted: &str) -> Option<String> {
    let (_, mut cursor, _) = html_tag_name_range(source, start, tag_end)?;
    let bytes = source.as_bytes();
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
            cursor += source[cursor..].chars().next()?.len_utf8();
        }
        let name_end = cursor;
        while cursor < tag_end && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if bytes.get(cursor) != Some(&b'=') {
            continue;
        }
        cursor += 1;
        while cursor < tag_end && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let (value_start, value_end) = if matches!(bytes.get(cursor), Some(b'"') | Some(b'\'')) {
            let quote = bytes[cursor];
            let value_start = cursor + 1;
            let value_end = value_start + source[value_start..tag_end].find(quote as char)?;
            cursor = value_end + 1;
            (value_start, value_end)
        } else {
            let value_start = cursor;
            while cursor < tag_end && !bytes[cursor].is_ascii_whitespace() {
                cursor += source[cursor..].chars().next()?.len_utf8();
            }
            (value_start, cursor)
        };
        if source[name_start..name_end].eq_ignore_ascii_case(wanted) {
            return Some(source[value_start..value_end].to_owned());
        }
    }
    None
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
