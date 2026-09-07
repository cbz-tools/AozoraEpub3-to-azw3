//! Parse OPF XML into the package-level [`ParsedOpf`] intermediate form.
//!
//! Manifest, spine, metadata, and package-level cover/layout declarations are
//! handled here. XHTML semantics and KF8 serialization are outside this module.

use std::collections::HashSet;
use std::io::Cursor;
use std::path::Path;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use super::navigation::split_link_suffix;
use super::package::resolve_href;
use super::xhtml::{is_primary_writing_mode_meta, parse_writing_mode};
use crate::book::{Metadata, PageProgression, SemanticDocument, WritingMode};
use crate::error::{Error, Result};
use crate::xhtml::path::normalize_path_lossy as normalize_path;
#[derive(Debug, Clone)]
pub(super) struct ManifestItem {
    pub(super) id: String,
    pub(super) href: String,
    pub(super) media_type: String,
    pub(super) properties: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum SpineLayout {
    #[default]
    Reflowable,
    PrePaginated,
}

impl SpineLayout {
    fn from_itemref_properties(properties: &[String]) -> Option<Self> {
        properties.iter().find_map(|property| {
            let property = property.to_ascii_lowercase();
            if property == "rendition:layout-pre-paginated" {
                Some(Self::PrePaginated)
            } else if property == "rendition:layout-reflowable" {
                Some(Self::Reflowable)
            } else {
                None
            }
        })
    }
}

#[derive(Debug, Clone)]
pub(super) struct SpineItem {
    pub(super) idref: String,
    pub(super) linear: bool,
    pub(super) properties: Vec<String>,
    pub(super) layout: SpineLayout,
}

#[derive(Debug, Default)]
pub(super) struct ParsedOpf {
    pub(super) metadata: Metadata,
    pub(super) manifest: Vec<ManifestItem>,
    pub(super) spine: Vec<SpineItem>,
    pub(super) publication_layout: Option<SpineLayout>,
    pub(super) page_progression: PageProgression,
    pub(super) primary_writing_mode: Option<WritingMode>,
    pub(super) ncx_id: Option<String>,
}

#[derive(Debug, Clone, Copy)]
enum MetadataMetaField {
    Cover,
    FixedLayout,
    RenditionLayout,
    BookType,
    OrientationLock,
    RenditionOrientation,
    OriginalResolution,
    PrimaryWritingMode,
}

pub(super) fn parse_rootfile(xml: &[u8]) -> Result<String> {
    let mut reader = Reader::from_reader(Cursor::new(xml));
    let mut buffer = Vec::new();
    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Empty(event) | Event::Start(event)
                if local_name(event.name().as_ref()) == "rootfile" =>
            {
                if let Some(path) = attr(&event, "full-path") {
                    return Ok(normalize_path(&path));
                }
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    Err(Error::InvalidEpub(
        "container.xml has no rootfile full-path".to_owned(),
    ))
}

pub(super) fn parse_opf(xml: &[u8]) -> Result<ParsedOpf> {
    let mut result = ParsedOpf::default();
    let mut reader = Reader::from_reader(Cursor::new(xml));
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut metadata_field: Option<String> = None;
    let mut metadata_text = String::new();
    let mut metadata_meta_field: Option<MetadataMetaField> = None;
    let mut metadata_meta_text = String::new();
    loop {
        match reader.read_event_into(&mut buffer)? {
            Event::Start(event) => {
                let name = local_name(event.name().as_ref());
                match name.as_str() {
                    "metadata" => {}
                    "title" | "creator" | "language" | "identifier" | "publisher"
                    | "description" => {
                        metadata_field = Some(name);
                        metadata_text.clear();
                    }
                    "meta" => {
                        metadata_meta_field = metadata_meta_kind(&event);
                        metadata_meta_text.clear();
                        if let Some(value) = attr(&event, "content") {
                            if let Some(field) = metadata_meta_field {
                                apply_metadata_meta(&mut result, field, &value);
                            }
                            metadata_meta_field = None;
                        }
                    }
                    "manifest" | "item" | "spine" | "itemref" => {
                        parse_opf_start(&event, &mut result);
                    }
                    _ => {}
                }
            }
            Event::Empty(event) => {
                if local_name(event.name().as_ref()) == "meta" {
                    if let (Some(field), Some(value)) =
                        (metadata_meta_kind(&event), attr(&event, "content"))
                    {
                        apply_metadata_meta(&mut result, field, &value);
                    }
                } else {
                    parse_opf_start(&event, &mut result);
                }
            }
            Event::Text(event) => {
                let text = event
                    .unescape()
                    .map_err(|error| Error::Xml(error.to_string()))?;
                if metadata_meta_field.is_some() {
                    metadata_meta_text.push_str(&text);
                } else if metadata_field.is_some() {
                    metadata_text.push_str(&text);
                }
            }
            Event::End(event) => {
                let name = local_name(event.name().as_ref());
                if name == "meta" {
                    if let Some(field) = metadata_meta_field.take() {
                        apply_metadata_meta(&mut result, field, &metadata_meta_text);
                    }
                }
                if let Some(field) = metadata_field.take() {
                    let value = metadata_text.trim().to_owned();
                    match field.as_str() {
                        "title" => result.metadata.title = Some(value),
                        "creator" => result.metadata.creator = Some(value),
                        "language" => result.metadata.language = Some(value),
                        "identifier" => result.metadata.identifier = Some(value),
                        "publisher" => result.metadata.publisher = Some(value),
                        "description" => result.metadata.description = Some(value),
                        _ => {}
                    }
                }
                let _ = name;
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    if result.manifest.is_empty() || result.spine.is_empty() {
        return Err(Error::InvalidEpub(
            "OPF must contain a manifest and spine".to_owned(),
        ));
    }
    let publication_layout = result.publication_layout.unwrap_or_default();
    for spine_item in &mut result.spine {
        spine_item.layout = SpineLayout::from_itemref_properties(&spine_item.properties)
            .unwrap_or(publication_layout);
    }
    Ok(result)
}

fn metadata_meta_kind(event: &BytesStart<'_>) -> Option<MetadataMetaField> {
    if is_primary_writing_mode_meta(event) {
        return Some(MetadataMetaField::PrimaryWritingMode);
    }
    let name = attr(event, "name");
    if name
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("cover"))
    {
        return Some(MetadataMetaField::Cover);
    }
    if name
        .as_deref()
        .is_some_and(|value| value.eq_ignore_ascii_case("fixed-layout"))
    {
        return Some(MetadataMetaField::FixedLayout);
    }
    if attr(event, "property").as_deref().is_some_and(|value| {
        value
            .split_whitespace()
            .any(|token| token.eq_ignore_ascii_case("rendition:layout"))
    }) {
        return Some(MetadataMetaField::RenditionLayout);
    }
    if attr(event, "property").as_deref().is_some_and(|value| {
        value
            .split_whitespace()
            .any(|token| token.eq_ignore_ascii_case("rendition:orientation"))
    }) {
        return Some(MetadataMetaField::RenditionOrientation);
    }
    match name.as_deref().map(str::to_ascii_lowercase).as_deref() {
        Some("book-type") => Some(MetadataMetaField::BookType),
        Some("orientation-lock") => Some(MetadataMetaField::OrientationLock),
        Some("original-resolution") => Some(MetadataMetaField::OriginalResolution),
        _ => None,
    }
}

fn apply_metadata_meta(result: &mut ParsedOpf, field: MetadataMetaField, value: &str) {
    let value = value.trim();
    match field {
        MetadataMetaField::Cover => result.metadata.cover = Some(value.to_owned()),
        MetadataMetaField::FixedLayout => {
            if value.eq_ignore_ascii_case("true") {
                result.metadata.is_fixed_layout = true;
                result.publication_layout = Some(SpineLayout::PrePaginated);
            }
        }
        MetadataMetaField::RenditionLayout => {
            let layout = if value.eq_ignore_ascii_case("pre-paginated") {
                Some(SpineLayout::PrePaginated)
            } else if value.eq_ignore_ascii_case("reflowable") {
                Some(SpineLayout::Reflowable)
            } else {
                None
            };
            if let Some(layout) = layout {
                result.publication_layout = Some(layout);
                result.metadata.is_fixed_layout |= layout == SpineLayout::PrePaginated;
            }
        }
        MetadataMetaField::BookType => result.metadata.book_type = Some(value.to_owned()),
        MetadataMetaField::OrientationLock => {
            result.metadata.orientation_lock = Some(value.to_owned());
        }
        MetadataMetaField::RenditionOrientation => {
            result.metadata.orientation = Some(value.to_owned());
        }
        MetadataMetaField::OriginalResolution => {
            result.metadata.original_resolution = Some(value.to_owned());
        }
        MetadataMetaField::PrimaryWritingMode => {
            let normalized = value.to_ascii_lowercase();
            let parsed = if normalized == "horizontal-rl" {
                Some(WritingMode::HorizontalTb)
            } else {
                parse_writing_mode(value)
            };
            if result.primary_writing_mode.is_none() {
                if let Some(parsed) = parsed {
                    result.primary_writing_mode = Some(parsed);
                    result.metadata.primary_writing_mode = Some(value.to_owned());
                }
            }
        }
    }
}

fn parse_opf_start(event: &BytesStart<'_>, result: &mut ParsedOpf) {
    match local_name(event.name().as_ref()).as_str() {
        "item" => {
            let Some(id) = attr(event, "id") else { return };
            let Some(href) = attr(event, "href") else {
                return;
            };
            let Some(media_type) = attr(event, "media-type") else {
                return;
            };
            let properties = attr(event, "properties")
                .unwrap_or_default()
                .split_whitespace()
                .map(str::to_owned)
                .collect();
            let item = ManifestItem {
                id: id.clone(),
                href,
                media_type,
                properties,
            };
            if item
                .media_type
                .eq_ignore_ascii_case("application/x-dtbncx+xml")
            {
                result.ncx_id = Some(id);
            }
            result.manifest.push(item);
        }
        "itemref" => {
            if let Some(idref) = attr(event, "idref") {
                let linear = attr(event, "linear")
                    .map(|value| value != "no")
                    .unwrap_or(true);
                let properties = attr(event, "properties")
                    .unwrap_or_default()
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect();
                result.spine.push(SpineItem {
                    idref,
                    linear,
                    properties,
                    layout: SpineLayout::default(),
                });
            }
        }
        "spine" => {
            if let Some(value) = attr(event, "page-progression-direction") {
                result.page_progression = match value.as_str() {
                    "rtl" => PageProgression::Rtl,
                    "ltr" => PageProgression::Ltr,
                    _ => PageProgression::Default,
                };
            }
        }
        _ => {}
    }
}

pub(super) fn has_property(item: &ManifestItem, property: &str) -> bool {
    item.properties
        .iter()
        .any(|value| value.eq_ignore_ascii_case(property))
}

pub(super) fn cover_image_paths(parsed: &ParsedOpf, opf_base: &Path) -> HashSet<String> {
    parsed
        .manifest
        .iter()
        .filter(|item| {
            has_property(item, "cover-image")
                || (parsed.metadata.cover.as_deref() == Some(item.id.as_str())
                    && item.media_type.to_ascii_lowercase().starts_with("image/"))
        })
        .map(|item| resolve_href(opf_base, &item.href))
        .collect()
}

pub(super) fn is_legacy_svg_cover_document(
    item: &ManifestItem,
    semantic: &SemanticDocument,
    first_spine_id: Option<&String>,
    cover_image_paths: &HashSet<String>,
    opf_base: &Path,
) -> bool {
    first_spine_id == Some(&item.id)
        && (has_property(item, "svg")
            || has_property(item, "cover")
            || has_property(item, "cover-document"))
        && !cover_image_paths.is_empty()
        && semantic.image_references.iter().any(|reference| {
            let document_path = resolve_href(opf_base, &item.href);
            let document_base = Path::new(&document_path)
                .parent()
                .unwrap_or_else(|| Path::new(""));
            let (reference_path, _) = split_link_suffix(reference);
            cover_image_paths.contains(&resolve_href(document_base, reference_path))
        })
}

pub(super) fn local_name(name: &[u8]) -> String {
    local_name_ref(name).to_ascii_lowercase()
}

pub(super) fn local_name_ref(name: &[u8]) -> &str {
    let name = std::str::from_utf8(name).unwrap_or_default();
    name.rsplit(':').next().unwrap_or(name)
}

pub(super) fn attr(event: &BytesStart<'_>, wanted: &str) -> Option<String> {
    event.attributes().flatten().find_map(|attribute| {
        if local_name(attribute.key.as_ref()) == wanted {
            attribute
                .unescape_value()
                .ok()
                .map(|value| value.into_owned())
        } else {
            None
        }
    })
}
