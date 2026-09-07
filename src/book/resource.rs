#[derive(Debug, Clone)]
pub struct Resource {
    pub id: String,
    pub href: String,
    pub media_type: String,
    pub properties: Vec<String>,
    pub data: Vec<u8>,
}

#[derive(Debug, Clone, Default)]
pub struct Resources {
    pub items: Vec<Resource>,
}
