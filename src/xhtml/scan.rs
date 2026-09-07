//! Low-level, syntax-only XHTML/HTML scanning primitives.
//!
//! The scanner handles tag boundaries, names, attributes, and raw-text
//! elements. Semantic interpretation such as cover, stylesheet, layout, or
//! position-bearing status belongs to callers.

pub(crate) fn advance_char(source: &str, cursor: usize) -> usize {
    source
        .get(cursor..)
        .and_then(|remaining| remaining.chars().next())
        .map_or(source.len(), |character| cursor + character.len_utf8())
}

/// Find an ASCII needle without allocating a lowercased copy of `source`.
///
/// This has the same case-folding scope as `str::to_ascii_lowercase`: bytes
/// outside ASCII are compared unchanged and ASCII letters are compared
/// case-insensitively.
pub(crate) fn find_ascii_case_insensitive(
    source: &str,
    needle: &str,
    start: usize,
) -> Option<usize> {
    let needle = needle.as_bytes();
    if needle.is_empty() {
        return Some(start.min(source.len()));
    }
    source
        .as_bytes()
        .get(start..)?
        .windows(needle.len())
        .position(|window| ascii_bytes_eq_ignore_case(window, needle))
        .map(|relative| start + relative)
}

pub(crate) fn contains_any_ascii_case_insensitive(source: &str, needles: &[&str]) -> bool {
    let bytes = source.as_bytes();
    bytes.iter().enumerate().any(|(start, _)| {
        needles.iter().any(|needle| {
            let needle = needle.as_bytes();
            !needle.is_empty()
                && bytes
                    .get(start..start.saturating_add(needle.len()))
                    .is_some_and(|window| ascii_bytes_eq_ignore_case(window, needle))
        })
    })
}

pub(crate) fn contains_ascii_case_insensitive_bytes(source: &[u8], needle: &[u8]) -> bool {
    if needle.is_empty() {
        return true;
    }
    source
        .windows(needle.len())
        .any(|window| ascii_bytes_eq_ignore_case(window, needle))
}

fn ascii_bytes_eq_ignore_case(left: &[u8], right: &[u8]) -> bool {
    left.len() == right.len()
        && left
            .iter()
            .zip(right)
            .all(|(&left, &right)| left.eq_ignore_ascii_case(&right))
}

pub(crate) fn html_tag_end(source: &str, start: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let mut cursor = start + 1;
    let mut quote = None;
    while cursor < bytes.len() {
        match (quote, bytes[cursor]) {
            (Some(expected), byte) if byte == expected => quote = None,
            (Some(_), _) => {}
            (None, b'"' | b'\'') => quote = Some(bytes[cursor]),
            (None, b'>') => return Some(cursor),
            (None, _) => {}
        }
        cursor = advance_char(source, cursor);
    }
    None
}

pub(crate) fn html_tag_name_range(
    source: &str,
    start: usize,
    tag_end: usize,
) -> Option<(usize, usize, bool)> {
    let bytes = source.as_bytes();
    let mut cursor = start + 1;
    let closing = bytes.get(cursor) == Some(&b'/');
    if closing {
        cursor += 1;
    }
    if cursor >= tag_end || matches!(bytes[cursor], b'!' | b'?') {
        return None;
    }
    let name_start = cursor;
    while cursor < tag_end
        && !bytes[cursor].is_ascii_whitespace()
        && !matches!(bytes[cursor], b'/' | b'>')
    {
        cursor = advance_char(source, cursor);
    }
    (name_start < cursor).then_some((name_start, cursor, closing))
}

pub(crate) fn html_tag_name_range_with_leading_space(
    source: &str,
    start: usize,
    tag_end: usize,
) -> Option<(usize, usize, bool)> {
    let bytes = source.as_bytes();
    let mut cursor = start + 1;
    let closing = bytes.get(cursor) == Some(&b'/');
    if closing {
        cursor += 1;
    }
    while cursor < tag_end && bytes[cursor].is_ascii_whitespace() {
        cursor += 1;
    }
    if cursor >= tag_end || matches!(bytes[cursor], b'!' | b'?') {
        return None;
    }
    let name_start = cursor;
    while cursor < tag_end
        && !bytes[cursor].is_ascii_whitespace()
        && !matches!(bytes[cursor], b'/' | b'>')
    {
        cursor = advance_char(source, cursor);
    }
    (name_start < cursor).then_some((name_start, cursor, closing))
}

pub(crate) fn html_local_name_is(source: &str, start: usize, end: usize, wanted: &str) -> bool {
    source[start..end]
        .rsplit(':')
        .next()
        .is_some_and(|name| name.eq_ignore_ascii_case(wanted))
}

pub(crate) fn html_local_name_is_text(name: &str, wanted: &str) -> bool {
    name.rsplit(':')
        .next()
        .is_some_and(|local| local.eq_ignore_ascii_case(wanted))
}

pub(crate) fn html_raw_text_end(source: &str, start: usize, tag_end: usize) -> Option<usize> {
    let bytes = source.as_bytes();
    let (name_start, name_end, closing) = html_tag_name_range(source, start, tag_end)?;
    if closing {
        return None;
    }
    let mut content_end = tag_end;
    while content_end > start && bytes[content_end - 1].is_ascii_whitespace() {
        content_end -= 1;
    }
    if bytes.get(content_end - 1) == Some(&b'/') {
        return None;
    }
    let raw_name = if html_local_name_is(source, name_start, name_end, "script") {
        "script"
    } else if html_local_name_is(source, name_start, name_end, "style") {
        "style"
    } else {
        return None;
    };
    let mut cursor = tag_end + 1;
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
                cursor = advance_char(source, cursor);
            }
            continue;
        }
        if block_comment {
            if byte == b'*' && bytes.get(cursor + 1) == Some(&b'/') {
                block_comment = false;
                cursor += 2;
            } else {
                cursor = advance_char(source, cursor);
            }
            continue;
        }
        if line_comment {
            if byte == b'\r' || byte == b'\n' {
                line_comment = false;
            }
            cursor = advance_char(source, cursor);
            continue;
        }
        if let Some(delimiter) = quote {
            if byte == b'\\' {
                cursor = advance_char(source, cursor);
                if cursor < bytes.len() {
                    cursor = advance_char(source, cursor);
                }
            } else {
                if byte == delimiter {
                    quote = None;
                }
                cursor = advance_char(source, cursor);
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
        if raw_name == "script" && byte == b'/' && bytes.get(cursor + 1) == Some(&b'/') {
            line_comment = true;
            cursor += 2;
            continue;
        }
        if matches!(byte, b'\'' | b'"' | b'`') {
            quote = Some(byte);
            cursor = advance_char(source, cursor);
            continue;
        }
        if byte == b'<' && bytes.get(cursor + 1) == Some(&b'/') {
            let Some(candidate_end) = html_tag_end(source, cursor) else {
                return Some(source.len());
            };
            if let Some((candidate_start, candidate_name_end, candidate_closing)) =
                html_tag_name_range(source, cursor, candidate_end)
            {
                if candidate_closing
                    && html_local_name_is(source, candidate_start, candidate_name_end, raw_name)
                {
                    return Some(candidate_end + 1);
                }
            }
            cursor = candidate_end + 1;
            continue;
        }
        cursor = advance_char(source, cursor);
    }
    Some(source.len())
}
