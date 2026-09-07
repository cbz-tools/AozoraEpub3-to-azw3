//! Classify already-prepared Kindle resources and compute their KF8 record geometry.
//!
//! This module computes placement geometry for prepared resource payloads but
//! never decodes or re-encodes images;
//! representation lowering belongs to `kindle::cover`. EXTH 201/202 offsets are
//! relative to First Image, so zero is valid when the first image is the target.

use crate::{error::Result, kindle::KindleResource};

#[derive(Debug, Clone, Copy)]
pub(crate) struct BinaryResourceGeometry {
    pub(crate) count: u32,
    pub(crate) first_image_record: u32,
    pub(crate) last_image_record: u16,
    pub(crate) cover_offset: Option<u32>,
    pub(crate) thumbnail_offset: Option<u32>,
}

impl BinaryResourceGeometry {
    pub(crate) fn compute(
        binary_resources: &[&KindleResource],
        resource_record_start: u32,
        cover_resource_id: Option<&str>,
        has_thumbnail: bool,
    ) -> Result<Self> {
        let count_usize = binary_resources
            .len()
            .checked_add(if has_thumbnail { 1 } else { 0 })
            .ok_or_else(|| crate::error::Error::Output("resource count overflow".to_owned()))?;
        let count = u32::try_from(count_usize)
            .map_err(|_| crate::error::Error::Output("resource count exceeds u32".to_owned()))?;
        let first_image_position = binary_resources
            .iter()
            .position(|resource| is_image_resource(resource));
        let first_image_record = if let Some(offset) = first_image_position {
            let offset = u32::try_from(offset).map_err(|_| {
                crate::error::Error::Output("image resource count exceeds u32".to_owned())
            })?;
            resource_record_start.checked_add(offset).ok_or_else(|| {
                crate::error::Error::Output("image record index overflow".to_owned())
            })?
        } else {
            u32::MAX
        };
        let last_image_position = if has_thumbnail {
            Some(count_usize - 1)
        } else {
            binary_resources
                .iter()
                .rposition(|resource| is_image_resource(resource))
        };
        let last_image_record = if let Some(offset) = last_image_position {
            let offset = u32::try_from(offset).map_err(|_| {
                crate::error::Error::Output("image resource count exceeds u32".to_owned())
            })?;
            let record = resource_record_start.checked_add(offset).ok_or_else(|| {
                crate::error::Error::Output("last image record index overflow".to_owned())
            })?;
            u16::try_from(record).map_err(|_| {
                crate::error::Error::Output("last image record index exceeds u16".to_owned())
            })?
        } else {
            u16::MAX
        };
        // EXTH 201 is relative to First Image, not an absolute record number.
        // Thus zero means that the native cover is First Image and is valid;
        // absence is represented by `None`, not by a zero offset.
        let cover_offset = match (cover_resource_id, first_image_position) {
            (Some(cover_id), Some(first_image_position)) => binary_resources
                .iter()
                .position(|resource| resource.id == cover_id)
                .map(|cover_position| {
                    cover_position
                        .checked_sub(first_image_position)
                        .ok_or_else(|| {
                            crate::error::Error::Output(
                                "cover precedes first image record".to_owned(),
                            )
                        })
                        .and_then(|offset| {
                            u32::try_from(offset).map_err(|_| {
                                crate::error::Error::Output("cover offset exceeds u32".to_owned())
                            })
                        })
                })
                .transpose()?,
            _ => None,
        };
        // EXTH 202 uses the same First Image-relative coordinate system as
        // EXTH 201; a missing thumbnail is represented separately by None.
        let thumbnail_offset = has_thumbnail
            .then(|| {
                first_image_position
                    .and_then(|first| u32::try_from(binary_resources.len() - first).ok())
            })
            .flatten();
        Ok(Self {
            count,
            first_image_record,
            last_image_record,
            cover_offset,
            thumbnail_offset,
        })
    }
}

pub(crate) fn is_text_resource(resource: &KindleResource) -> bool {
    [
        "application/xhtml+xml",
        "text/html",
        "application/x-dtbncx+xml",
    ]
    .iter()
    .any(|media_type| resource.media_type.eq_ignore_ascii_case(media_type))
}

pub(crate) fn is_css_resource(resource: &KindleResource) -> bool {
    resource.media_type.eq_ignore_ascii_case("text/css")
}

pub(crate) fn is_image_resource(resource: &KindleResource) -> bool {
    resource
        .media_type
        .get(.."image/".len())
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("image/"))
}

pub(crate) fn is_font_resource(resource: &KindleResource) -> bool {
    [
        "application/font-sfnt",
        "application/x-font-ttf",
        "font/sfnt",
        "font/ttf",
        "font/truetype",
        "font/otf",
        "font/opentype",
    ]
    .iter()
    .any(|media_type| resource.media_type.eq_ignore_ascii_case(media_type))
}

pub(crate) fn is_binary_resource(resource: &KindleResource) -> bool {
    !is_text_resource(resource) && !is_css_resource(resource)
}
