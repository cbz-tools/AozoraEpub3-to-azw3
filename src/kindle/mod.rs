mod cover;
mod css;
mod ir;
mod normalize;

pub(crate) use cover::{generate_library_thumbnail, prepare_cover_resource};
pub(crate) use css::project_css_for_kindle;
pub use ir::{
    KindleBook, KindleLandmark, KindleLayout, KindleLayoutSemantic, KindleNavigationItem,
    KindleResource, KindleSection,
};
pub(crate) use ir::{KindleDirection, KindlePageProgression, KindleWritingMode};
pub(crate) use normalize::normalize;
