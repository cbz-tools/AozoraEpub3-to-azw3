//! Orchestrate the KF8 build pipeline and assemble the final record set.
//!
//! Subsystems own cover lowering, CSS/RawML preparation, geometry, indexes,
//! and coordinate semantics; this module coordinates their order and combines
//! their results without redefining those algorithms.

use super::css_flow::{
    CssResourceIndex, ResourceIndex, SectionIndex, css_resource_base_href,
    referenced_css_resources, rewrite_css_assets,
};
use super::div::Div;
use super::exth::ExthHeader;
use super::fcis::{encode_eof, encode_fcis};
use super::fdst::Fdst;
use super::flis::encode_flis;
use super::format::{to_base32, to_base32_fixed};
use super::fragment::{Fragment, FragmentEntry};
use super::guide::Guide;
use super::mobi_header::MobiHeader;
use super::ncx::Ncx;
use super::palmdoc::PalmDocHeader;
use super::position::{AnchorIndex, PositionMap, assign_aids};
use super::rawml::{
    PendingInternalLink, SectionParts, generated_layout_css, lower_pre_paginated_section,
    materialize_internal_links, rewrite_internal_links, rewrite_layout_class_for_document,
    rewrite_layout_fallback_link, rewrite_projected_attributes,
    rewrite_stylesheet_links_with_references, section_has_explicit_layout, split_section_parts,
};
use super::resc::encode as encode_resc;
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
    pub resc_record: u32,
    pub records: Vec<Kf8Record>,
}

pub(crate) struct Kf8Builder;

struct PreparedContent<'a> {
    sections: Vec<KindleSection>,
    resource_index: ResourceIndex<'a>,
    css_resources: CssResourceIndex<'a>,
    section_index: SectionIndex,
    page_flows: Vec<Vec<u8>>,
    pending_links: Vec<Vec<PendingInternalLink>>,
    uses_generated_layout: bool,
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
    skel: Skel,
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
    resc_sections: Vec<KindleSection>,
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
    resc_record: u32,
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
        let geometry = build_geometry(&book, prepared)?;
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
    let (resource_index, section_lookup, css_resources, layout_flow_number) =
        classify_content(sections, resources);
    let (sections, uses_generated_layout) = project_style_and_resources(
        sections,
        &resource_index,
        &css_resources,
        layout_flow_number,
        cover_resource_id,
    )?;
    let (mut sections, page_flows) =
        normalize_structural_content(sections, &css_resources, uses_generated_layout)?;
    let anchor_indices = assign_positioning_metadata(&mut sections)?;
    let pending_links = prepare_link_materialization(
        &mut sections,
        &section_lookup,
        &anchor_indices,
        &css_resources,
    )?;
    Ok(PreparedContent {
        sections,
        resource_index,
        css_resources,
        section_index: section_lookup,
        page_flows,
        pending_links,
        uses_generated_layout,
        library_thumbnail,
    })
}

fn classify_content<'a>(
    sections: &[KindleSection],
    resources: &'a [KindleResource],
) -> (
    ResourceIndex<'a>,
    SectionIndex,
    super::css_flow::CssResourceIndex<'a>,
    Option<u32>,
) {
    let resource_index = ResourceIndex::new(resources);
    let section_lookup = SectionIndex::new(sections);
    let css_resources = referenced_css_resources(sections, &resource_index, &section_lookup);
    let layout_flow_number = u32::try_from(css_resources.len())
        .ok()
        .and_then(|count| count.checked_add(1));
    (
        resource_index,
        section_lookup,
        css_resources,
        layout_flow_number,
    )
}

fn project_style_and_resources(
    sections: &mut Vec<KindleSection>,
    resource_index: &ResourceIndex<'_>,
    css_resources: &super::css_flow::CssResourceIndex<'_>,
    layout_flow_number: Option<u32>,
    cover_resource_id: Option<&str>,
) -> Result<(Vec<KindleSection>, bool)> {
    let mut projected = Vec::with_capacity(sections.len());
    let mut uses_generated_layout = false;
    for (index, section) in std::mem::take(sections).into_iter().enumerate() {
        let has_explicit_layout = section_has_explicit_layout(&section, resource_index);
        let layout_rewrite =
            rewrite_layout_class_for_document(section.source_xhtml, index, has_explicit_layout);
        uses_generated_layout |= layout_rewrite.uses_generated_layout;
        let source = layout_rewrite.source;
        let source = rewrite_stylesheet_links_with_references(
            source,
            &section.href,
            css_resources,
            Some(&section.referenced_styles),
        );
        let source = if layout_rewrite.uses_generated_layout {
            if let Some(flow_number) = layout_flow_number {
                rewrite_layout_fallback_link(source, flow_number)
            } else {
                source
            }
        } else {
            source
        };
        let source =
            rewrite_projected_attributes(source, &section.href, cover_resource_id, resource_index)?;
        projected.push(KindleSection {
            id: section.id,
            href: section.href,
            source_xhtml: source,
            referenced_styles: section.referenced_styles,
            linear: section.linear,
            layout: section.layout,
            rendition: section.rendition,
            source_properties: section.source_properties,
            source_spine_index: section.source_spine_index,
        });
    }
    Ok((projected, uses_generated_layout))
}

fn normalize_structural_content(
    mut sections: Vec<KindleSection>,
    css_resources: &super::css_flow::CssResourceIndex<'_>,
    uses_generated_layout: bool,
) -> Result<(Vec<KindleSection>, Vec<Vec<u8>>)> {
    for section in &mut sections {
        let source = std::mem::take(&mut section.source_xhtml);
        section.source_xhtml = super::rawml::materialize_ordered_list_values(source)?;
    }
    let page_flow_start = css_resources
        .len()
        .checked_add(usize::from(uses_generated_layout))
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
            super::css_flow::css_flow_number(&section.href, style_href, css_resources)
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
    Ok((sections, page_flows))
}

fn assign_positioning_metadata(sections: &mut [KindleSection]) -> Result<Vec<AnchorIndex>> {
    let mut next_aid = 0u32;
    let mut anchor_indices = Vec::with_capacity(sections.len());
    for section in &mut *sections {
        let source = std::mem::take(&mut section.source_xhtml);
        let assignment = assign_aids(source, &mut next_aid)?;
        section.source_xhtml = assignment.xhtml;
        anchor_indices.push(assignment.anchors);
    }
    Ok(anchor_indices)
}

fn prepare_link_materialization(
    sections: &mut [KindleSection],
    section_lookup: &SectionIndex,
    anchor_indices: &[AnchorIndex],
    css_resources: &super::css_flow::CssResourceIndex<'_>,
) -> Result<Vec<Vec<PendingInternalLink>>> {
    let mut pending_links = Vec::with_capacity(sections.len());
    for (section_number, section) in sections.iter_mut().enumerate() {
        let source = std::mem::take(&mut section.source_xhtml);
        let (source, links) = rewrite_internal_links(
            source,
            &section.href,
            section_lookup,
            section_number,
            anchor_indices,
            css_resources,
        )?;
        section.source_xhtml = source;
        pending_links.push(links);
    }
    Ok(pending_links)
}

fn build_geometry(book: &KindleBook, prepared: PreparedContent<'_>) -> Result<TextGeometry> {
    let PreparedContent {
        sections,
        resource_index,
        css_resources,
        section_index,
        page_flows,
        pending_links,
        uses_generated_layout,
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
    drop(pending_links);
    let mut css_flows = Vec::with_capacity(css_resources.len() + 1);
    for &resource in &css_resources.resources {
        let css_base_href = css_resource_base_href(&section_index, resource);
        css_flows.push(rewrite_css_assets(
            &resource.data,
            &css_base_href,
            &resource_index,
            &css_resources,
        ));
    }
    if uses_generated_layout {
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
    for section in &mut sections {
        drop(std::mem::take(&mut section.source_xhtml));
    }
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
        resc_sections,
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

    // RESC follows all binary resource records and precedes FLIS/FCIS/EOF.
    // The established MOBI header has no dedicated RESC pointer, so the
    // first-image/resource field below remains tied to image geometry.
    let resc_record = u32::try_from(records.len() + 1)
        .map_err(|_| crate::error::Error::Output("RESC record index overflow".to_owned()))?;
    records.push(Kf8Record {
        data: encode_resc(
            &resc_sections,
            book.metadata.rendition,
            book.metadata.rendition_viewport.as_deref(),
        )?,
    });

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
        resc_record,
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
        resc_record,
        geometry,
    } = layout;
    let mut exth = ExthHeader::default();
    if book.metadata.authors.is_empty() {
        if let Some(value) = &book.metadata.creator {
            exth.push_text(100, value);
        }
    } else {
        for value in &book.metadata.authors {
            exth.push_text(100, value);
        }
    }
    for value in &book.metadata.contributors {
        exth.push_text(108, value);
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
    if let Some(value) = &book.metadata.title_file_as {
        exth.push_text(508, value);
    }
    if let Some(value) = &book.metadata.creator_file_as {
        exth.push_text(517, value);
    }
    if let Some(value) = &book.metadata.publisher_file_as {
        exth.push_text(522, value);
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
        if let Some(value) = book.metadata.book_type.as_deref().map(str::trim) {
            let value = if value.eq_ignore_ascii_case("comic") {
                Some("comic")
            } else if value.eq_ignore_ascii_case("children") {
                Some("children")
            } else {
                None
            };
            if let Some(value) = value {
                exth.push_text(123, value);
            }
        }
    }
    let orientation = if book.metadata.is_fixed_layout {
        book.metadata
            .orientation_lock
            .as_deref()
            .or(book.metadata.orientation.as_deref())
    } else {
        book.metadata.orientation.as_deref()
    };
    if let Some(value) = orientation {
        let value = if value.eq_ignore_ascii_case("auto") {
            "none"
        } else {
            value
        };
        exth.push_text(124, value);
    }
    if book.metadata.is_fixed_layout {
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
        resc_record,
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
    let page_flow_lengths = page_flows.iter().map(Vec::len).collect::<Vec<_>>();
    drop(page_flows);
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
    let mut skel_offset = 0u32;
    let mut skel_entries = Vec::with_capacity(section_parts.len());
    let mut xhtml_length = 0usize;
    for parts in &section_parts {
        let skel_start = skel_offset;
        let skeleton_length = u32::try_from(parts.skeleton.len()).map_err(|_| {
            crate::error::Error::Output("SKEL section length exceeds u32".to_owned())
        })?;
        let fragments_length = parts.fragments.iter().try_fold(0usize, |total, fragment| {
            total
                .checked_add(fragment.len())
                .ok_or_else(|| crate::error::Error::Output("XHTML length overflow".to_owned()))
        })?;
        let fragments_length_u32 = u32::try_from(fragments_length)
            .map_err(|_| crate::error::Error::Output("SKEL position overflow".to_owned()))?;
        let section_length = parts
            .skeleton
            .len()
            .checked_add(fragments_length)
            .ok_or_else(|| crate::error::Error::Output("XHTML length overflow".to_owned()))?;
        xhtml_length = xhtml_length
            .checked_add(section_length)
            .ok_or_else(|| crate::error::Error::Output("XHTML length overflow".to_owned()))?;
        skel_offset = skel_offset
            .checked_add(skeleton_length)
            .and_then(|offset| offset.checked_add(fragments_length_u32))
            .ok_or_else(|| crate::error::Error::Output("SKEL position overflow".to_owned()))?;
        skel_entries.push(SkelEntry {
            fragment_count: u32::try_from(parts.fragments.len()).map_err(|_| {
                crate::error::Error::Output("SKEL fragment count exceeds u32".to_owned())
            })?,
            start: skel_start,
            length: skeleton_length,
        });
    }
    let skel = Skel {
        entries: skel_entries,
    };
    drop(section_parts);
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
        skel,
        position_map,
        text_records,
        text_length_u32,
        text_record_count,
        xhtml_length_u32,
        css_flow_lengths,
        page_flow_lengths,
        library_thumbnail,
    })
}

fn build_indexes(book: &KindleBook, text: TextData) -> Result<Indexes> {
    let TextData {
        sections,
        skel,
        position_map,
        text_records,
        text_length_u32,
        text_record_count,
        xhtml_length_u32,
        css_flow_lengths,
        page_flow_lengths,
        library_thumbnail,
    } = text;
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
        resc_sections: sections,
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
