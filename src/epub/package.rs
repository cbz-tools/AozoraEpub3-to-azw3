//! Orchestrate EPUB ZIP, package, document, style, and navigation parsing.
//!
//! The flow is ZIP → container/OPF → resources/XHTML/CSS → navigation → Book
//! IR. Detailed OPF, navigation, and XHTML semantics live in their neighboring
//! modules rather than being reassembled here.

use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};

use zip::ZipArchive;

use crate::book::{
    Book, ContentDocument, Navigation, ReadingOrderItem, RenditionAlign, RenditionSemantics,
    Resource, Resources, Styles,
};
use crate::css::SYNTHETIC_INLINE_CSS_PROPERTY;
use crate::error::{Error, Result};
use crate::xhtml::path::{
    is_external_reference, normalize_path_lossy as normalize_path, resolve_path,
};

use super::css::validate_kf8_css;
use super::navigation::{canonicalize_navigation, parse_nav_xhtml, parse_ncx};
use super::opf::{
    ManifestItem, cover_image_paths, has_property, is_legacy_svg_cover_document, parse_opf,
    parse_rootfile,
};
use super::xhtml::{
    document_styles_with_occupied_hrefs, infer_layout,
    parse_xhtml_semantics_and_document_root_writing_mode, reject_mathml, reject_scripting,
    reject_scripting_and_media_playback, reject_unsupported_srcset_and_generic_object,
    unique_resource_id, validate_viewport,
};

/// Parse an EPUB package from bytes without depending on a filesystem.
struct LoadedPackage<'a> {
    archive: ZipArchive<Cursor<&'a [u8]>>,
    parsed: super::opf::ParsedOpf,
    base: PathBuf,
    manifest_id_index: ManifestIdIndex,
    font_obfuscation_keys: HashMap<String, [u8; 20]>,
    content_source_ids: HashSet<String>,
}

struct ManifestIdIndex {
    positions: HashMap<String, usize>,
}

impl ManifestIdIndex {
    fn from_manifest(manifest: &[ManifestItem]) -> Self {
        let mut positions = HashMap::with_capacity(manifest.len());
        for (index, item) in manifest.iter().enumerate() {
            positions.entry(item.id.clone()).or_insert(index);
        }
        Self { positions }
    }

    fn get<'a>(&self, manifest: &'a [ManifestItem], id: &str) -> Option<&'a ManifestItem> {
        self.positions.get(id).map(|&index| &manifest[index])
    }
}

struct LoadedResources {
    resources: Resources,
    xhtml: HashMap<String, String>,
}

struct DiscoveredContent {
    content: Vec<ContentDocument>,
    reading_items: Vec<ReadingOrderItem>,
    document_writing_modes: Vec<Option<crate::book::WritingMode>>,
}

fn load_package(input: &[u8]) -> Result<LoadedPackage<'_>> {
    let mut archive = ZipArchive::new(Cursor::new(input))
        .map_err(|error| Error::InvalidEpub(format!("not a readable EPUB ZIP: {error}")))?;
    let container = read_zip_entry(&mut archive, "META-INF/container.xml")?;
    let opf_path = parse_rootfile(&container)?;
    let opf = read_zip_entry(&mut archive, &opf_path)?;
    let parsed = parse_opf(&opf)?;
    let manifest_id_index = ManifestIdIndex::from_manifest(&parsed.manifest);
    if let Some(item) = parsed
        .manifest
        .iter()
        .find(|item| has_property(item, "mathml"))
    {
        return Err(Error::UnsupportedEpub(format!(
            "MathML is unsupported by the KF8 projection (manifest item {})",
            item.id
        )));
    }
    validate_rendition_semantics(&parsed)?;
    reject_unsupported_media_semantics(&parsed, &manifest_id_index)?;
    let base = Path::new(&opf_path)
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .to_path_buf();
    let font_obfuscation_keys =
        super::font_obfuscation::load_font_obfuscation(&mut archive, &parsed, &base)?;
    let mut content_source_ids = HashSet::new();
    for spine_item in &parsed.spine {
        let source = manifest_id_index
            .get(&parsed.manifest, &spine_item.idref)
            .ok_or_else(|| {
                Error::InvalidEpub(format!(
                    "spine references missing manifest item {}",
                    spine_item.idref
                ))
            })?;
        content_source_ids.insert(
            resolve_content_source(source, &parsed.manifest, &manifest_id_index)?
                .id
                .clone(),
        );
    }
    Ok(LoadedPackage {
        archive,
        parsed,
        base,
        manifest_id_index,
        font_obfuscation_keys,
        content_source_ids,
    })
}

fn load_resources(
    archive: &mut ZipArchive<Cursor<&[u8]>>,
    parsed: &super::opf::ParsedOpf,
    base: &Path,
    font_obfuscation_keys: &HashMap<String, [u8; 20]>,
    content_source_ids: &HashSet<String>,
) -> Result<LoadedResources> {
    let mut resources = Resources::default();
    let mut xhtml = HashMap::<String, String>::new();
    for item in &parsed.manifest {
        if is_external_reference(&item.href) {
            // Remote resources are not EPUB ZIP entries. Preserve the
            // external reference in source documents/CSS, but do not turn it
            // into an accidental ZIP path lookup failure. The
            // `remote-resources` property is therefore an explicit safe
            // omission boundary for resources that cannot be packaged.
            continue;
        }
        let path = resolve_href(base, &item.href);
        let data = read_zip_entry(archive, &path)?;
        let data = match font_obfuscation_keys.get(&path) {
            Some(key) => super::font_obfuscation::deobfuscate_font(&data, key),
            None => data,
        };
        let resource_data = if item
            .media_type
            .eq_ignore_ascii_case("application/xhtml+xml")
            || item.media_type.eq_ignore_ascii_case("text/html")
        {
            let source = decode_text_entry(&data, TextKind::Xhtml)?;
            reject_mathml(&source)?;
            if content_source_ids.contains(item.id.as_str()) {
                reject_scripting_and_media_playback(&source)?;
                reject_unsupported_srcset_and_generic_object(
                    &source,
                    &item.href,
                    &parsed.manifest,
                )?;
                xhtml.insert(item.id.clone(), source);
            }
            // ContentDocument owns the parsed XHTML source. XHTML resources
            // are routing metadata only after parsing and are never emitted
            // as binary KF8 resources, so do not retain a second full byte
            // buffer in Book.resources.
            Vec::new()
        } else if item.media_type.eq_ignore_ascii_case("text/css") {
            let source = decode_text_entry(&data, TextKind::Css)?;
            // Downstream CSS flows consume the one canonical UTF-8 form.
            source.into_bytes()
        } else if item.media_type.eq_ignore_ascii_case("image/svg+xml")
            && content_source_ids.contains(item.id.as_str())
        {
            xhtml.insert(item.id.clone(), svg_content_document(&data)?);
            Vec::new()
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
    Ok(LoadedResources { resources, xhtml })
}

fn discover_content(
    parsed: &super::opf::ParsedOpf,
    base: &Path,
    resources: &mut Resources,
    xhtml: &mut HashMap<String, String>,
    manifest_id_index: &ManifestIdIndex,
) -> Result<DiscoveredContent> {
    let mut occupied_hrefs = resources
        .items
        .iter()
        .map(|resource| normalize_path(&resource.href))
        .collect::<HashSet<_>>();
    let mut content = Vec::new();
    let mut reading_items = Vec::new();
    let mut document_writing_modes = Vec::new();
    let cover_image_paths = cover_image_paths(parsed, base);
    let mut occupied_resource_ids = resources
        .items
        .iter()
        .map(|resource| resource.id.clone())
        .collect::<HashSet<_>>();
    let effective_items = parsed
        .spine
        .iter()
        .map(|spine_item| {
            let item = manifest_id_index
                .get(&parsed.manifest, &spine_item.idref)
                .ok_or_else(|| {
                    Error::InvalidEpub(format!(
                        "spine references missing manifest item {}",
                        spine_item.idref
                    ))
                })?;
            resolve_content_source(item, &parsed.manifest, manifest_id_index)
        })
        .collect::<Result<Vec<_>>>()?;
    let mut remaining_source_uses = HashMap::<String, usize>::new();
    for effective_item in &effective_items {
        *remaining_source_uses
            .entry(effective_item.id.clone())
            .or_default() += 1;
    }
    for (spine_index, (spine_item, effective_item)) in
        parsed.spine.iter().zip(effective_items).enumerate()
    {
        let source = match remaining_source_uses.get_mut(&effective_item.id) {
            Some(remaining) if *remaining == 1 => xhtml.remove(&effective_item.id),
            Some(remaining) => {
                *remaining -= 1;
                xhtml.get(&effective_item.id).cloned()
            }
            None => None,
        };
        if let Some(source) = source {
            if spine_item.layout == super::opf::SpineLayout::PrePaginated {
                validate_viewport(&source)?;
            }
            let (mut semantic, writing_mode) =
                parse_xhtml_semantics_and_document_root_writing_mode(&source)?;
            semantic.is_cover = semantic.is_cover
                || is_legacy_svg_cover_document(
                    effective_item,
                    &semantic,
                    parsed.spine.first().map(|spine_item| &spine_item.idref),
                    &cover_image_paths,
                    base,
                );
            let is_cover = semantic.is_cover;
            drop(semantic);
            document_writing_modes.push(writing_mode);
            let discovered_styles = document_styles_with_occupied_hrefs(
                &source,
                &effective_item.href,
                &mut occupied_hrefs,
            )?;
            let referenced_styles = discovered_styles
                .iter()
                .map(|style| style.reference.clone())
                .collect();
            let rendition = merge_rendition(parsed.rendition, spine_item.rendition);
            let mut inline_index = 0usize;
            for style in &discovered_styles {
                let Some(source) = style.inline_source.as_deref() else {
                    continue;
                };
                let Some(href) = style.resource_href.as_deref() else {
                    continue;
                };
                let base_id = format!("__inline_style_{}_{}", spine_item.idref, inline_index);
                let resource_id = unique_resource_id(&base_id, &occupied_resource_ids);
                occupied_resource_ids.insert(resource_id.clone());
                resources.items.push(Resource {
                    id: resource_id,
                    href: href.to_owned(),
                    media_type: "text/css".to_owned(),
                    properties: vec![SYNTHETIC_INLINE_CSS_PROPERTY.to_owned()],
                    data: source.as_bytes().to_vec(),
                });
                inline_index += 1;
            }
            let mut content_document = ContentDocument {
                id: spine_item.idref.clone(),
                href: effective_item.href.clone(),
                media_type: effective_item.media_type.clone(),
                source_xhtml: source,
                is_cover,
                source_properties: spine_item.properties.clone(),
                source_spine_index: spine_index,
                referenced_styles,
                rendition,
                ..ContentDocument::default()
            };
            content_document.set_layout_pre_paginated(
                spine_item.layout == super::opf::SpineLayout::PrePaginated,
            );
            content.push(content_document);
        }
        reading_items.push(ReadingOrderItem {
            id: spine_item.idref.clone(),
            href: effective_item.href.clone(),
            media_type: effective_item.media_type.clone(),
            linear: spine_item.linear,
        });
    }
    Ok(DiscoveredContent {
        content,
        reading_items,
        document_writing_modes,
    })
}

pub fn parse_epub(input: &[u8]) -> Result<Book> {
    let LoadedPackage {
        mut archive,
        parsed,
        base,
        manifest_id_index,
        font_obfuscation_keys,
        content_source_ids,
    } = load_package(input)?;
    let LoadedResources {
        resources,
        mut xhtml,
    } = load_resources(
        &mut archive,
        &parsed,
        &base,
        &font_obfuscation_keys,
        &content_source_ids,
    )?;
    let mut resources = resources;
    let mut styles = Styles::default();
    let DiscoveredContent {
        content,
        reading_items,
        document_writing_modes,
    } = discover_content(
        &parsed,
        &base,
        &mut resources,
        &mut xhtml,
        &manifest_id_index,
    )?;
    let active_css_hrefs = super::css::active_css_stylesheets(&content, &resources.items);
    for resource in resources
        .items
        .iter()
        .filter(|resource| resource.media_type.eq_ignore_ascii_case("text/css"))
    {
        if !active_css_hrefs.contains(&normalize_path(&resource.href)) {
            continue;
        }
        let source = std::str::from_utf8(&resource.data)
            .expect("EPUB CSS resources are normalized to UTF-8");
        validate_kf8_css(source)?;
        styles
            .sheets
            .push(crate::epub::parse_css(&resource.href, source));
    }
    styles.computed = styles
        .sheets
        .iter()
        .flat_map(|sheet| sheet.computed_styles())
        .collect();

    let nav_item = parsed
        .manifest
        .iter()
        .find(|item| has_property(item, "nav"));
    let nav_is_primary = parsed.ncx_id.is_none() && nav_item.is_some();
    let (mut navigation, navigation_path, supplemental_nav) =
        if let Some(ncx_id) = parsed.ncx_id.as_deref() {
            let item = manifest_id_index.get(&parsed.manifest, ncx_id);
            if let Some(item) = item {
                let path = resolve_href(&base, &item.href);
                (
                    parse_ncx(&read_zip_entry(&mut archive, &path)?)?,
                    Some(path),
                    nav_item.map(|nav_item| resolve_href(&base, &nav_item.href)),
                )
            } else {
                (
                    Navigation::default(),
                    None,
                    nav_item.map(|nav_item| resolve_href(&base, &nav_item.href)),
                )
            }
        } else if let Some(item) = nav_item {
            let path = resolve_href(&base, &item.href);
            (
                parse_nav_xhtml(&read_zip_entry(&mut archive, &path)?)?,
                Some(path),
                None,
            )
        } else {
            (Navigation::default(), None, None)
        };
    // Landmarks are a separate semantic channel from the visible TOC. Keep
    // them when an EPUB 3 package also supplies an NCX, because Guide and
    // initial-route consumers use the landmark meanings rather than the TOC
    // source selection.
    if nav_is_primary {
        let use_supplemental_items = navigation.items.is_empty();
        let mut nav = Navigation {
            items: if use_supplemental_items {
                std::mem::take(&mut navigation.items)
            } else {
                Vec::new()
            },
            page_list: std::mem::take(&mut navigation.page_list),
            landmarks: std::mem::take(&mut navigation.landmarks),
            custom: std::mem::take(&mut navigation.custom),
            ..Navigation::default()
        };
        if let Some(nav_path) = navigation_path.as_deref() {
            canonicalize_navigation(&mut nav, nav_path, &base, &parsed.manifest);
        }
        navigation.landmarks = nav.landmarks;
        navigation.page_list = nav.page_list;
        navigation.custom = nav.custom;
        if use_supplemental_items {
            navigation.items = nav.items;
        }
    } else if let Some(nav_path) = supplemental_nav {
        let mut nav = parse_nav_xhtml(&read_zip_entry(&mut archive, &nav_path)?)?;
        canonicalize_navigation(&mut nav, &nav_path, &base, &parsed.manifest);
        navigation.landmarks = nav.landmarks;
        navigation.page_list = nav.page_list;
        navigation.custom = nav.custom;
        if navigation.items.is_empty() {
            navigation.items = nav.items;
        }
    }
    if let Some(path) = navigation_path {
        canonicalize_navigation(&mut navigation, &path, &base, &parsed.manifest);
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
        rendition: parsed.rendition,
        styles,
    })
}

fn reject_unsupported_media_semantics(
    parsed: &super::opf::ParsedOpf,
    manifest_id_index: &ManifestIdIndex,
) -> Result<()> {
    for item in &parsed.manifest {
        if let Some(media_overlay) = item.media_overlay.as_deref() {
            return Err(Error::UnsupportedEpub(format!(
                "G8-06/G8-08/G8-09 media overlay boundary: manifest item {} declares media-overlay {}",
                item.id, media_overlay
            )));
        }
        if has_property(item, "media-overlay") {
            return Err(Error::UnsupportedEpub(format!(
                "G8-06/G8-08/G8-09 media overlay boundary: manifest item {} declares the media-overlay property",
                item.id
            )));
        }
    }
    for spine_item in &parsed.spine {
        if let Some(media_overlay) = spine_item.media_overlay.as_deref() {
            return Err(Error::UnsupportedEpub(format!(
                "G8-06/G8-08/G8-09 media overlay boundary: spine item {} declares media-overlay {}",
                spine_item.idref, media_overlay
            )));
        }
        let item = manifest_id_index
            .get(&parsed.manifest, &spine_item.idref)
            .ok_or_else(|| {
                Error::InvalidEpub(format!(
                    "spine references missing manifest item {}",
                    spine_item.idref
                ))
            })?;
        if is_audio_media_type(&item.media_type) {
            return Err(Error::UnsupportedEpub(format!(
                "G6-23/G6-25 media boundary: spine item {} is an audio resource and playback is unsupported",
                item.id
            )));
        }
        if is_video_media_type(&item.media_type) {
            return Err(Error::UnsupportedEpub(format!(
                "G6-24/G6-25 media boundary: spine item {} is a video resource and playback is unsupported",
                item.id
            )));
        }
        if is_smil_media_type(&item.media_type) {
            return Err(Error::UnsupportedEpub(format!(
                "G8-06/G8-07/G8-08/G8-09 media overlay boundary: spine item {} is an SMIL document",
                item.id
            )));
        }
    }
    Ok(())
}

fn is_audio_media_type(media_type: &str) -> bool {
    media_type.eq_ignore_ascii_case("audio/mpeg")
        || media_type.eq_ignore_ascii_case("audio/mp4")
        || media_type.eq_ignore_ascii_case("audio/ogg")
        || media_type.eq_ignore_ascii_case("audio/wav")
        || media_type.eq_ignore_ascii_case("audio/webm")
        || media_type.to_ascii_lowercase().starts_with("audio/")
}

fn is_video_media_type(media_type: &str) -> bool {
    media_type.to_ascii_lowercase().starts_with("video/")
}

fn is_smil_media_type(media_type: &str) -> bool {
    media_type.eq_ignore_ascii_case("application/smil+xml")
}

fn validate_rendition_semantics(parsed: &super::opf::ParsedOpf) -> Result<()> {
    validate_rendition("publication", parsed.rendition)?;
    for item in &parsed.spine {
        validate_rendition(&format!("spine item {}", item.idref), item.rendition)?;
    }
    Ok(())
}

fn validate_rendition(scope: &str, rendition: RenditionSemantics) -> Result<()> {
    if scope == "publication" && rendition.align_x == Some(RenditionAlign::Center) {
        return Err(Error::UnsupportedEpub(format!(
            "{scope} rendition:align-x-center has no demonstrated KF8 projection"
        )));
    }
    Ok(())
}

fn merge_rendition(
    publication: crate::book::RenditionSemantics,
    item: crate::book::RenditionSemantics,
) -> crate::book::RenditionSemantics {
    crate::book::RenditionSemantics {
        orientation: item.orientation.or(publication.orientation),
        spread: item.spread.or(publication.spread),
        flow: item.flow.or(publication.flow),
        align_x: item.align_x.or(publication.align_x),
        page_spread: item.page_spread,
    }
}

pub(super) fn read_zip_entry<R: Read + std::io::Seek>(
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

fn is_content_document(item: &ManifestItem) -> bool {
    item.media_type
        .eq_ignore_ascii_case("application/xhtml+xml")
        || item.media_type.eq_ignore_ascii_case("text/html")
        || item.media_type.eq_ignore_ascii_case("image/svg+xml")
}

fn resolve_content_source<'a>(
    item: &'a ManifestItem,
    manifest: &'a [ManifestItem],
    manifest_id_index: &ManifestIdIndex,
) -> Result<&'a ManifestItem> {
    let mut current = item;
    let mut visited = HashSet::new();
    loop {
        if is_content_document(current) {
            return Ok(current);
        }
        if !visited.insert(current.id.as_str()) {
            return Err(Error::InvalidEpub(format!(
                "fallback chain for {} contains a cycle at {}",
                item.id, current.id
            )));
        }
        let Some(fallback_id) = current.fallback.as_deref() else {
            return Err(Error::UnsupportedEpub(format!(
                "fallback chain for {} ends at unsupported media type {}",
                item.id, current.media_type
            )));
        };
        current = manifest_id_index
            .get(manifest, fallback_id)
            .ok_or_else(|| {
                Error::InvalidEpub(format!(
                    "fallback chain for {} references missing target {}",
                    item.id, fallback_id
                ))
            })?;
    }
}

fn svg_content_document(data: &[u8]) -> Result<String> {
    let source = decode_text_entry(data, TextKind::Xhtml)?;
    reject_scripting(&source)?;
    let source = source.trim_start_matches('\u{feff}');
    let source = strip_svg_prolog(source);
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?><html xmlns="http://www.w3.org/1999/xhtml"><head><title>SVG Content Document</title></head><body><div class="epub-svg-content">{source}</div></body></html>"#
    ))
}

fn strip_svg_prolog(source: &str) -> &str {
    let mut source = source.trim_start();
    if source.starts_with("<?xml") {
        if let Some(end) = source.find("?>") {
            source = &source[end + 2..];
        }
    }
    source.trim_start()
}

#[derive(Debug, Clone, Copy)]
enum TextKind {
    Xhtml,
    Css,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TextEncoding {
    Utf8,
    Utf16Le,
    Utf16Be,
}

fn decode_text_entry(data: &[u8], kind: TextKind) -> Result<String> {
    let (encoding, offset) = if data.starts_with(&[0xef, 0xbb, 0xbf]) {
        (TextEncoding::Utf8, 3)
    } else if data.starts_with(&[0xff, 0xfe]) {
        (TextEncoding::Utf16Le, 2)
    } else if data.starts_with(&[0xfe, 0xff]) {
        (TextEncoding::Utf16Be, 2)
    } else if looks_like_utf16le(data) {
        (TextEncoding::Utf16Le, 0)
    } else if looks_like_utf16be(data) {
        (TextEncoding::Utf16Be, 0)
    } else {
        (TextEncoding::Utf8, 0)
    };
    let source = match encoding {
        TextEncoding::Utf8 => std::str::from_utf8(&data[offset..])
            .map(str::to_owned)
            .map_err(|error| {
                Error::InvalidEpub(format!(
                    "invalid UTF-8 {} entry: {error}",
                    text_kind_name(kind)
                ))
            })?,
        TextEncoding::Utf16Le | TextEncoding::Utf16Be => {
            if (data.len() - offset) % 2 != 0 {
                return Err(Error::InvalidEpub(format!(
                    "odd-length UTF-16 {} entry",
                    text_kind_name(kind)
                )));
            }
            let little_endian = encoding == TextEncoding::Utf16Le;
            let units = data[offset..].chunks_exact(2).map(|pair| {
                if little_endian {
                    u16::from_le_bytes([pair[0], pair[1]])
                } else {
                    u16::from_be_bytes([pair[0], pair[1]])
                }
            });
            char::decode_utf16(units)
                .collect::<std::result::Result<String, _>>()
                .map_err(|error| {
                    Error::InvalidEpub(format!(
                        "invalid UTF-16 {} entry: {error}",
                        text_kind_name(kind)
                    ))
                })?
        }
    };
    validate_declared_encoding(&source, kind, encoding)?;
    Ok(source)
}

fn looks_like_utf16le(data: &[u8]) -> bool {
    data.len() >= 4 && data[1] == 0 && data[3] == 0 && data[0] != 0 && data[2] != 0
}

fn looks_like_utf16be(data: &[u8]) -> bool {
    data.len() >= 4 && data[0] == 0 && data[2] == 0 && data[1] != 0 && data[3] != 0
}

fn text_kind_name(kind: TextKind) -> &'static str {
    match kind {
        TextKind::Xhtml => "XHTML",
        TextKind::Css => "CSS",
    }
}

fn validate_declared_encoding(source: &str, kind: TextKind, actual: TextEncoding) -> Result<()> {
    let declaration = match kind {
        TextKind::Xhtml => find_quoted_encoding(source, "encoding"),
        TextKind::Css => find_css_charset(source),
    };
    let Some(declaration) = declaration else {
        return Ok(());
    };
    let normalized_declaration = declaration
        .trim()
        .to_ascii_lowercase()
        .replace(['-', '_'], "");
    let declared = normalize_encoding_name(&declaration).ok_or_else(|| {
        Error::InvalidEpub(format!(
            "unsupported {} encoding declaration {declaration}",
            text_kind_name(kind)
        ))
    })?;
    let generic_utf16 = normalized_declaration == "utf16"
        && matches!(actual, TextEncoding::Utf16Le | TextEncoding::Utf16Be);
    if !generic_utf16 && declared != actual {
        return Err(Error::InvalidEpub(format!(
            "{} encoding declaration {declaration} does not match decoded bytes",
            text_kind_name(kind)
        )));
    }
    Ok(())
}

fn normalize_encoding_name(value: &str) -> Option<TextEncoding> {
    match value
        .trim()
        .to_ascii_lowercase()
        .replace(['-', '_'], "")
        .as_str()
    {
        "utf8" => Some(TextEncoding::Utf8),
        "utf16" | "utf16le" => Some(TextEncoding::Utf16Le),
        "utf16be" => Some(TextEncoding::Utf16Be),
        _ => None,
    }
}

fn find_quoted_encoding(source: &str, key: &str) -> Option<String> {
    let prefix = source.get(..source.len().min(1024))?;
    let lower = prefix.to_ascii_lowercase();
    let start = lower.find(key)? + key.len();
    let remainder = &prefix[start..];
    let quote = remainder.find(['\'', '"'])?;
    let quoted = &remainder[quote + 1..];
    let end = quoted.find(remainder.as_bytes()[quote] as char)?;
    Some(quoted[..end].to_owned())
}

fn find_css_charset(source: &str) -> Option<String> {
    let prefix = source
        .trim_start()
        .get(..source.trim_start().len().min(256))?;
    let lower = prefix.to_ascii_lowercase();
    let start = lower.find("@charset")? + "@charset".len();
    let remainder = &prefix[start..];
    let quote = remainder.find(['\'', '"'])?;
    let quoted = &remainder[quote + 1..];
    let end = quoted.find(remainder.as_bytes()[quote] as char)?;
    Some(quoted[..end].to_owned())
}

pub(super) fn resolve_href(base: &Path, href: &str) -> String {
    let base = base.to_string_lossy();
    let base = format!("{base}/");
    resolve_path(&base, href).unwrap_or_default()
}
