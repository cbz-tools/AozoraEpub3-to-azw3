//! KF8 pipeline components and low-level record encoders.
//!
//! MobileRead MOBI Wiki is treated as a reverse-engineered field-layout and
//! semantic reference, while calibre/Kindling and same-input KindleGen
//! comparisons provide writer behavior references. Physical Kindle behavior is
//! the final acceptance oracle; tentative or unknown fields are not promoted
//! to requirements without corroborating evidence.

mod builder;
mod css_flow;
mod div;
mod exth;
mod fcis;
mod fdst;
mod flis;
mod fragment;
mod fragmentize;
mod guide;
mod indx;
mod mobi_header;
mod ncx;
mod palmdoc;
mod position;
mod rawml;
mod resource;
mod serializer;
mod skel;
mod tbs;
mod text;

use crate::{error::Result, kindle::KindleBook};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TextCompression {
    None,
    PalmDoc,
}

pub(crate) use builder::{Kf8Book, Kf8Builder};
pub(crate) use rawml::SectionParts;
pub(crate) use serializer::serialize;

pub(crate) fn build(book: KindleBook, compression: TextCompression) -> Result<Kf8Book> {
    Kf8Builder::build_with_compression(book, matches!(compression, TextCompression::PalmDoc))
}
