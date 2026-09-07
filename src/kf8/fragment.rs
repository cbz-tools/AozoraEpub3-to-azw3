use super::indx::{
    EncodedIndex, EncodedIndexWithCtoc, RawIndexEntry, encode_ctoc_entries, encode_index_pair,
};
use crate::error::Result;

#[derive(Debug, Clone)]
/// One KF8 FRAG index entry.
///
/// `insert_position` is the SKEL/RawML insertion coordinate, while tag-6
/// `start` and `length` describe the owning document's concatenated fragment
/// payload stream. `start` resets for each `file_number` and advances by the
/// preceding payload lengths; it is neither a RawML absolute offset nor an
/// insertion position. `file_number` selects the owning document/skeleton and
/// `sequence` is the KF8 fragment-navigation sequence. Physical Kindle testing
/// confirmed that cumulative starts remove the nav repetition seen when every
/// fragment used zero. DOM insertion context is carried separately by each
/// fragment's CTOC selector; the DOM-aware SKEL/FRAG context in this baseline
/// preserves the inline-nav hierarchy on the acceptance book. `P`/`S` remain
/// the observed selector forms rather than a claim about wider proprietary
/// selector semantics.
pub struct FragmentEntry {
    pub insert_position: u32,
    pub file_number: u32,
    pub sequence: u32,
    pub start: u32,
    pub length: u32,
}

#[derive(Debug, Clone, Default)]
pub struct Fragment {
    pub entries: Vec<FragmentEntry>,
}

impl Fragment {
    pub fn validate(&self) -> Result<()> {
        let mut previous = None;
        for entry in &self.entries {
            entry
                .start
                .checked_add(entry.length)
                .ok_or_else(|| crate::error::Error::Output("fragment range overflow".to_owned()))?;
            let key = (entry.insert_position, entry.sequence);
            if previous.is_some_and(|previous| key < previous) {
                return Err(crate::error::Error::Output(
                    "fragment positions must be monotonic".to_owned(),
                ));
            }
            previous = Some(key);
        }
        Ok(())
    }

    /// Encode FRAG and its CTOC selectors. Tag 2 is a CTOC-record offset,
    /// while tag 6 is the document-local payload-stream start and length.
    /// `insert_position` remains the SKEL/RawML insertion coordinate. The
    /// selector is an observed `P`/`S` DOM-context form: it identifies the
    /// AID-bearing parent/sibling context retained in SKEL, rather than making
    /// every payload a body-level insertion. That context is required for the
    /// Physical Kindle hierarchy behavior of the acceptance navigation.
    pub fn encode_pair_with_ctoc(&self, selectors: &[String]) -> Result<EncodedIndexWithCtoc> {
        self.validate()?;
        if selectors.len() != self.entries.len() {
            return Err(crate::error::Error::Output(
                "fragment CTOC selector count does not match entries".to_owned(),
            ));
        }
        let (offsets, ctoc) = encode_ctoc_entries(selectors.iter().map(String::as_bytes))?;
        let ctoc_count = u32::try_from(ctoc.len()).map_err(|_| {
            crate::error::Error::Output("fragment CTOC count exceeds u32".to_owned())
        })?;
        let (main, details) = self.encode_pair_with_offsets(&offsets, ctoc_count)?;
        Ok((main, details, ctoc))
    }

    fn encode_pair_with_offsets(&self, offsets: &[u32], ctoc_count: u32) -> Result<EncodedIndex> {
        let entries = self
            .entries
            .iter()
            .zip(offsets)
            .map(|(entry, offset)| RawIndexEntry {
                text: format!("{:010}", entry.insert_position).into_bytes(),
                tags: vec![
                    (2, vec![*offset]),
                    (3, vec![entry.file_number]),
                    (4, vec![entry.sequence]),
                    (6, vec![entry.start, entry.length]),
                ],
            })
            .collect::<Vec<_>>();
        encode_index_pair(
            &entries,
            &[(2, 1, 1), (3, 1, 2), (4, 1, 4), (6, 2, 8)],
            ctoc_count,
        )
    }
}
