use super::indx::{EncodedIndexWithCtoc, RawIndexEntry, encode_ctoc_entries, encode_index_pair};
use super::position::GuidePosition;
use crate::book::plain_display_text;
use crate::error::Result;

#[derive(Debug, Clone, Default)]
/// Guide routes use KF8 fid/off navigation coordinates for landmarks.
/// EXTH 116 separately stores the absolute RawML Start Reading position.
pub(crate) struct Guide {
    positions: Vec<GuidePosition>,
}

impl Guide {
    pub(crate) fn from_positions(positions: Vec<GuidePosition>) -> Self {
        Self { positions }
    }

    pub(crate) fn encode_pair(&self) -> Result<EncodedIndexWithCtoc> {
        if self.positions.is_empty() {
            return Ok((Vec::new(), Vec::new(), Vec::new()));
        }
        let mut entries = Vec::with_capacity(self.positions.len());
        let labels = self
            .positions
            .iter()
            .map(|position| plain_display_text(&position.label))
            .collect::<Vec<_>>();
        let (offsets, ctoc) = encode_ctoc_entries(labels.iter().map(String::as_bytes))?;
        for (position, ctoc_offset) in self.positions.iter().zip(offsets) {
            entries.push(RawIndexEntry {
                text: position.kind.as_bytes().to_vec(),
                tags: vec![
                    (1, vec![ctoc_offset]),
                    (6, vec![position.sequence_number, position.off]),
                ],
            });
        }
        let ctoc_count = u32::try_from(ctoc.len())
            .map_err(|_| crate::error::Error::Output("Guide CTOC count exceeds u32".to_owned()))?;
        let (main, details) = encode_index_pair(&entries, &[(1, 1, 1), (6, 2, 2)], ctoc_count)?;
        Ok((main, details, ctoc))
    }
}
