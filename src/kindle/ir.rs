use crate::book::{Direction, PageProgression, WritingMode};

#[derive(Debug, Clone)]
pub struct KindleBook {
    pub metadata: KindleMetadata,
    pub layout: KindleLayout,
    pub sections: Vec<KindleSection>,
    pub navigation: Vec<KindleNavigationItem>,
    pub landmarks: Vec<KindleLandmark>,
    pub resources: Vec<KindleResource>,
}

#[derive(Debug, Clone)]
pub struct KindleResource {
    pub id: String,
    pub href: String,
    pub media_type: String,
    pub properties: Vec<String>,
    pub data: Vec<u8>,
}

pub(crate) type KindleDirection = Direction;
pub(crate) type KindlePageProgression = PageProgression;
pub(crate) type KindleWritingMode = WritingMode;

#[derive(Debug, Clone, Default)]
pub struct KindleMetadata {
    pub title: Option<String>,
    pub creator: Option<String>,
    pub language: Option<String>,
    // Semantic metadata is retained in the Kindle IR even though the current
    // minimal EXTH policy deliberately does not serialize an identifier.
    #[allow(dead_code)]
    pub identifier: Option<String>,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub cover_resource_id: Option<String>,
    pub is_fixed_layout: bool,
    pub primary_writing_mode: Option<String>,
    pub book_type: Option<String>,
    #[allow(dead_code)]
    pub orientation: Option<String>,
    pub orientation_lock: Option<String>,
    pub original_resolution: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KindleLayout {
    pub writing_mode: WritingMode,
    pub page_progression: PageProgression,
    pub direction: Direction,
}

#[derive(Debug, Clone)]
pub struct KindleSection {
    pub id: String,
    pub href: String,
    pub source_xhtml: String,
    /// Stylesheet hrefs linked by this document, kept in the Kindle IR so the
    /// KF8 writer does not emit unreferenced manifest CSS into the CSS flow.
    pub referenced_styles: Vec<String>,
    pub linear: bool,
    pub layout: KindleLayoutSemantic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KindleLayoutSemantic {
    #[default]
    Reflowable,
    PrePaginated,
}

#[derive(Debug, Clone, Default)]
pub struct KindleNavigationItem {
    pub label: String,
    pub href: String,
    pub children: Vec<KindleNavigationItem>,
}

#[derive(Debug, Clone, Default)]
pub struct KindleLandmark {
    pub kind: String,
    pub label: String,
    pub href: String,
}
