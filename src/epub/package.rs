//! Orchestrate EPUB ZIP, package, document, style, and navigation parsing.
//!
//! The flow is ZIP → container/OPF → resources/XHTML/CSS → navigation → Book
//! IR. Detailed OPF, navigation, and XHTML semantics live in their neighboring
//! modules rather than being reassembled here.

use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read};
use std::path::Path;

use zip::ZipArchive;

use crate::book::{
    Book, ContentDocument, Navigation, ReadingOrderItem, Resource, Resources, Styles,
};
use crate::error::{Error, Result};
use crate::xhtml::path::normalize_path_lossy as normalize_path;

use super::navigation::{canonicalize_navigation, parse_nav_xhtml, parse_ncx};
use super::opf::{
    cover_image_paths, has_property, is_legacy_svg_cover_document, parse_opf, parse_rootfile,
};
use super::xhtml::{
    document_root_writing_mode, document_styles_with_occupied_hrefs, infer_layout,
    parse_xhtml_semantics, unique_resource_id,
};

const SYNTHETIC_INLINE_CSS_PROPERTY: &str = "__synthetic_inline_css";

/// Parse an EPUB package from bytes without depending on a filesystem.
pub fn parse_epub(input: &[u8]) -> Result<Book> {
    let mut archive = ZipArchive::new(Cursor::new(input))
        .map_err(|error| Error::InvalidEpub(format!("not a readable EPUB ZIP: {error}")))?;
    let container = read_zip_entry(&mut archive, "META-INF/container.xml")?;
    let opf_path = parse_rootfile(&container)?;
    let opf = read_zip_entry(&mut archive, &opf_path)?;
    let parsed = parse_opf(&opf)?;
    let base = Path::new(&opf_path)
        .parent()
        .unwrap_or_else(|| Path::new(""));
    let spine_ids = parsed
        .spine
        .iter()
        .map(|spine_item| spine_item.idref.as_str())
        .collect::<HashSet<_>>();

    let mut resources = Resources::default();
    let mut xhtml = HashMap::<String, String>::new();
    let mut styles = Styles::default();
    for item in &parsed.manifest {
        let path = resolve_href(base, &item.href);
        let data = read_zip_entry(&mut archive, &path)?;
        let resource_data = if item
            .media_type
            .eq_ignore_ascii_case("application/xhtml+xml")
            || item.media_type.eq_ignore_ascii_case("text/html")
        {
            if spine_ids.contains(item.id.as_str()) {
                let source = into_lossy_string(data);
                xhtml.insert(item.id.clone(), source);
            }
            // ContentDocument owns the parsed XHTML source. XHTML resources
            // are routing metadata only after parsing and are never emitted
            // as binary KF8 resources, so do not retain a second full byte
            // buffer in Book.resources.
            Vec::new()
        } else if item.media_type.eq_ignore_ascii_case("text/css") {
            let source = String::from_utf8_lossy(&data).into_owned();
            styles
                .sheets
                .push(crate::epub::parse_css(&item.href, source));
            data
        } else {
            data
        };
        resources.items.push(Resource {
            id: item.id.clone(),
            href: item.href.clone(),
            media_type: item.media_type.clone(),
            properties: item.properties.clone(),
            data: resource_data,
        });
    }
    let mut occupied_hrefs = resources
        .items
        .iter()
        .map(|resource| normalize_path(&resource.href))
        .collect::<HashSet<_>>();
    let mut content = Vec::new();
    let mut reading_items = Vec::new();
    let mut document_writing_modes = Vec::new();
    let cover_image_paths = cover_image_paths(&parsed, base);
    let mut occupied_resource_ids = resources
        .items
        .iter()
        .map(|resource| resource.id.clone())
        .collect::<HashSet<_>>();
    for spine_item in &parsed.spine {
        let Some(item) = parsed
            .manifest
            .iter()
            .find(|item| item.id == spine_item.idref)
        else {
            return Err(Error::InvalidEpub(format!(
                "spine references missing manifest item {}",
                spine_item.idref
            )));
        };
        if let Some(source) = xhtml.remove(&item.id) {
            let mut semantic = parse_xhtml_semantics(&source)?;
            semantic.is_cover = semantic.is_cover
                || is_legacy_svg_cover_document(
                    item,
                    &semantic,
                    parsed.spine.first().map(|spine_item| &spine_item.idref),
                    &cover_image_paths,
                    base,
                );
            let is_cover = semantic.is_cover;
            drop(semantic);
            document_writing_modes.push(document_root_writing_mode(&source));
            let discovered_styles =
                document_styles_with_occupied_hrefs(&source, &item.href, &mut occupied_hrefs);
            let referenced_styles = discovered_styles
                .iter()
                .map(|style| style.reference.clone())
                .collect();
            let mut inline_index = 0usize;
            for style in &discovered_styles {
                let Some(source) = style.inline_source.as_deref() else {
                    continue;
                };
                let Some(href) = style.resource_href.as_deref() else {
                    continue;
                };
                let base_id = format!("__inline_style_{}_{}", item.id, inline_index);
                let resource_id = unique_resource_id(&base_id, &occupied_resource_ids);
                occupied_resource_ids.insert(resource_id.clone());
                resources.items.push(Resource {
                    id: resource_id,
                    href: href.to_owned(),
                    media_type: "text/css".to_owned(),
                    properties: vec![SYNTHETIC_INLINE_CSS_PROPERTY.to_owned()],
                    data: source.as_bytes().to_vec(),
                });
                styles
                    .sheets
                    .push(crate::epub::parse_css(href.to_owned(), source));
                inline_index += 1;
            }
            let mut content_document = ContentDocument {
                id: item.id.clone(),
                href: item.href.clone(),
                media_type: item.media_type.clone(),
                source_xhtml: source,
                is_cover,
                referenced_styles,
                ..ContentDocument::default()
            };
            content_document.set_layout_pre_paginated(
                spine_item.layout == super::opf::SpineLayout::PrePaginated,
            );
            content.push(content_document);
        }
        reading_items.push(ReadingOrderItem {
            id: item.id.clone(),
            href: item.href.clone(),
            media_type: item.media_type.clone(),
            linear: spine_item.linear,
        });
    }
    styles.computed = styles
        .sheets
        .iter()
        .flat_map(|sheet| sheet.computed_styles())
        .collect();

    let (mut navigation, navigation_path) = if let Some(ncx_id) = parsed.ncx_id.as_deref() {
        let item = parsed.manifest.iter().find(|item| item.id == ncx_id);
        if let Some(item) = item {
            let path = resolve_href(base, &item.href);
            (
                parse_ncx(&read_zip_entry(&mut archive, &path)?)?,
                Some(path),
            )
        } else {
            (Navigation::default(), None)
        }
    } else if let Some(item) = parsed
        .manifest
        .iter()
        .find(|item| has_property(item, "nav"))
    {
        let path = resolve_href(base, &item.href);
        (
            parse_nav_xhtml(&read_zip_entry(&mut archive, &path)?)?,
            Some(path),
        )
    } else {
        (Navigation::default(), None)
    };
    // Landmarks are a separate semantic channel from the visible TOC. Keep
    // them when an EPUB 3 package also supplies an NCX, because Guide and
    // initial-route consumers use the landmark meanings rather than the TOC
    // source selection.
    if let Some(nav_item) = parsed
        .manifest
        .iter()
        .find(|item| has_property(item, "nav"))
    {
        let nav_path = resolve_href(base, &nav_item.href);
        let mut nav = parse_nav_xhtml(&read_zip_entry(&mut archive, &nav_path)?)?;
        canonicalize_navigation(&mut nav, &nav_path, base, &parsed.manifest);
        navigation.landmarks = nav.landmarks;
        if navigation.items.is_empty() {
            navigation.items = nav.items;
        }
    }
    if let Some(path) = navigation_path {
        canonicalize_navigation(&mut navigation, &path, base, &parsed.manifest);
    }

    let layout = infer_layout(
        &styles,
        parsed.page_progression,
        parsed.primary_writing_mode,
        &document_writing_modes,
    );
    let cover = parsed.metadata.cover.clone().or_else(|| {
        parsed
            .manifest
            .iter()
            .find(|item| {
                item.properties
                    .iter()
                    .any(|property| property.eq_ignore_ascii_case("cover-image"))
            })
            .map(|item| item.id.clone())
    });
    let mut metadata = parsed.metadata;
    metadata.cover = cover;
    Ok(Book {
        metadata,
        reading_order: crate::book::ReadingOrder {
            items: reading_items,
            page_progression: parsed.page_progression,
        },
        navigation,
        content,
        resources,
        layout,
        styles,
    })
}

fn read_zip_entry<R: Read + std::io::Seek>(
    archive: &mut ZipArchive<R>,
    path: &str,
) -> Result<Vec<u8>> {
    let mut entry = archive
        .by_name(path)
        .map_err(|error| Error::InvalidEpub(format!("missing EPUB entry {path}: {error}")))?;
    let mut data = Vec::new();
    entry.read_to_end(&mut data).map_err(|source| Error::Io {
        path: path.to_owned(),
        source,
    })?;
    Ok(data)
}

fn into_lossy_string(data: Vec<u8>) -> String {
    match String::from_utf8(data) {
        Ok(source) => source,
        Err(error) => String::from_utf8_lossy(error.as_bytes()).into_owned(),
    }
}

pub(super) fn resolve_href(base: &Path, href: &str) -> String {
    normalize_path(&base.join(href).to_string_lossy())
}
