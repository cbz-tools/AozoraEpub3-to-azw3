//! Release integration coverage grouped by conversion responsibility.

mod support;

#[path = "e2e/accessibility.rs"]
mod accessibility;
#[path = "e2e/content_documents.rs"]
mod content_documents;
#[path = "e2e/cover_resources.rs"]
mod cover_resources;
#[path = "e2e/css_safety.rs"]
mod css_safety;
#[path = "e2e/css_transport.rs"]
mod css_transport;
#[path = "e2e/embedded_fonts.rs"]
mod embedded_fonts;
#[path = "e2e/fixed_layout.rs"]
mod fixed_layout;
#[path = "e2e/layout_rendition_media.rs"]
mod layout_rendition_media;
#[path = "e2e/mixed_layout.rs"]
mod mixed_layout;
#[path = "e2e/navigation.rs"]
mod navigation;
#[path = "e2e/ocf_container.rs"]
mod ocf_container;
#[path = "e2e/package_metadata.rs"]
mod package_metadata;
#[path = "e2e/resc.rs"]
mod resc;
#[path = "e2e/resource_resolution.rs"]
mod resource_resolution;
#[path = "e2e/unicode.rs"]
mod unicode;
