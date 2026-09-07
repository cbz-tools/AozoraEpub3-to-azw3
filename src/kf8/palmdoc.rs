#[derive(Debug, Clone, Copy)]
/// PalmDOC header for the continuous fixed-size decoded text-record stream.
/// KF8 uses `record_size = 4096`; callers must preserve that global boundary so
/// trailing UTF-8 overlap/TBS data and absolute RawML-derived coordinates remain
/// aligned across sections and flows.
pub struct PalmDocHeader {
    pub compression: u16,
    pub text_length: u32,
    pub record_count: u16,
    pub record_size: u16,
    pub encryption: u16,
}

impl PalmDocHeader {
    pub fn validate(&self) -> crate::error::Result<()> {
        if !matches!(self.compression, 1 | 2) {
            return Err(crate::error::Error::Output(
                "unsupported PalmDOC compression".to_owned(),
            ));
        }
        if self.record_size == 0 {
            return Err(crate::error::Error::Output(
                "PalmDOC record size must be non-zero".to_owned(),
            ));
        }
        if self.encryption != 0 {
            return Err(crate::error::Error::Output(
                "encrypted PalmDOC output is not supported".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn encode(self) -> Vec<u8> {
        self.validate()
            .expect("PalmDOC invariants must hold before encoding");
        let mut bytes = Vec::with_capacity(16);
        bytes.extend_from_slice(&self.compression.to_be_bytes());
        bytes.extend_from_slice(&0u16.to_be_bytes());
        bytes.extend_from_slice(&self.text_length.to_be_bytes());
        bytes.extend_from_slice(&self.record_count.to_be_bytes());
        bytes.extend_from_slice(&self.record_size.to_be_bytes());
        bytes.extend_from_slice(&self.encryption.to_be_bytes());
        bytes.extend_from_slice(&0u16.to_be_bytes());
        bytes
    }
}
