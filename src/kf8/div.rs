#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DivEntry {
    pub record: u32,
    pub offset: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Div {
    /// Diagnostic DIV coordinate table. Standalone KF8 serialization uses
    /// the FRAG index/CTOC path; this model keeps the legacy DIV invariants
    /// explicit without emitting a reader-incompatible fake DIV record.
    pub entries: Vec<DivEntry>,
}

impl Div {
    pub fn for_records(record_count: usize) -> Self {
        assert!(
            record_count <= u32::MAX as usize,
            "DIV record count must fit u32"
        );
        Self {
            entries: (0..record_count)
                .map(|record| DivEntry {
                    record: record as u32,
                    offset: 0,
                })
                .collect(),
        }
    }

    pub fn validate(&self, record_count: usize) -> crate::error::Result<()> {
        if self.entries.len() != record_count {
            return Err(crate::error::Error::Output(
                "DIV entry count does not match record count".to_owned(),
            ));
        }
        if self
            .entries
            .iter()
            .any(|entry| entry.record as usize >= record_count)
        {
            return Err(crate::error::Error::Output(
                "DIV record pointer is outside text records".to_owned(),
            ));
        }
        if self.entries.windows(2).any(|entries| {
            (entries[0].record, entries[0].offset) > (entries[1].record, entries[1].offset)
        }) {
            return Err(crate::error::Error::Output(
                "DIV pointers must be monotonic".to_owned(),
            ));
        }
        Ok(())
    }
}
