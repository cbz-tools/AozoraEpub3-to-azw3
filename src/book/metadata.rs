#[derive(Debug, Clone, Default)]
pub struct Metadata {
    pub title: Option<String>,
    pub creator: Option<String>,
    pub language: Option<String>,
    pub identifier: Option<String>,
    pub publisher: Option<String>,
    pub description: Option<String>,
    pub cover: Option<String>,
    pub is_fixed_layout: bool,
    pub primary_writing_mode: Option<String>,
    pub book_type: Option<String>,
    pub orientation: Option<String>,
    pub orientation_lock: Option<String>,
    pub original_resolution: Option<String>,
}
