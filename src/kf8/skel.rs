use super::indx::{EncodedIndex, RawIndexEntry, encode_index_pair};
use crate::error::Result;

#[derive(Debug, Clone)]
/// One document skeleton/shell range in reconstructed RawML.
///
/// SKEL geometry describes the document shell; FRAG `insert_position` names
/// where payload is inserted into that shell, and FRAG tag-6 geometry covers
/// the document-local fragment-payload stream instead.
pub struct SkelEntry {
    pub fragment_count: u32,
    pub start: u32,
    pub length: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Skel {
    pub entries: Vec<SkelEntry>,
}

impl Skel {
    pub fn validate(&self) -> Result<()> {
        let mut previous_end = 0u32;
        for entry in &self.entries {
            let end = entry
                .start
                .checked_add(entry.length)
                .ok_or_else(|| crate::error::Error::Output("SKEL range overflow".to_owned()))?;
            if entry.start < previous_end {
                return Err(crate::error::Error::Output(
                    "SKEL ranges must be monotonic".to_owned(),
                ));
            }
            previous_end = end;
        }
        Ok(())
    }

    pub fn encode_pair(&self) -> Result<EncodedIndex> {
        self.validate()?;
        let entries = self
            .entries
            .iter()
            .enumerate()
            .map(|(index, entry)| RawIndexEntry {
                text: format!("SKEL{index:010}").into_bytes(),
                tags: vec![
                    // Calibre/Kindling encode these as two occurrences under
                    // packed masks: duplicate values are intentional and let
                    // the TAGX reader recover the occurrence count.
                    (1, vec![entry.fragment_count, entry.fragment_count]),
                    (
                        6,
                        vec![entry.start, entry.length, entry.start, entry.length],
                    ),
                ],
            })
            .collect::<Vec<_>>();
        encode_index_pair(&entries, &[(1, 1, 3), (6, 2, 12)], 0)
    }
}
