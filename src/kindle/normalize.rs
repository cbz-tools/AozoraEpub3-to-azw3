use std::collections::{HashMap, HashSet};

use super::{
    KindleBook, KindleLandmark, KindleLayout, KindleNavigationItem, KindleResource, KindleSection,
    ir::{KindleLayoutSemantic, KindleMetadata},
};
use crate::book::{Book, Layout, NavigationItem, NavigationLandmark, plain_display_text};
use crate::xhtml::scan::{find_ascii_case_insensitive, html_tag_end, html_tag_name_range};

const COVER_LANDMARK_MARKER: &str = "kindle:cover-landmark";

/// Normalize the semantic Book IR into the Kindle-specific IR consumed by the
/// KF8 writer. Container and record details deliberately stay in `kf8`.
pub(crate) fn normalize(book: Book) -> KindleBook {
    let cover_hrefs = cover_document_hrefs(&book);
    let cover_ids = book
        .content
        .iter()
        .filter(|content| content.is_cover || cover_hrefs.contains(&document_path(&content.href)))
        .map(|content| content.id.clone())
        .collect::<HashSet<_>>();
    let keep_comic_cover_page = book.metadata.is_fixed_layout
        && book
            .metadata
            .book_type
            .as_deref()
            .is_some_and(|book_type| book_type.trim().eq_ignore_ascii_case("comic"));
    let omitted_cover_hrefs = book
        .reading_order
        .items
        .iter()
        .filter(|item| {
            cover_ids.contains(item.id.as_str()) || cover_hrefs.contains(&document_path(&item.href))
        })
        .map(|item| document_path(&item.href))
        .filter(|href| !href.is_empty())
        .collect::<HashSet<_>>();
    let mut content_by_id = book
        .content
        .into_iter()
        .map(|content| (content.id.clone(), content))
        .collect::<HashMap<_, _>>();
    let mut sections: Vec<KindleSection> = book
        .reading_order
        .items
        .into_iter()
        .filter_map(|item| {
            let content = content_by_id.remove(&item.id)?;
            let is_cover = cover_ids.contains(item.id.as_str())
                || cover_hrefs.contains(&document_path(&item.href));
            if is_cover && !(keep_comic_cover_page && item.linear && content.is_pre_paginated()) {
                return None;
            }
            let layout = if content.is_pre_paginated() {
                KindleLayoutSemantic::PrePaginated
            } else {
                KindleLayoutSemantic::Reflowable
            };
            let source_xhtml = if cover_hrefs.is_empty() {
                content.source_xhtml
            } else {
                match neutralize_cover_links(&content.source_xhtml, &item.href, &cover_hrefs) {
                    Some(source) => source,
                    None => content.source_xhtml,
                }
            };
            Some(KindleSection {
                id: item.id,
                href: item.href,
                source_xhtml,
                referenced_styles: content.referenced_styles,
                linear: item.linear,
                layout,
            })
        })
        .collect();
    let navigation = prune_navigation(book.navigation.items, &cover_hrefs)
        .into_iter()
        .map(KindleNavigationItem::from)
        .collect::<Vec<_>>();
    // A manifest nav resource is a navigation source, not visible content, when
    // it is absent from the spine. When it is in the spine, preserve its source
    // position and linear semantics.
    let navigation_resource = book.resources.items.iter().find(|resource| {
        (resource
            .media_type
            .eq_ignore_ascii_case("application/xhtml+xml")
            || resource.media_type.eq_ignore_ascii_case("text/html"))
            && resource
                .properties
                .iter()
                .any(|property| property.eq_ignore_ascii_case("nav"))
    });
    let toc_href = if let Some(resource) = navigation_resource {
        if sections.iter().any(|section| section.id == resource.id) {
            Some(resource.href.clone())
        } else {
            None
        }
    } else if navigation.is_empty() {
        None
    } else {
        let href = synthetic_toc_href(&sections, &book.resources.items);
        sections.insert(
            0,
            KindleSection {
                id: "__kindle_toc".to_owned(),
                href: href.clone(),
                source_xhtml: render_synthetic_toc(&navigation),
                referenced_styles: Vec::new(),
                linear: true,
                layout: KindleLayoutSemantic::Reflowable,
            },
        );
        Some(href)
    };
    let fallback_body_href = sections
        .iter()
        .find(|section| {
            section.linear
                && toc_href
                    .as_deref()
                    .is_none_or(|toc| document_path(&section.href) != document_path(toc))
        })
        .map(|section| section.href.clone());
    let mut landmarks = book
        .navigation
        .landmarks
        .into_iter()
        .filter(|landmark| !landmark.kind.eq_ignore_ascii_case("cover"))
        .map(|mut landmark| {
            let target_path = document_path(&landmark.href);
            let target_is_omitted_cover = !target_path.is_empty()
                && omitted_cover_hrefs.contains(&target_path)
                && !sections
                    .iter()
                    .any(|section| document_path(&section.href) == target_path);
            if is_bodymatter_landmark(&landmark.kind)
                && target_is_omitted_cover
                && let Some(fallback_body_href) = fallback_body_href.as_ref()
            {
                landmark.href = fallback_body_href.clone();
            }
            landmark
        })
        .map(KindleLandmark::from)
        .collect::<Vec<_>>();
    if !landmarks
        .iter()
        .any(|landmark| is_bodymatter_landmark(&landmark.kind))
    {
        if let Some(body_section) = sections
            .iter()
            .find(|section| section.linear && Some(section.href.as_str()) != toc_href.as_deref())
        {
            landmarks.push(KindleLandmark {
                kind: "text".to_owned(),
                label: "本文".to_owned(),
                href: body_section.href.clone(),
            });
        }
    }
    if let Some(href) = toc_href {
        if !landmarks
            .iter()
            .any(|landmark| landmark.kind.eq_ignore_ascii_case("toc"))
        {
            landmarks.push(KindleLandmark {
                kind: "toc".to_owned(),
                label: "目次".to_owned(),
                href,
            });
        }
    }
    let metadata = book.metadata;
    KindleBook {
        metadata: KindleMetadata {
            title: metadata.title,
            creator: metadata.creator,
            language: metadata.language,
            identifier: metadata.identifier,
            publisher: metadata.publisher,
            description: metadata.description,
            cover_resource_id: metadata.cover,
            is_fixed_layout: metadata.is_fixed_layout,
            primary_writing_mode: metadata.primary_writing_mode,
            book_type: metadata.book_type,
            orientation: metadata.orientation,
            orientation_lock: metadata.orientation_lock,
            original_resolution: metadata.original_resolution,
        },
        layout: KindleLayout {
            writing_mode: book.layout.writing_mode,
            page_progression: book.layout.page_progression,
            direction: book.layout.direction,
        },
        sections,
        navigation,
        landmarks,
        resources: book
            .resources
            .items
            .into_iter()
            .map(|resource| {
                let crate::book::Resource {
                    id,
                    href,
                    media_type,
                    properties,
                    data,
                } = resource;
                let is_css = media_type.eq_ignore_ascii_case("text/css");
                KindleResource {
                    id,
                    href,
                    media_type,
                    properties,
                    data: if is_css {
                        crate::kindle::project_css_for_kindle(&String::from_utf8_lossy(&data))
                            .into_bytes()
                    } else {
                        data
                    },
                }
            })
            .collect(),
    }
}

fn is_bodymatter_landmark(kind: &str) -> bool {
    matches!(
        kind.to_ascii_lowercase().as_str(),
        "text" | "body" | "bodymatter" | "start"
    )
}

fn cover_document_hrefs(book: &Book) -> HashSet<String> {
    let mut hrefs = book
        .navigation
        .landmarks
        .iter()
        .filter(|landmark| landmark.kind.eq_ignore_ascii_case("cover"))
        .map(|landmark| document_path(&landmark.href))
        .filter(|href| !href.is_empty())
        .collect::<HashSet<_>>();
    hrefs.extend(
        book.content
            .iter()
            .filter(|content| content.is_cover)
            .map(|content| document_path(&content.href)),
    );
    hrefs.retain(|href| !href.is_empty());
    hrefs
}

fn prune_navigation(
    items: Vec<NavigationItem>,
    cover_hrefs: &HashSet<String>,
) -> Vec<NavigationItem> {
    let mut result = Vec::new();
    for item in items {
        let NavigationItem {
            label,
            href,
            children: item_children,
        } = item;
        let children = prune_navigation(item_children, cover_hrefs);
        if cover_hrefs.contains(&document_path(&href)) {
            result.extend(children);
        } else {
            result.push(NavigationItem {
                label,
                href,
                children,
            });
        }
    }
    result
}

fn document_path(href: &str) -> String {
    let path = href
        .find(['#', '?'])
        .map(|index| &href[..index])
        .unwrap_or(href)
        .replace('\\', "/");
    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            value => parts.push(value),
        }
    }
    parts.join("/")
}

fn neutralize_cover_links(
    source: &str,
    section_href: &str,
    cover_hrefs: &HashSet<String>,
) -> Option<String> {
    if cover_hrefs.is_empty() {
        return None;
    }
    let mut output: Option<String> = None;
    let mut output_cursor = 0usize;
    let mut search_cursor = 0usize;
    while let Some(start) = find_ascii_case_insensitive(source, "href", search_cursor) {
        let previous = start
            .checked_sub(1)
            .and_then(|index| source.as_bytes().get(index));
        if previous.is_some_and(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b':' | b'-'))
        {
            search_cursor = start + 4;
            continue;
        }
        let mut value_start = start + 4;
        while source
            .as_bytes()
            .get(value_start)
            .is_some_and(u8::is_ascii_whitespace)
        {
            value_start += 1;
        }
        if source.as_bytes().get(value_start) != Some(&b'=') {
            search_cursor = start + 4;
            continue;
        }
        value_start += 1;
        while source
            .as_bytes()
            .get(value_start)
            .is_some_and(u8::is_ascii_whitespace)
        {
            value_start += 1;
        }
        let Some(&quote) = source.as_bytes().get(value_start) else {
            break;
        };
        if !matches!(quote, b'"' | b'\'') {
            search_cursor = value_start;
            continue;
        }
        let content_start = value_start + 1;
        let Some(relative_end) = source[content_start..].find(quote as char) else {
            break;
        };
        let content_end = content_start + relative_end;
        let is_cover_link = cover_hrefs.contains(&resolve_document_path(
            section_href,
            &source[content_start..content_end],
        ));
        let replacement = if is_cover_landmark(source, content_start) {
            COVER_LANDMARK_MARKER
        } else {
            "#"
        };
        if let Some(result) = output.as_mut() {
            result.push_str(&source[output_cursor..content_start]);
            if is_cover_link {
                result.push_str(replacement);
            } else {
                result.push_str(&source[content_start..content_end]);
            }
        } else if is_cover_link {
            let mut result = String::with_capacity(source.len());
            result.push_str(&source[..content_start]);
            result.push_str(replacement);
            output = Some(result);
        }
        if is_cover_link || output.is_some() {
            output_cursor = content_end;
        }
        search_cursor = content_end;
    }
    output.map(|mut result| {
        result.push_str(&source[output_cursor..]);
        result
    })
}

fn is_cover_landmark(source: &str, href_start: usize) -> bool {
    let Some(tag_start) = source[..href_start].rfind('<') else {
        return false;
    };
    let Some(tag_end) = html_tag_end(source, tag_start) else {
        return false;
    };
    let Some((name_start, name_end, closing)) = html_tag_name_range(source, tag_start, tag_end)
    else {
        return false;
    };
    let tag = &source[tag_start..=tag_end];
    let anchor = !closing && source[name_start..name_end].eq_ignore_ascii_case("a");
    if !anchor || !has_attribute_token(tag, "epub:type", "cover") {
        return false;
    }

    let mut cursor = 0;
    let mut nav_stack = Vec::new();
    while let Some(nav_start) = find_ascii_case_insensitive(source, "<", cursor) {
        if nav_start >= tag_start {
            break;
        }
        let Some(nav_end) = html_tag_end(source, nav_start) else {
            break;
        };
        if let Some((name_start, name_end, closing)) =
            html_tag_name_range(source, nav_start, nav_end)
        {
            let is_nav = source[name_start..name_end].eq_ignore_ascii_case("nav");
            if is_nav {
                if closing {
                    nav_stack.pop();
                } else {
                    nav_stack.push(has_attribute_token(
                        &source[nav_start..=nav_end],
                        "epub:type",
                        "landmarks",
                    ));
                }
            }
        }
        cursor = nav_end + 1;
    }
    // `epub:type="cover"` is a landmark only inside the source landmarks
    // navigation. Other suppressed cover hyperlinks retain their established
    // fragment-only normalization.
    nav_stack.into_iter().any(|is_landmarks| is_landmarks)
}

fn has_attribute_token(tag: &str, wanted: &str, value: &str) -> bool {
    let Some(start) = find_ascii_case_insensitive(tag, wanted, 0) else {
        return false;
    };
    let mut cursor = start + wanted.len();
    while tag
        .as_bytes()
        .get(cursor)
        .is_some_and(u8::is_ascii_whitespace)
    {
        cursor += 1;
    }
    if tag.as_bytes().get(cursor) != Some(&b'=') {
        return false;
    }
    cursor += 1;
    while tag
        .as_bytes()
        .get(cursor)
        .is_some_and(u8::is_ascii_whitespace)
    {
        cursor += 1;
    }
    let Some(&quote) = tag.as_bytes().get(cursor) else {
        return false;
    };
    if !matches!(quote, b'"' | b'\'') {
        return false;
    }
    let start = cursor + 1;
    let Some(end) = tag[start..].find(quote as char) else {
        return false;
    };
    tag[start..start + end]
        .split_whitespace()
        .any(|candidate| candidate.eq_ignore_ascii_case(value))
}

fn resolve_document_path(section_href: &str, target: &str) -> String {
    let target_path = target
        .find(['#', '?'])
        .map(|index| &target[..index])
        .unwrap_or(target);
    if target_path.is_empty() {
        return document_path(section_href);
    }
    let section_path = document_path(section_href);
    let base = section_path
        .rsplit_once('/')
        .map(|(directory, _)| directory)
        .unwrap_or_default();
    document_path(&format!("{base}/{target_path}"))
}

fn synthetic_toc_href(sections: &[KindleSection], resources: &[crate::book::Resource]) -> String {
    let mut index = 0usize;
    loop {
        let href = if index == 0 {
            "__kindle_toc.xhtml".to_owned()
        } else {
            format!("__kindle_toc-{index}.xhtml")
        };
        if sections.iter().all(|section| section.href != href)
            && resources.iter().all(|resource| resource.href != href)
        {
            return href;
        }
        index += 1;
    }
}

fn render_synthetic_toc(items: &[KindleNavigationItem]) -> String {
    let mut output = String::from(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" class="hltr">
<head><title>目　次</title></head>
<body class="p-toc"><nav epub:type="toc" id="toc"><h1>目　次</h1><ol>"#,
    );
    render_synthetic_toc_items(items, &mut output);
    output.push_str("</ol></nav></body></html>");
    output
}

fn render_synthetic_toc_items(items: &[KindleNavigationItem], output: &mut String) {
    for item in items {
        output.push_str("<li>");
        if item.href.is_empty() {
            push_html_text(output, &item.label);
        } else {
            output.push_str("<a href=\"");
            push_html_attribute(output, &item.href);
            output.push_str("\">");
            push_html_text(output, &item.label);
            output.push_str("</a>");
        }
        if !item.children.is_empty() {
            output.push_str("<ol>");
            render_synthetic_toc_items(&item.children, output);
            output.push_str("</ol>");
        }
        output.push_str("</li>");
    }
}

fn push_html_text(output: &mut String, value: &str) {
    push_html_escaped(output, value, false);
}

fn push_html_attribute(output: &mut String, value: &str) {
    push_html_escaped(output, value, true);
}

fn push_html_escaped(output: &mut String, value: &str, attribute: bool) {
    for character in value.chars() {
        match character {
            '&' => output.push_str("&amp;"),
            '<' => output.push_str("&lt;"),
            '>' => output.push_str("&gt;"),
            '"' if attribute => output.push_str("&quot;"),
            _ => output.push(character),
        }
    }
}

impl From<NavigationLandmark> for KindleLandmark {
    fn from(landmark: NavigationLandmark) -> Self {
        Self {
            kind: landmark.kind,
            label: plain_display_text(&landmark.label),
            href: landmark.href,
        }
    }
}

impl From<NavigationItem> for KindleNavigationItem {
    fn from(item: NavigationItem) -> Self {
        Self {
            label: plain_display_text(&item.label),
            href: item.href,
            children: item.children.into_iter().map(Self::from).collect(),
        }
    }
}

impl From<Layout> for KindleLayout {
    fn from(layout: Layout) -> Self {
        Self {
            writing_mode: layout.writing_mode,
            page_progression: layout.page_progression,
            direction: layout.direction,
        }
    }
}
