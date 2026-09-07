mod content;
mod layout;
mod metadata;
mod resource;
mod style;

pub(crate) use content::plain_display_text;
pub use content::{ContentDocument, SemanticDocument};
pub use layout::{Direction, Layout, PageProgression, WritingMode};
pub use metadata::Metadata;
pub use resource::{Resource, Resources};
pub use style::{CssDeclaration, CssRule, StyleSheet, Styles};

#[derive(Debug, Clone, Default)]
pub struct Book {
    pub metadata: Metadata,
    pub reading_order: ReadingOrder,
    pub navigation: Navigation,
    pub content: Vec<ContentDocument>,
    pub resources: Resources,
    pub layout: Layout,
    // The parsed stylesheet graph is part of the semantic Book IR even though
    // KF8 transport currently reads the original CSS resources instead.
    #[allow(dead_code)]
    pub styles: Styles,
}

#[derive(Debug, Clone, Default)]
pub struct ReadingOrder {
    pub items: Vec<ReadingOrderItem>,
    // Retained from the EPUB spine metadata; the Kindle projection uses the
    // normalized book layout while preserving this source-level value.
    #[allow(dead_code)]
    pub page_progression: PageProgression,
}

#[derive(Debug, Clone)]
pub struct ReadingOrderItem {
    pub id: String,
    pub href: String,
    // Preserved source metadata; section selection is keyed by the manifest
    // item ID and the writer emits the normalized Kindle section type.
    #[allow(dead_code)]
    pub media_type: String,
    pub linear: bool,
}

#[derive(Debug, Clone, Default)]
pub struct Navigation {
    // The visible TOC is represented by `items`; the source title remains in
    // the semantic IR for consumers that need EPUB navigation metadata.
    #[allow(dead_code)]
    pub title: Option<String>,
    pub items: Vec<NavigationItem>,
    /// Semantic routes from the EPUB navigation landmarks section. These are
    /// retained even when an NCX is selected as the visible TOC source.
    pub landmarks: Vec<NavigationLandmark>,
}

#[derive(Debug, Clone, Default)]
pub struct NavigationLandmark {
    pub kind: String,
    pub label: String,
    pub href: String,
}

#[derive(Debug, Clone, Default)]
pub struct NavigationItem {
    pub label: String,
    pub href: String,
    pub children: Vec<NavigationItem>,
}
