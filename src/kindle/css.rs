/// Project the AozoraEpub3 CSS subset observed in KindleGen output. This is a
/// semantic stage: it only changes declarations in parsed CSS blocks and does
/// not resolve resources, imports, URLs, or stylesheet scope.
pub fn project_css_for_kindle(source: &str) -> String {
    project_css_range(source, 0, source.len())
}

fn project_css_range(source: &str, start: usize, end: usize) -> String {
    let Some(open) = find_top_level_open(source, start, end) else {
        return source[start..end].to_owned();
    };
    let Some(close) = matching_brace(source, open, end) else {
        return source[start..end].to_owned();
    };
    let prelude = &source[start..open];
    let body = &source[open + 1..close];
    let mut result = String::with_capacity(end - start);
    result.push_str(prelude);
    result.push('{');
    // A stylesheet may begin with statement at-rules such as @charset and
    // @namespace before its first style rule. Only the portion after the
    // final statement separator belongs to the block prelude; otherwise the
    // leading @charset would make every following declaration look nested
    // and silently skip projection.
    let block_prelude = prelude
        .rsplit_once(';')
        .map_or(prelude, |(_, remainder)| remainder);
    if block_prelude.trim_start().starts_with('@')
        || find_top_level_open(source, open + 1, close).is_some()
    {
        result.push_str(&project_css_range(source, open + 1, close));
    } else {
        result.push_str(&project_declarations(body, prelude));
    }
    result.push('}');
    result.push_str(&project_css_range(source, close + 1, end));
    result
}

fn project_declarations(source: &str, selector: &str) -> String {
    let remove_measure = is_aozora_measure_selector(selector);
    let mut result = String::with_capacity(source.len());
    let mut segment_start = 0;
    let mut cursor = 0;
    while cursor < source.len() {
        let next = advance_css_char(source, cursor);
        match source.as_bytes()[cursor] {
            b'/' if source.as_bytes().get(cursor + 1) == Some(&b'*') => {
                cursor = skip_comment(source, cursor).unwrap_or(source.len());
            }
            b'\'' | b'"' => {
                cursor = skip_string(source, cursor).unwrap_or(source.len());
            }
            b';' => {
                append_declaration(&mut result, &source[segment_start..cursor], remove_measure);
                result.push(';');
                cursor = next;
                segment_start = cursor;
            }
            _ => cursor = next,
        }
    }
    append_declaration(&mut result, &source[segment_start..], remove_measure);
    result
}

fn append_declaration(result: &mut String, declaration: &str, remove_measure: bool) {
    let property_start = declaration
        .find(|character: char| !character.is_ascii_whitespace())
        .unwrap_or(declaration.len());
    let property_end = declaration[property_start..]
        .find(':')
        .map(|offset| property_start + offset)
        .unwrap_or(property_start);
    if property_start == property_end {
        result.push_str(declaration);
        return;
    }
    let property = declaration[property_start..property_end]
        .trim()
        .to_ascii_lowercase();
    if remove_measure && matches!(property.as_str(), "max-width" | "max-height") {
        // Keep indentation/comments around the removed declaration so this
        // projection does not become a whitespace or comment normalizer.
        result.push_str(&declaration[..property_start]);
        return;
    }
    let projected = match property.as_str() {
        "-epub-writing-mode" => Some("-webkit-writing-mode"),
        "-epub-text-combine" => Some("-webkit-text-combine"),
        value if value.starts_with("-epub-text-emphasis-") => Some("-webkit-text-emphasis-"),
        _ => None,
    };
    let Some(projected) = projected else {
        result.push_str(declaration);
        return;
    };
    result.push_str(&declaration[..property_start]);
    if projected == "-webkit-text-emphasis-" {
        result.push_str(projected);
        result.push_str(&property["-epub-text-emphasis-".len()..]);
    } else {
        result.push_str(projected);
    }
    result.push_str(&declaration[property_end..]);
}

fn is_aozora_measure_selector(selector: &str) -> bool {
    // Comments can contain examples of generated selectors; they are not
    // selector context and must not widen the projection policy.
    let selector = remove_comments(selector);
    let bytes = selector.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor] != b'.' {
            cursor = advance_css_char(&selector, cursor);
            continue;
        }
        let start = cursor + 1;
        let mut end = start;
        while end < bytes.len()
            && (bytes[end].is_ascii_alphanumeric() || matches!(bytes[end], b'_' | b'-'))
        {
            end += 1;
        }
        let class = selector[start..end].to_ascii_lowercase();
        if class == "fit"
            || class.starts_with("max-width-")
            || class.starts_with("max-height-")
            || class.starts_with("max-measure-")
            || class.starts_with("max-extent-")
            || class.starts_with("max-size-")
            || (class.starts_with("jzm") && class[3..].chars().all(|c| c.is_ascii_digit()))
        {
            return true;
        }
        cursor = end.max(cursor + 1);
    }
    // AozoraEpub3 uses these direction-qualified image wrappers for the
    // logical max-width/max-height pair. Keep ordinary `span.img` and
    // unrelated selectors untouched.
    let has_image_wrapper = selector.contains("span.img");
    has_image_wrapper && (selector.contains(".vrtl") || selector.contains(".hltr"))
}

fn find_top_level_open(source: &str, start: usize, end: usize) -> Option<usize> {
    let mut cursor = start;
    while cursor < end {
        match source.as_bytes()[cursor] {
            b'/' if source.as_bytes().get(cursor + 1) == Some(&b'*') => {
                cursor = skip_comment(source, cursor).unwrap_or(end)
            }
            b'\'' | b'"' => cursor = skip_string(source, cursor).unwrap_or(end),
            b'{' => return Some(cursor),
            _ => cursor = advance_css_char(source, cursor),
        }
    }
    None
}

fn matching_brace(source: &str, open: usize, end: usize) -> Option<usize> {
    let mut depth = 1usize;
    let mut cursor = open + 1;
    while cursor < end {
        match source.as_bytes()[cursor] {
            b'/' if source.as_bytes().get(cursor + 1) == Some(&b'*') => {
                cursor = skip_comment(source, cursor).unwrap_or(end)
            }
            b'\'' | b'"' => cursor = skip_string(source, cursor).unwrap_or(end),
            b'{' => {
                depth += 1;
                cursor = advance_css_char(source, cursor);
            }
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(cursor);
                }
                cursor = advance_css_char(source, cursor);
            }
            _ => cursor = advance_css_char(source, cursor),
        }
    }
    None
}

fn skip_comment(source: &str, start: usize) -> Option<usize> {
    source[start + 2..]
        .find("*/")
        .map(|offset| start + 2 + offset + 2)
}

fn skip_string(source: &str, start: usize) -> Option<usize> {
    let quote = *source.as_bytes().get(start)?;
    let mut cursor = start + 1;
    while cursor < source.len() {
        match source.as_bytes()[cursor] {
            byte if byte == quote => return Some(cursor + 1),
            b'\\' => {
                cursor = advance_css_char(source, cursor);
                cursor = advance_css_char(source, cursor);
            }
            _ => cursor = advance_css_char(source, cursor),
        }
    }
    None
}

fn advance_css_char(source: &str, cursor: usize) -> usize {
    source
        .get(cursor..)
        .and_then(|remaining| remaining.chars().next())
        .map_or(source.len(), |character| cursor + character.len_utf8())
}

fn remove_comments(source: &str) -> String {
    let mut result = String::with_capacity(source.len());
    let mut rest = source;
    while let Some(start) = rest.find("/*") {
        result.push_str(&rest[..start]);
        let Some(end) = rest[start + 2..].find("*/") else {
            break;
        };
        rest = &rest[start + 2 + end + 2..];
    }
    result.push_str(rest);
    result
}
