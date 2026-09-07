//! Orchestrate the KF8 build pipeline and assemble the final record set.
//!
//! Subsystems own cover lowering, CSS/RawML preparation, geometry, indexes,
//! and coordinate semantics; this module coordinates their order and combines
//! their results without redefining those algorithms.

use super::css_flow::{css_resource_base_href, referenced_css_resources, rewrite_css_assets};
use super::div::Div;
use super::exth::ExthHeader;
use super::fcis::{encode_eof, encode_fcis};
use super::fdst::Fdst;
use super::flis::encode_flis;
use super::fragment::{Fragment, FragmentEntry};
use super::guide::Guide;
use super::mobi_header::MobiHeader;
use super::ncx::Ncx;
use super::palmdoc::PalmDocHeader;
use super::position::{PositionMap, assign_aids};
use super::rawml::{
    PendingInternalLink, SectionParts, generated_layout_css, lower_pre_paginated_section,
    materialize_internal_links, rewrite_internal_links, rewrite_layout_class_for_document,
    rewrite_layout_fallback_link, rewrite_section_assets, rewrite_stylesheet_links_with_references,
    section_has_explicit_layout, split_section_parts,
};
use super::resource::{
    BinaryResourceGeometry, is_binary_resource, is_css_resource, is_text_resource,
};
use super::skel::{Skel, SkelEntry};
use super::text::{PalmDocCompressor, TextRecord};
use crate::error::Result;
use crate::kindle::{
    KindleBook, KindleLayoutSemantic, KindlePageProgression as PageProgression, KindleResource,
    KindleSection, KindleWritingMode as WritingMode, generate_library_thumbnail,
    prepare_cover_resource,
};
use crate::xhtml::scan::{advance_char, html_local_name_is, html_tag_name_range};

const GENERATED_TEXT_DIRECTORY: &str = "Text";
const GENERATED_SECTION_PREFIX: &str = "part";
const GENERATED_SECTION_EXTENSION: &str = ".xhtml";
pub(crate) const SYNTHETIC_INLINE_CSS_PROPERTY: &str = "__synthetic_inline_css";

#[derive(Debug)]
pub(crate) struct Kf8Record {
    pub(crate) data: Vec<u8>,
}

#[derive(Debug)]
pub(crate) struct Kf8Book {
    pub palm_doc: PalmDocHeader,
    pub mobi: MobiHeader,
    pub exth: ExthHeader,
    pub title: Option<String>,
    pub records: Vec<Kf8Record>,
}

pub(crate) struct Kf8Builder;

struct PreparedContent<'a> {
    sections: Vec<KindleSection>,
    css_resources: Vec<&'a KindleResource>,
    page_flows: Vec<Vec<u8>>,
    pending_links: Vec<Vec<PendingInternalLink>>,
    library_thumbnail: Option<Vec<u8>>,
}

struct TextGeometry {
    sections: Vec<KindleSection>,
    section_parts: Vec<SectionParts>,
    position_map: PositionMap,
    css_flows: Vec<Vec<u8>>,
    css_flow_lengths: Vec<usize>,
    page_flows: Vec<Vec<u8>>,
    rawml_length: usize,
    library_thumbnail: Option<Vec<u8>>,
}

struct TextData {
    sections: Vec<KindleSection>,
    section_parts: Vec<SectionParts>,
    position_map: PositionMap,
    text_records: Vec<TextRecord>,
    text_length_u32: u32,
    text_record_count: usize,
    xhtml_length_u32: u32,
    css_flow_lengths: Vec<usize>,
    page_flow_lengths: Vec<usize>,
    library_thumbnail: Option<Vec<u8>>,
}

struct Indexes {
    position_map: PositionMap,
    text_records: Vec<TextRecord>,
    indexing_tbs: Vec<Vec<u8>>,
    text_length_u32: u32,
    text_record_count: usize,
    skel_main: Vec<u8>,
    skel_details: Vec<Vec<u8>>,
    fragment_main: Vec<u8>,
    fragment_details: Vec<Vec<u8>>,
    fragment_ctoc: Vec<Vec<u8>>,
    guide_main: Vec<u8>,
    guide_details: Vec<Vec<u8>>,
    guide_ctoc: Vec<Vec<u8>>,
    ncx_main: Vec<u8>,
    ncx_details: Vec<Vec<u8>>,
    ncx_ctoc: Vec<Vec<u8>>,
    fdst: Fdst,
    library_thumbnail: Option<Vec<u8>>,
}

struct PhysicalLayout {
    records: Vec<Kf8Record>,
    position_map: PositionMap,
    fdst: Fdst,
    text_length_u32: u32,
    text_record_count: usize,
    palm_doc_compression: bool,
    first_non_text_record: u32,
    fragment_record: u32,
    skel_record: u32,
    guide_record: u32,
    ncx_record: u32,
    fdst_record: u32,
    fcis_record: u32,
    flis_record: u32,
    geometry: BinaryResourceGeometry,
}

impl Kf8Builder {
    pub(crate) fn build_with_compression(
        mut book: KindleBook,
        palm_doc_compression: bool,
    ) -> Result<Kf8Book> {
        let mut resources = std::mem::take(&mut book.resources);
        let library_thumbnail = prepare_cover(&book, &mut resources)?;
        let cover_resource_id = book.metadata.cover_resource_id.clone();
        let prepared = prepare_content(
            &mut book.sections,
            &resources,
            library_thumbnail,
            cover_resource_id.as_deref(),
        )?;
        let geometry = build_geometry(&book, &resources, prepared)?;
        book.resources = resources;
        let text = build_text_records(geometry)?;
        let indexes = build_indexes(&book, text)?;
        let layout = assemble_records(&mut book, indexes, palm_doc_compression)?;
        build_record0(book, layout)
    }
}

fn prepare_cover(book: &KindleBook, resources: &mut [KindleResource]) -> Result<Option<Vec<u8>>> {
    let library_thumbnail = book
        .metadata
        .cover_resource_id
        .as_deref()
        .and_then(|cover_id| resources.iter().find(|resource| resource.id == cover_id))
        .and_then(generate_library_thumbnail);
    prepare_cover_resource(resources, book.metadata.cover_resource_id.as_deref())?;
    Ok(library_thumbnail)
}

fn prepare_content<'a>(
    sections: &mut Vec<KindleSection>,
    resources: &'a [KindleResource],
    library_thumbnail: Option<Vec<u8>>,
    cover_resource_id: Option<&str>,
) -> Result<PreparedContent<'a>> {
    let css_resources = referenced_css_resources(sections, resources);
    let layout_flow_number = u32::try_from(css_resources.len())
        .ok()
        .and_then(|count| count.checked_add(1));
    let mut sections = std::mem::take(sections)
        .into_iter()
        .enumerate()
        .map(|(index, section)| -> Result<KindleSection> {
            let source = rewrite_layout_class_for_document(
                &section.source_xhtml,
                index,
                section_has_explicit_layout(&section, resources),
            );
            let source = rewrite_stylesheet_links_with_references(
                source,
                &section.href,
                &css_resources,
                Some(&section.referenced_styles),
            );
            let source = if source.contains("kf8-layout") {
                if let Some(flow_number) = layout_flow_number {
                    rewrite_layout_fallback_link(&source, flow_number)
                } else {
                    source
                }
            } else {
                source
            };
            let source = rewrite_section_assets(source, &section.href, resources)?;
            let source = super::rawml::rewrite_cover_landmark_reference(
                source,
                &section.href,
                cover_resource_id,
                resources,
            )?;
            let source_xhtml = super::rawml::materialize_ordered_list_values(&source)?;
            Ok(KindleSection {
                id: section.id,
                href: section.href,
                source_xhtml,
                referenced_styles: section.referenced_styles,
                linear: section.linear,
                layout: section.layout,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let generated_layout_flow = sections
        .iter()
        .any(|section| section.source_xhtml.contains("kf8-layout"));
    let page_flow_start = css_resources
        .len()
        .checked_add(usize::from(generated_layout_flow))
        .and_then(|count| count.checked_add(1))
        .and_then(|count| u32::try_from(count).ok())
        .ok_or_else(|| crate::error::Error::Output("page flow number overflow".to_owned()))?;
    let mut page_flows = Vec::new();
    for section in &mut sections {
        if section.layout != KindleLayoutSemantic::PrePaginated {
            continue;
        }
        let flow_number = page_flow_start
            .checked_add(u32::try_from(page_flows.len()).map_err(|_| {
                crate::error::Error::Output("page flow count exceeds u32".to_owned())
            })?)
            .ok_or_else(|| crate::error::Error::Output("page flow number overflow".to_owned()))?;
        let flow_reference = format!(
            "kindle:flow:{}?mime=image/svg+xml",
            to_base32_fixed(flow_number, 4)?
        );
        let css_reference = section.referenced_styles.iter().find_map(|style_href| {
            super::css_flow::css_flow_number(&section.href, style_href, &css_resources)
                .map(super::css_flow::stylesheet_flow_reference)
        });
        let Some((source_xhtml, page_flow)) = lower_pre_paginated_section(
            &section.source_xhtml,
            &flow_reference,
            css_reference.as_deref(),
        ) else {
            return Err(crate::error::Error::Output(format!(
                "pre-paginated section {} has no page presentation",
                section.href
            )));
        };
        section.source_xhtml = source_xhtml;
        page_flows.push(page_flow);
    }
    let mut next_aid = 0u32;
    for section in &mut sections {
        section.source_xhtml = assign_aids(&section.source_xhtml, &mut next_aid)?;
    }
    let mut pending_links = Vec::with_capacity(sections.len());
    for section_index in 0..sections.len() {
        let source = std::mem::take(&mut sections[section_index].source_xhtml);
        let (source, links) = rewrite_internal_links(
            source,
            &sections[section_index].href,
            section_index,
            &sections,
            &css_resources,
        )?;
        sections[section_index].source_xhtml = source;
        pending_links.push(links);
    }
    Ok(PreparedContent {
        sections,
        css_resources,
        page_flows,
        pending_links,
        library_thumbnail,
    })
}

fn build_geometry(
    book: &KindleBook,
    resources: &[KindleResource],
    prepared: PreparedContent<'_>,
) -> Result<TextGeometry> {
    let PreparedContent {
        sections,
        css_resources,
        page_flows,
        pending_links,
        library_thumbnail,
    } = prepared;
    let mut section_parts = sections
        .iter()
        .map(|section| split_section_parts(&section.source_xhtml))
        .collect::<Result<Vec<_>>>()?;
    let position_map = PositionMap::build(&sections, &section_parts)?;
    let mut sections = sections;
    materialize_internal_links(
        &mut sections,
        &pending_links,
        &position_map,
        &mut section_parts,
    )?;
    let mut css_flows = Vec::with_capacity(css_resources.len() + 1);
    for resource in &css_resources {
        let css_base_href = css_resource_base_href(&sections, resource);
        css_flows.push(rewrite_css_assets(
            &resource.data,
            &css_base_href,
            resources,
            &css_resources,
        ));
    }
    if sections
        .iter()
        .any(|section| section.source_xhtml.contains("kf8-layout"))
    {
        css_flows.push(generated_layout_css(book.layout).into_bytes());
    }
    let css_flow_lengths = css_flows.iter().map(Vec::len).collect::<Vec<_>>();
    let rawml_length = section_parts
        .iter()
        .try_fold(0usize, |total, parts| {
            let total = total.checked_add(parts.skeleton.len())?;
            parts
                .fragments
                .iter()
                .try_fold(total, |total, fragment| total.checked_add(fragment.len()))
        })
        .and_then(|total| {
            css_flows
                .iter()
                .try_fold(total, |total, flow| total.checked_add(flow.len()))
        })
        .and_then(|total| {
            page_flows
                .iter()
                .try_fold(total, |total, flow| total.checked_add(flow.len()))
        })
        .ok_or_else(|| crate::error::Error::Output("text length overflow".to_owned()))?;
    Ok(TextGeometry {
        sections,
        section_parts,
        position_map,
        css_flows,
        css_flow_lengths,
        page_flows,
        rawml_length,
        library_thumbnail,
    })
}

fn assemble_records(
    book: &mut KindleBook,
    indexes: Indexes,
    palm_doc_compression: bool,
) -> Result<PhysicalLayout> {
    let Indexes {
        position_map,
        text_records,
        indexing_tbs,
        text_length_u32,
        text_record_count,
        skel_main,
        skel_details,
        fragment_main,
        fragment_details,
        fragment_ctoc,
        guide_main,
        guide_details,
        guide_ctoc,
        ncx_main,
        ncx_details,
        ncx_ctoc,
        fdst,
        library_thumbnail,
    } = indexes;
    let mut compressor = palm_doc_compression.then(PalmDocCompressor::new);
    let mut records = Vec::with_capacity(text_record_count);
    for (record, tbs) in text_records.into_iter().zip(indexing_tbs.iter()) {
        records.push(Kf8Record {
            data: record.into_trailing_data_with_compression(tbs, compressor.as_mut()),
        });
    }
    records.push(Kf8Record {
        data: vec![0x00, 0x00],
    });
    let first_non_text_record =
        1u32.checked_add(u32::try_from(text_record_count).map_err(|_| {
            crate::error::Error::Output("text record count exceeds u32".to_owned())
        })?)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| crate::error::Error::Output("record index overflow".to_owned()))?;
    let fragment_record = first_non_text_record;
    records.push(Kf8Record {
        data: fragment_main,
    });
    for detail in fragment_details {
        records.push(Kf8Record { data: detail });
    }
    for ctoc in fragment_ctoc {
        records.push(Kf8Record { data: ctoc });
    }
    let skel_record = u32::try_from(
        records
            .len()
            .checked_add(1)
            .ok_or_else(|| crate::error::Error::Output("SKEL record index overflow".to_owned()))?,
    )
    .map_err(|_| crate::error::Error::Output("SKEL record index overflow".to_owned()))?;
    records.push(Kf8Record { data: skel_main });
    for detail in skel_details {
        records.push(Kf8Record { data: detail });
    }
    let guide_record = if guide_main.is_empty() {
        u32::MAX
    } else {
        let record = u32::try_from(records.len().checked_add(1).ok_or_else(|| {
            crate::error::Error::Output("Guide record index overflow".to_owned())
        })?)
        .map_err(|_| crate::error::Error::Output("Guide record index overflow".to_owned()))?;
        records.push(Kf8Record { data: guide_main });
        for detail in guide_details {
            records.push(Kf8Record { data: detail });
        }
        for ctoc in guide_ctoc {
            records.push(Kf8Record { data: ctoc });
        }
        record
    };
    let ncx_record = u32::try_from(
        records
            .len()
            .checked_add(1)
            .ok_or_else(|| crate::error::Error::Output("NCX record index overflow".to_owned()))?,
    )
    .map_err(|_| crate::error::Error::Output("NCX record index overflow".to_owned()))?;
    records.push(Kf8Record { data: ncx_main });
    for detail in ncx_details {
        records.push(Kf8Record { data: detail });
    }
    for ctoc in ncx_ctoc {
        records.push(Kf8Record { data: ctoc });
    }
    let fdst_record = u32::try_from(
        records
            .len()
            .checked_add(1)
            .ok_or_else(|| crate::error::Error::Output("FDST record index overflow".to_owned()))?,
    )
    .map_err(|_| crate::error::Error::Output("FDST record index overflow".to_owned()))?;
    records.push(Kf8Record {
        data: fdst.encode(),
    });
    let binary_resources = book
        .resources
        .iter()
        .filter(|resource| is_binary_resource(resource))
        .collect::<Vec<_>>();
    let resource_record_start =
        u32::try_from(records.len().checked_add(1).ok_or_else(|| {
            crate::error::Error::Output("resource record index overflow".to_owned())
        })?)
        .map_err(|_| crate::error::Error::Output("resource record index overflow".to_owned()))?;
    let geometry = BinaryResourceGeometry::compute(
        &binary_resources,
        resource_record_start,
        book.metadata.cover_resource_id.as_deref(),
        library_thumbnail.is_some(),
    )?;
    let resources = std::mem::take(&mut book.resources);
    for resource in resources {
        if is_text_resource(&resource) || is_css_resource(&resource) {
            continue;
        }
        records.push(Kf8Record {
            data: resource.data,
        });
    }
    if let Some(thumbnail) = library_thumbnail {
        records.push(Kf8Record { data: thumbnail });
    }

    let flis_record = u32::try_from(records.len() + 1)
        .map_err(|_| crate::error::Error::Output("FLIS record index overflow".to_owned()))?;
    records.push(Kf8Record {
        data: encode_flis(),
    });
    let fcis_record = u32::try_from(records.len() + 1)
        .map_err(|_| crate::error::Error::Output("FCIS record index overflow".to_owned()))?;
    records.push(Kf8Record {
        data: encode_fcis(text_length_u32)?,
    });
    records.push(Kf8Record { data: encode_eof() });

    Ok(PhysicalLayout {
        records,
        position_map,
        fdst,
        text_length_u32,
        text_record_count,
        palm_doc_compression,
        first_non_text_record,
        fragment_record,
        skel_record,
        guide_record,
        ncx_record,
        fdst_record,
        fcis_record,
        flis_record,
        geometry,
    })
}

fn build_record0(book: KindleBook, layout: PhysicalLayout) -> Result<Kf8Book> {
    let PhysicalLayout {
        records,
        position_map,
        fdst,
        text_length_u32,
        text_record_count,
        palm_doc_compression,
        first_non_text_record,
        fragment_record,
        skel_record,
        guide_record,
        ncx_record,
        fdst_record,
        fcis_record,
        flis_record,
        geometry,
    } = layout;
    let mut exth = ExthHeader::default();
    if let Some(value) = &book.metadata.creator {
        exth.push_text(100, value);
    }
    if let Some(value) = &book.metadata.publisher {
        exth.push_text(101, value);
    }
    if let Some(value) = &book.metadata.description {
        exth.push_text(103, value);
    }
    if let Some(value) = &book.metadata.title {
        exth.push_text(503, value);
    }
    if let Some(value) = &book.metadata.language {
        exth.push_text(524, value);
    }
    exth.push_bytes(125, geometry.count.to_be_bytes());
    if let Some(start_reading_offset) = position_map.start_reading_offset(&book.landmarks)? {
        exth.push_bytes(116, start_reading_offset.to_be_bytes());
    }
    if let Some(cover_offset) = geometry.cover_offset {
        exth.push_bytes(201, cover_offset.to_be_bytes());
    }
    if let Some(thumbnail_offset) = geometry.thumbnail_offset {
        exth.push_bytes(202, thumbnail_offset.to_be_bytes());
        exth.push_text(129, format!("kindle:embed:{}", to_base32(thumbnail_offset)));
    }
    let writing_mode = if book.metadata.is_fixed_layout {
        book.metadata
            .primary_writing_mode
            .as_deref()
            .unwrap_or_else(|| writing_mode_value(book.layout.writing_mode))
    } else {
        writing_mode_value(book.layout.writing_mode)
    };
    exth.push_text(525, writing_mode);
    exth.push_text(527, page_progression_value(book.layout.page_progression));
    if book.metadata.is_fixed_layout {
        exth.push_text(122, "true");
        if book
            .metadata
            .book_type
            .as_deref()
            .is_some_and(|value| value.trim().eq_ignore_ascii_case("comic"))
        {
            exth.push_text(123, "comic");
        }
        if let Some(value) = &book.metadata.orientation_lock {
            exth.push_text(124, value);
        }
        if let Some(value) = &book.metadata.original_resolution {
            exth.push_text(126, value);
        }
    }
    let mut mobi = MobiHeader {
        first_non_text_record,
        first_resource_record: geometry.first_image_record,
        first_image_index: geometry.first_image_record,
        fcis_record,
        fcis_count: 1,
        flis_record,
        flis_count: 1,
        last_image_index: geometry.last_image_record,
        extra_data_flags: 0x0003,
        fdst_record,
        fdst_flow_count: fdst.entries.len() as u32,
        index_record: fragment_record,
        ncx_record,
        skel_record,
        guide_record,
        ..MobiHeader::default()
    };
    mobi.language = book
        .metadata
        .language
        .as_deref()
        .map(language_code)
        .unwrap_or(0);
    let title = book.metadata.title;
    Ok(Kf8Book {
        palm_doc: PalmDocHeader {
            compression: if palm_doc_compression { 2 } else { 1 },
            text_length: text_length_u32,
            record_count: text_record_count as u16,
            record_size: 4096,
            encryption: 0,
        },
        mobi,
        exth,
        title,
        records,
    })
}

fn build_text_records(geometry: TextGeometry) -> Result<TextData> {
    let TextGeometry {
        sections,
        section_parts,
        position_map,
        css_flows,
        css_flow_lengths,
        page_flows,
        rawml_length,
        library_thumbnail,
    } = geometry;
    let mut stream_chunks = Vec::with_capacity(
        section_parts
            .iter()
            .map(|parts| parts.fragments.len() + 1)
            .sum::<usize>()
            + css_flows.len()
            + page_flows.len(),
    );
    for parts in &section_parts {
        stream_chunks.push(parts.skeleton.as_slice());
        stream_chunks.extend(parts.fragments.iter().map(Vec::as_slice));
    }
    stream_chunks.extend(css_flows.iter().map(Vec::as_slice));
    stream_chunks.extend(page_flows.iter().map(Vec::as_slice));
    let text_records = TextRecord::split_chunks(&stream_chunks);
    drop(stream_chunks);
    drop(css_flows);
    let expected_record_count = rawml_length
        .checked_add(4096 - 1)
        .ok_or_else(|| crate::error::Error::Output("text length overflow".to_owned()))?
        / 4096;
    if text_records.len() != expected_record_count
        || text_records
            .iter()
            .take(text_records.len().saturating_sub(1))
            .any(|record| record.data.len() != 4096)
    {
        return Err(crate::error::Error::Output(
            "PalmDOC text records do not match fixed 4096-byte coordinates".to_owned(),
        ));
    }
    if text_records.len() > u16::MAX as usize {
        return Err(crate::error::Error::Output(
            "PalmDOC text record count exceeds u16".to_owned(),
        ));
    }
    let xhtml_length = section_parts
        .iter()
        .try_fold(0usize, |total, parts| -> Result<usize> {
            let fragments_length = parts.fragments.iter().try_fold(0usize, |total, fragment| {
                total
                    .checked_add(fragment.len())
                    .ok_or_else(|| crate::error::Error::Output("XHTML length overflow".to_owned()))
            })?;
            let section_length = parts
                .skeleton
                .len()
                .checked_add(fragments_length)
                .ok_or_else(|| crate::error::Error::Output("XHTML length overflow".to_owned()))?;
            total
                .checked_add(section_length)
                .ok_or_else(|| crate::error::Error::Output("XHTML length overflow".to_owned()))
        })?;
    let text_length = text_records
        .iter()
        .map(|record| record.data.len())
        .try_fold(0usize, |total, length| total.checked_add(length))
        .ok_or_else(|| crate::error::Error::Output("text length overflow".to_owned()))?;
    if text_length != rawml_length {
        return Err(crate::error::Error::Output(
            "PalmDOC text length does not match the logical stream".to_owned(),
        ));
    }
    let xhtml_length_u32 = u32::try_from(xhtml_length)
        .map_err(|_| crate::error::Error::Output("XHTML length exceeds u32".to_owned()))?;
    let text_length_u32 = u32::try_from(text_length)
        .map_err(|_| crate::error::Error::Output("text length exceeds u32".to_owned()))?;
    let text_record_count = text_records.len();
    Ok(TextData {
        sections,
        section_parts,
        position_map,
        text_records,
        text_length_u32,
        text_record_count,
        xhtml_length_u32,
        css_flow_lengths,
        page_flow_lengths: page_flows.iter().map(Vec::len).collect(),
        library_thumbnail,
    })
}

fn build_indexes(book: &KindleBook, text: TextData) -> Result<Indexes> {
    let TextData {
        sections,
        section_parts,
        position_map,
        text_records,
        text_length_u32,
        text_record_count,
        xhtml_length_u32,
        css_flow_lengths,
        page_flow_lengths,
        library_thumbnail,
    } = text;
    let mut offset = 0u32;
    let skel = Skel {
        entries: section_parts
            .iter()
            .map(|parts| {
                let start = offset;
                let length = u32::try_from(parts.skeleton.len()).map_err(|_| {
                    crate::error::Error::Output("SKEL section length exceeds u32".to_owned())
                })?;
                offset = offset
                    .checked_add(length)
                    .and_then(|value| {
                        value.checked_add(
                            u32::try_from(parts.fragments.iter().map(Vec::len).sum::<usize>())
                                .ok()?,
                        )
                    })
                    .ok_or_else(|| {
                        crate::error::Error::Output("SKEL position overflow".to_owned())
                    })?;
                Ok(SkelEntry {
                    fragment_count: u32::try_from(parts.fragments.len()).map_err(|_| {
                        crate::error::Error::Output("SKEL fragment count exceeds u32".to_owned())
                    })?,
                    start,
                    length,
                })
            })
            .collect::<Result<Vec<_>>>()?,
    };
    skel.validate()?;
    let fragments = fragments_from_position_map(&position_map);
    fragments.validate()?;
    let div = Div::for_records(text_records.len());
    div.validate(text_records.len())?;
    let mut fdst_ranges = vec![(0, xhtml_length_u32)];
    let mut flow_start = xhtml_length_u32;
    for length in css_flow_lengths {
        let length = u32::try_from(length)
            .map_err(|_| crate::error::Error::Output("CSS flow length exceeds u32".to_owned()))?;
        let flow_end = flow_start
            .checked_add(length)
            .ok_or_else(|| crate::error::Error::Output("CSS flow position overflow".to_owned()))?;
        fdst_ranges.push((flow_start, flow_end));
        flow_start = flow_end;
    }
    for length in page_flow_lengths {
        let length = u32::try_from(length)
            .map_err(|_| crate::error::Error::Output("page flow length exceeds u32".to_owned()))?;
        let flow_end = flow_start
            .checked_add(length)
            .ok_or_else(|| crate::error::Error::Output("page flow position overflow".to_owned()))?;
        fdst_ranges.push((flow_start, flow_end));
        flow_start = flow_end;
    }
    let fdst = Fdst::from_ranges(&fdst_ranges);
    fdst.validate(text_length_u32)?;
    let (skel_main, skel_details) = skel
        .encode_pair()
        .map_err(|error| crate::error::Error::Output(format!("SKEL: {error}")))?;
    let fragment_selectors = position_map
        .fragments
        .iter()
        .map(|fragment| fragment.selector.clone())
        .collect::<Vec<_>>();
    let (fragment_main, fragment_details, fragment_ctoc) = fragments
        .encode_pair_with_ctoc(&fragment_selectors)
        .map_err(|error| {
            crate::error::Error::Output(format!(
                "FRAG ({} entries): {error}",
                fragments.entries.len()
            ))
        })?;
    let ncx = Ncx::from_navigation(&book.navigation);
    let text_record_lengths = text_records
        .iter()
        .map(|record| record.data.len())
        .collect::<Vec<_>>();
    let (_, indexing_tbs) = ncx.indexing_tbs_with_position_map(
        &position_map,
        &sections,
        &text_record_lengths,
        Some(&book.navigation),
    )?;
    if indexing_tbs.len() != text_records.len() {
        return Err(crate::error::Error::Output(
            "TBS count does not match PalmDOC text record count".to_owned(),
        ));
    }
    let (ncx_main, ncx_details, ncx_ctoc) = ncx
        .encode_pair_with_position_map_and_navigation(&position_map, &sections, &book.navigation)
        .map_err(|error| crate::error::Error::Output(format!("NCX: {error}")))?;
    let guide =
        Guide::from_positions(position_map.guide_positions(&book.landmarks, &book.navigation)?);
    let (guide_main, guide_details, guide_ctoc) = guide
        .encode_pair()
        .map_err(|error| crate::error::Error::Output(format!("Guide: {error}")))?;
    Ok(Indexes {
        position_map,
        text_records,
        indexing_tbs,
        text_length_u32,
        text_record_count,
        skel_main,
        skel_details,
        fragment_main,
        fragment_details,
        fragment_ctoc,
        guide_main,
        guide_details,
        guide_ctoc,
        ncx_main,
        ncx_details,
        ncx_ctoc,
        fdst,
        library_thumbnail,
    })
}

fn writing_mode_value(writing_mode: WritingMode) -> &'static str {
    match writing_mode {
        WritingMode::HorizontalTb => "horizontal-lr",
        WritingMode::VerticalRl => "vertical-rl",
        WritingMode::VerticalLr => "vertical-lr",
    }
}

fn page_progression_value(page_progression: PageProgression) -> &'static str {
    match page_progression {
        PageProgression::Default => "default",
        PageProgression::Ltr => "ltr",
        PageProgression::Rtl => "rtl",
    }
}

fn language_code(language: &str) -> u32 {
    let primary = language
        .trim()
        .to_ascii_lowercase()
        .split(['-', '_'])
        .next()
        .unwrap_or_default()
        .to_owned();
    match primary.as_str() {
        "ja" | "jpn" => 0x11,
        "en" | "eng" => 0x09,
        "de" | "deu" | "ger" => 0x07,
        "es" | "spa" => 0x0a,
        "fr" | "fra" | "fre" => 0x0c,
        "it" | "ita" => 0x10,
        "ko" | "kor" => 0x12,
        "zh" | "chi" | "zho" => 0x04,
        _ => 0,
    }
}

pub(crate) fn to_base32(value: u32) -> String {
    const DIGITS: &[u8; 32] = b"0123456789ABCDEFGHIJKLMNOPQRSTUV";
    let mut value = value;
    let mut digits = Vec::new();
    while value != 0 {
        digits.push(DIGITS[(value % 32) as usize]);
        value /= 32;
    }
    if digits.is_empty() {
        digits.push(b'0');
    }
    while digits.len() < 4 {
        digits.push(b'0');
    }
    digits.reverse();
    String::from_utf8(digits).expect("base32 alphabet is ASCII")
}

pub(crate) fn generated_section_path(index: usize) -> String {
    format!(
        "{GENERATED_TEXT_DIRECTORY}/{GENERATED_SECTION_PREFIX}{index:04}{GENERATED_SECTION_EXTENSION}"
    )
}

pub(super) fn preserved_style_attributes(source: &str, start: usize, tag_end: usize) -> String {
    let Some((_, mut cursor, closing)) = html_tag_name_range(source, start, tag_end) else {
        return String::new();
    };
    if closing {
        return String::new();
    }

    let bytes = source.as_bytes();
    let allowed = ["media", "title", "type"];
    let mut attributes = Vec::new();
    while cursor < tag_end {
        while cursor < tag_end && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= tag_end || bytes[cursor] == b'/' {
            break;
        }
        let attribute_start = cursor;
        while cursor < tag_end
            && !bytes[cursor].is_ascii_whitespace()
            && !matches!(bytes[cursor], b'=' | b'/' | b'>')
        {
            cursor = advance_char(source, cursor);
        }
        let attribute_end = cursor;
        while cursor < tag_end && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if attribute_start == attribute_end || bytes.get(cursor) != Some(&b'=') {
            continue;
        }
        cursor += 1;
        while cursor < tag_end && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let Some(&value_start_byte) = bytes.get(cursor) else {
            break;
        };
        let (value_start, value_end, quote) = if matches!(value_start_byte, b'"' | b'\'') {
            let value_start = cursor + 1;
            let Some(relative_end) = source[value_start..tag_end].find(value_start_byte as char)
            else {
                return String::new();
            };
            let value_end = value_start + relative_end;
            cursor = value_end + 1;
            (value_start, value_end, value_start_byte)
        } else {
            let value_start = cursor;
            while cursor < tag_end && !bytes[cursor].is_ascii_whitespace() && bytes[cursor] != b'>'
            {
                cursor = advance_char(source, cursor);
            }
            let value_end = if cursor > value_start
                && bytes[cursor - 1] == b'/'
                && source[cursor..tag_end].trim().is_empty()
            {
                cursor - 1
            } else {
                cursor
            };
            (value_start, value_end, b'"')
        };
        if let Some(name) = allowed
            .iter()
            .find(|name| html_local_name_is(source, attribute_start, attribute_end, name))
        {
            attributes.push((*name, &source[value_start..value_end], quote));
        }
    }

    let mut result = String::new();
    for (name, value, quote) in attributes {
        result.push(' ');
        result.push_str(name);
        result.push('=');
        result.push(quote as char);
        result.push_str(value);
        result.push(quote as char);
    }
    result
}

pub(super) fn stylesheet_link_href(
    source: &str,
    start: usize,
    tag_end: usize,
) -> Option<(usize, usize)> {
    let bytes = source.as_bytes();
    let (name_start, name_end, closing) = html_tag_name_range(source, start, tag_end)?;
    if closing || !html_local_name_is(source, name_start, name_end, "link") {
        return None;
    }
    let mut cursor = name_end;

    let mut has_stylesheet_rel = false;
    let mut href = None;
    while cursor < tag_end {
        while cursor < tag_end && bytes[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        if cursor >= tag_end || bytes[cursor] == b'/' {
            break;
        }
        let attribute_start = cursor;
        while cursor < tag_end
            && !bytes[cursor].is_ascii_whitespace()
            && !matches!(bytes[cursor], b'=' | b'/' | b'>')
        {
            cursor = advance_char(source, cursor);
        }
        let attribute_end = cursor;
        if attribute_start == attribute_end {
            cursor = advance_char(source, cursor);
            continue;
        }
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
        let attribute_name = &source[attribute_start..attribute_end];
        let quote = *bytes.get(cursor)?;
        if !matches!(quote, b'"' | b'\'') {
            let value_start = cursor;
            while cursor < tag_end && !bytes[cursor].is_ascii_whitespace() {
                cursor = advance_char(source, cursor);
            }
            if attribute_name.eq_ignore_ascii_case("rel") {
                has_stylesheet_rel = is_stylesheet_rel(&source[value_start..cursor]);
            } else if attribute_name.eq_ignore_ascii_case("href") {
                // HTML permits an unquoted attribute value. The value span
                // remains source-relative so only a resolved stylesheet link
                // is replaced and all surrounding bytes stay untouched.
                href = Some((value_start, cursor));
            }
            continue;
        }
        let value_start = cursor + 1;
        let value_end = value_start + source[value_start..tag_end].find(quote as char)?;
        if attribute_name.eq_ignore_ascii_case("rel") {
            has_stylesheet_rel = is_stylesheet_rel(&source[value_start..value_end]);
        } else if attribute_name.eq_ignore_ascii_case("href") {
            href = Some((value_start, value_end));
        }
        cursor = value_end + 1;
    }
    if has_stylesheet_rel { href } else { None }
}

fn is_stylesheet_rel(value: &str) -> bool {
    value
        .split_whitespace()
        .any(|token| token.eq_ignore_ascii_case("stylesheet"))
}

fn fragments_from_position_map(position_map: &PositionMap) -> Fragment {
    // PositionMap owns the canonical document-local payload-stream offset.
    // FRAG tag 6 uses it for `start`; insert_position remains the separate
    // SKEL/RawML insertion coordinate and must not be used to derive it.
    Fragment {
        entries: position_map
            .fragments
            .iter()
            .map(|fragment| FragmentEntry {
                insert_position: fragment.insert_position,
                file_number: fragment.file_number,
                sequence: fragment.sequence_number,
                start: fragment.payload_stream_start,
                length: fragment.payload_length,
            })
            .collect(),
    }
}

pub(crate) fn to_base32_fixed(value: u32, width: usize) -> Result<String> {
    let encoded = to_base32(value);
    if encoded.len() > width {
        return Err(crate::error::Error::Output(
            "base32 value exceeds fixed KF8 field width".to_owned(),
        ));
    }
    Ok(format!("{:0>width$}", encoded, width = width))
}
