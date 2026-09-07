#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FdstEntry {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Fdst {
    pub entries: Vec<FdstEntry>,
}

impl Fdst {
    pub fn from_ranges(ranges: &[(u32, u32)]) -> Self {
        Self {
            entries: ranges
                .iter()
                .map(|&(start, end)| FdstEntry { start, end })
                .collect(),
        }
    }

    pub fn validate(&self, raw_length: u32) -> crate::error::Result<()> {
        if self.entries.len() > u32::MAX as usize {
            return Err(crate::error::Error::Output(
                "FDST entry count exceeds u32".to_owned(),
            ));
        }
        let mut previous_end = 0u32;
        for (index, entry) in self.entries.iter().enumerate() {
            if index == 0 && entry.start != 0 {
                return Err(crate::error::Error::Output(
                    "FDST first range must start at zero".to_owned(),
                ));
            }
            if entry.start > entry.end || entry.start < previous_end {
                return Err(crate::error::Error::Output(
                    "FDST ranges must be monotonic and non-negative".to_owned(),
                ));
            }
            previous_end = entry.end;
        }
        if previous_end != raw_length {
            return Err(crate::error::Error::Output(
                "FDST ranges must end at raw markup length".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn encode(&self) -> Vec<u8> {
        let raw_length = self.entries.last().map_or(0, |entry| entry.end);
        self.validate(raw_length)
            .expect("FDST invariants must hold before encoding");
        let mut bytes = Vec::with_capacity(12 + self.entries.len() * 8);
        bytes.extend_from_slice(b"FDST");
        // FDST section_table_offset points to the first range, immediately
        // after the 12-byte FDST header (calibre reads this at FDST + 4).
        bytes.extend_from_slice(&(FDST_HEADER_LEN as u32).to_be_bytes());
        bytes.extend_from_slice(&(self.entries.len() as u32).to_be_bytes());
        for entry in &self.entries {
            bytes.extend_from_slice(&entry.start.to_be_bytes());
            bytes.extend_from_slice(&entry.end.to_be_bytes());
        }
        bytes
    }
}

const FDST_HEADER_LEN: usize = 12;
