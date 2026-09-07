//! Plan CSS dependencies and project them into KF8 CSS flows.
//!
//! The module owns stylesheet ordering, `@import` traversal, asset URL
//! rewriting, and flow references. XHTML semantic parsing and KF8 record
//! serialization remain outside this boundary.

use std::collections::HashSet;

use super::builder::{SYNTHETIC_INLINE_CSS_PROPERTY, to_base32, to_base32_fixed};
use super::resource::{is_binary_resource, is_css_resource, is_font_resource, is_image_resource};
use crate::kindle::{KindleResource as Resource, KindleSection};
use crate::xhtml::path::{is_external_reference, normalize_path, resolve_path};

pub(crate) fn referenced_css_resources<'a>(
    sections: &[KindleSection],
    source_resources: &'a [Resource],
) -> Vec<&'a Resource> {
    let mut referenced = HashSet::new();
    let mut result = Vec::new();
    for section in sections {
        for style_href in &section.referenced_styles {
            visit_css_resource(
                sections,
                source_resources,
                &section.href,
                style_href,
                &mut referenced,
                &mut result,
            );
        }
    }
    result
}

fn visit_css_resource<'a>(
    sections: &[KindleSection],
    source_resources: &'a [Resource],
    base_href: &str,
    style_href: &str,
    referenced: &mut HashSet<String>,
    result: &mut Vec<&'a Resource>,
) {
    let Some(resolved) = resolve_path(base_href, style_href) else {
        return;
    };
    if !referenced.insert(resolved.clone()) {
        return;
    }
    let Some(resource) = source_resources.iter().find(|resource| {
        is_css_resource(resource)
            && normalize_path(&resource.href).is_some_and(|href| href == resolved)
    }) else {
        return;
    };
    // DFS follows each source's imports in source order. Push after visiting
    // imports so dependencies receive stable flow IDs before their parents;
    // the set prevents shared resources and cycles from producing duplicates.
    let imports = css_import_targets(&String::from_utf8_lossy(&resource.data));
    let import_base_href = css_resource_base_href(sections, resource);
    for target in imports {
        visit_css_resource(
            sections,
            source_resources,
            &import_base_href,
            &target,
            referenced,
            result,
        );
    }
    result.push(resource);
}

pub(crate) fn css_resource_base_href(sections: &[KindleSection], resource: &Resource) -> String {
    let Some(normalized_resource_href) = normalize_path(&resource.href) else {
        return resource.href.clone();
    };
    if !normalized_resource_href.starts_with("__inline_css__/")
        || !resource
            .properties
            .iter()
            .any(|property| property == SYNTHETIC_INLINE_CSS_PROPERTY)
    {
        return resource.href.clone();
    }

    let resource_href = normalized_resource_href.as_str();
    sections
        .iter()
        .find(|section| {
            section.referenced_styles.iter().any(|style_href| {
                resolve_path(&section.href, style_href)
                    .is_some_and(|resolved| resolved == resource_href)
            })
        })
        .map(|section| section.href.clone())
        .unwrap_or_else(|| resource.href.clone())
}

pub(crate) fn rewrite_css_assets(
    source: &[u8],
    css_href: &str,
    resources: &[Resource],
    css_resources: &[&Resource],
) -> Vec<u8> {
    let Ok(source) = std::str::from_utf8(source) else {
        // Invalid CSS cannot be scanned safely as text. Preserve its raw
        // bytes rather than corrupting transport through replacement chars.
        return source.to_vec();
    };
    let source = rewrite_css_imports(source, css_href, css_resources);
    let mut result = String::with_capacity(source.len());
    let mut cursor = 0;
    for url in css_url_spans(&source) {
        let target = &source[url.target_start..url.target_end];
        let Some(reference) = resource_reference(css_href, target, resources) else {
            continue;
        };
        result.push_str(&source[cursor..url.start]);
        if url.preserve_wrappers {
            // Preserve whitespace, comments, and wrappers around a real URL,
            // while removing only source quote delimiters.
            let quote_offset = usize::from(url.quote.is_some());
            result.push_str(&source[url.start..url.target_start - quote_offset]);
            result.push_str(&reference);
            result.push_str(&source[url.target_end + quote_offset..url.close_end]);
        } else {
            result.push_str("url(");
            result.push_str(&reference);
            result.push(')');
        }
        cursor = url.close_end;
    }
    result.push_str(&source[cursor..]);
    result.into_bytes()
}

#[derive(Debug, Clone, Copy)]
struct CssUrlSpan {
    start: usize,
    target_start: usize,
    target_end: usize,
    close_end: usize,
    quote: Option<u8>,
    preserve_wrappers: bool,
}

#[derive(Debug, Clone, Copy)]
struct CssImportSpan {
    wrapper_start: usize,
    wrapper_end: usize,
    target_start: usize,
    target_end: usize,
    scan_end: usize,
}

fn css_url_spans(source: &str) -> Vec<CssUrlSpan> {
    let bytes = source.as_bytes();
    let mut spans = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor] == b'/' && bytes.get(cursor + 1) == Some(&b'*') {
            cursor = skip_css_comment(source, cursor).unwrap_or(bytes.len());
            continue;
        }
        if matches!(bytes[cursor], b'\'' | b'\"') {
            cursor = skip_css_string(source, cursor).unwrap_or(bytes.len());
            continue;
        }
        if css_function_at(source, cursor, "url") && bytes.get(cursor + 3) == Some(&b'(') {
            if let Some(span) = parse_css_url_span(source, cursor) {
                cursor = span.close_end;
                spans.push(span);
                continue;
            }
        }
        cursor = advance_css_char(source, cursor);
    }
    spans
}

fn parse_css_url_span(source: &str, start: usize) -> Option<CssUrlSpan> {
    let open = start.checked_add(3)?;
    if source.as_bytes().get(open) != Some(&b'(') {
        return None;
    }
    let (mut cursor, mut preserve_wrappers) = skip_css_space_comments(source, open + 1)?;
    if source
        .as_bytes()
        .get(cursor)
        .is_some_and(|byte| *byte == b'\'' || *byte == b'\"')
    {
        let quote_end = skip_css_string(source, cursor)?;
        let target_start = cursor + 1;
        let target_end = quote_end.checked_sub(1)?;
        let (close, trailing_comments) = skip_css_space_comments(source, quote_end)?;
        preserve_wrappers |= trailing_comments;
        if source.as_bytes().get(close) != Some(&b')') {
            return None;
        }
        return Some(CssUrlSpan {
            start,
            target_start,
            target_end,
            close_end: close + 1,
            quote: Some(source.as_bytes()[cursor]),
            preserve_wrappers,
        });
    }

    let target_start = cursor;
    while cursor < source.len() {
        let byte = source.as_bytes()[cursor];
        if byte == b')' {
            break;
        }
        if byte.is_ascii_whitespace()
            || (byte == b'/' && source.as_bytes().get(cursor + 1) == Some(&b'*'))
        {
            break;
        }
        cursor = advance_css_char(source, cursor);
    }
    let target_end = cursor;
    if target_end == target_start {
        return None;
    }
    let (close, trailing_comments) = skip_css_space_comments(source, cursor)?;
    preserve_wrappers |= trailing_comments;
    if source.as_bytes().get(close) != Some(&b')') {
        return None;
    }
    Some(CssUrlSpan {
        start,
        target_start,
        target_end,
        close_end: close + 1,
        quote: None,
        preserve_wrappers,
    })
}

fn rewrite_css_imports(source: &str, css_href: &str, css_resources: &[&Resource]) -> String {
    let mut result = String::with_capacity(source.len());
    let mut cursor = 0;
    for import in css_import_spans(source) {
        let target = &source[import.target_start..import.target_end];
        let Some(flow_number) = css_flow_number(css_href, target, css_resources) else {
            continue;
        };
        // Keep @import rather than flattening it: the CSS resource graph,
        // import order, and media/query suffix are all transport contracts,
        // while only the local target wrapper needs a flow address. KindleGen
        // transport uses the canonical unquoted url() wrapper for local CSS.
        result.push_str(&source[cursor..import.wrapper_start]);
        result.push_str("url(");
        result.push_str(&stylesheet_flow_reference(flow_number));
        result.push(')');
        cursor = import.wrapper_end;
    }
    result.push_str(&source[cursor..]);
    result
}

fn css_import_spans(source: &str) -> Vec<CssImportSpan> {
    let bytes = source.as_bytes();
    let mut spans = Vec::new();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor] == b'/' && bytes.get(cursor + 1) == Some(&b'*') {
            cursor = skip_css_comment(source, cursor).unwrap_or(bytes.len());
            continue;
        }
        if matches!(bytes[cursor], b'\'' | b'\"') {
            cursor = skip_css_string(source, cursor).unwrap_or(bytes.len());
            continue;
        }
        if css_keyword_at(source, cursor, "@import") {
            if let Some(span) = parse_css_import_span(source, cursor + "@import".len()) {
                cursor = span.scan_end;
                spans.push(span);
                continue;
            }
        }
        cursor = advance_css_char(source, cursor);
    }
    spans
}

fn parse_css_import_span(source: &str, start: usize) -> Option<CssImportSpan> {
    let (cursor, _) = skip_css_space_comments(source, start)?;
    if css_function_at(source, cursor, "url") && source.as_bytes().get(cursor + 3) == Some(&b'(') {
        let url = parse_css_url_span(source, cursor)?;
        return Some(CssImportSpan {
            wrapper_start: cursor,
            wrapper_end: url.close_end,
            target_start: url.target_start,
            target_end: url.target_end,
            scan_end: url.close_end,
        });
    }
    let quote = *source.as_bytes().get(cursor)?;
    if !matches!(quote, b'\'' | b'\"') {
        return None;
    }
    let quote_end = skip_css_string(source, cursor)?;
    Some(CssImportSpan {
        wrapper_start: cursor,
        wrapper_end: quote_end,
        target_start: cursor + 1,
        target_end: quote_end.checked_sub(1)?,
        scan_end: quote_end,
    })
}

fn skip_css_space_comments(source: &str, mut cursor: usize) -> Option<(usize, bool)> {
    let mut skipped = false;
    loop {
        while source
            .as_bytes()
            .get(cursor)
            .is_some_and(|byte| byte.is_ascii_whitespace())
        {
            cursor += 1;
            skipped = true;
        }
        if source.as_bytes().get(cursor) == Some(&b'/')
            && source.as_bytes().get(cursor + 1) == Some(&b'*')
        {
            skipped = true;
            cursor = skip_css_comment(source, cursor)?;
            continue;
        }
        return Some((cursor, skipped));
    }
}

fn skip_css_comment(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    if bytes.get(start) != Some(&b'/') || bytes.get(start + 1) != Some(&b'*') {
        return None;
    }
    let mut cursor = start + 2;
    while cursor < bytes.len() {
        if bytes[cursor] == b'*' && bytes.get(cursor + 1) == Some(&b'/') {
            return Some(cursor + 2);
        }
        cursor = advance_css_char(source, cursor);
    }
    None
}

fn skip_css_string(source: &str, start: usize) -> Option<usize> {
    let quote = *source.as_bytes().get(start)?;
    if !matches!(quote, b'\'' | b'\"') {
        return None;
    }
    let mut cursor = start + 1;
    while cursor < source.len() {
        match source.as_bytes()[cursor] {
            byte if byte == quote => return Some(cursor + 1),
            b'\\' => {
                cursor = advance_css_char(source, cursor);
                if cursor < source.len() {
                    cursor = advance_css_char(source, cursor);
                }
            }
            _ => cursor = advance_css_char(source, cursor),
        }
    }
    None
}

pub(crate) fn advance_css_char(source: &str, cursor: usize) -> usize {
    source
        .get(cursor..)
        .and_then(|remaining| remaining.chars().next())
        .map_or(source.len(), |character| cursor + character.len_utf8())
}

fn css_keyword_at(source: &str, start: usize, keyword: &str) -> bool {
    let bytes = source.as_bytes();
    let end = match start.checked_add(keyword.len()) {
        Some(end) if end <= bytes.len() => end,
        _ => return false,
    };
    if !bytes[start..end]
        .iter()
        .zip(keyword.bytes())
        .all(|(actual, expected)| actual.eq_ignore_ascii_case(&expected))
    {
        return false;
    }
    source
        .get(end..)
        .and_then(|remaining| remaining.chars().next())
        .is_none_or(|character| {
            !character.is_alphanumeric() && character != '_' && character != '-'
        })
}

fn css_function_at(source: &str, start: usize, name: &str) -> bool {
    // A CSS function name is an identifier token. Keep myurl(...) intact:
    // splitting the suffix "url(...)" would rewrite text that is not a URL.
    css_keyword_at(source, start, name) && css_identifier_boundary_before(source, start)
}

fn css_identifier_boundary_before(source: &str, start: usize) -> bool {
    source
        .get(..start)
        .and_then(|prefix| prefix.chars().next_back())
        .is_none_or(|character| {
            !character.is_alphanumeric() && character != '_' && character != '-'
        })
}

fn css_import_targets(source: &str) -> Vec<String> {
    css_import_spans(source)
        .into_iter()
        .map(|span| source[span.target_start..span.target_end].to_owned())
        .filter(|target| !is_external_reference(target))
        .collect()
}

pub(crate) fn resource_reference(
    base_href: &str,
    target: &str,
    resources: &[Resource],
) -> Option<String> {
    let resolved = resolve_path(base_href, target)?;
    let mut first_image = None;
    for (binary_index, resource) in resources
        .iter()
        .filter(|resource| is_binary_resource(resource))
        .enumerate()
    {
        if first_image.is_none() && is_image_resource(resource) {
            first_image = Some(binary_index);
        }
        let matches = normalize_path(&resource.href).is_some_and(|href| {
            href == resolved
                || resolved
                    .strip_suffix(&href)
                    .is_some_and(|prefix| prefix.ends_with('/'))
        });
        if matches {
            let embed_index = match first_image {
                Some(first_image) if binary_index >= first_image => binary_index - first_image + 1,
                Some(_) => return None,
                None if is_font_resource(resource) => binary_index + 1,
                None => return None,
            };
            return Some(format!(
                "kindle:embed:{}?mime={}",
                to_base32(u32::try_from(embed_index).ok()?),
                resource.media_type
            ));
        }
    }
    None
}

pub(crate) fn stylesheet_flow_reference(flow_number: u32) -> String {
    format!(
        "kindle:flow:{}?mime=text/css",
        to_base32_fixed(flow_number, 4).expect("CSS flow number fits in four digits")
    )
}

pub(crate) fn css_flow_number(
    base_href: &str,
    target: &str,
    css_resources: &[&Resource],
) -> Option<u32> {
    let resolved = resolve_path(base_href, target)?;
    let index = css_resources
        .iter()
        .position(|resource| normalize_path(&resource.href).is_some_and(|href| href == resolved))?;
    u32::try_from(index.checked_add(1)?).ok()
}
