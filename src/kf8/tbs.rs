//! Calculate and encode text-record trailing bytes used by KF8 indexes.
//!
//! The pipeline maps navigation semantic entries onto text-record intersections,
//! classifies each local entry as an [`EntryAction`], and groups those entries
//! into strand/layer intermediate data. The groups become TBS sequences, which
//! are encoded as VWI values in each record's trailing data. The intermediate
//! form preserves hierarchy and cross-record spans before byte encoding.

use super::indx::encode_vwi;
use crate::error::Result;
use crate::kindle::KindleNavigationItem;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TbsEntry {
    pub(super) index: usize,
    pub(super) start: usize,
    pub(super) length: usize,
    pub(super) depth: usize,
    pub(super) parent: Option<usize>,
}

#[derive(Debug, Clone)]
pub(super) struct TbsSeed {
    pub(super) entry: TbsEntry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EntryAction {
    Spans,
    Ends,
    Starts,
    Completes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct LocalTbsEntry {
    pub(super) entry: TbsEntry,
    pub(super) action: EntryAction,
    pub(super) start_offset: isize,
    length_offset: isize,
    pub(super) text_record_length: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum TbsEncodingError {
    NegativeStrandIndex,
    ValueTooLarge,
    SiblingCountTooLarge,
}

// Classify one navigation span against a single text record. Keeping this
// action explicit preserves the distinction between starts, ends, and spans
// before the compact TBS representation is encoded.
fn fill_entry(entry: &TbsEntry, start_offset: isize, text_record_length: usize) -> LocalTbsEntry {
    let length_offset = start_offset + entry.length as isize;
    let action = if start_offset < 0 {
        if length_offset > text_record_length as isize {
            EntryAction::Spans
        } else {
            EntryAction::Ends
        }
    } else if length_offset > text_record_length as isize {
        EntryAction::Starts
    } else {
        EntryAction::Completes
    };
    LocalTbsEntry {
        entry: entry.clone(),
        action,
        start_offset,
        length_offset,
        text_record_length,
    }
}

// Follow parent/child relationships and contiguous siblings into one strand so
// hierarchy survives the later layer grouping and sequence encoding.
fn populate_strand(parent: LocalTbsEntry, entries: &mut Vec<LocalTbsEntry>) -> Vec<LocalTbsEntry> {
    let mut answer = vec![parent.clone()];
    let children = entries
        .iter()
        .position(|entry| entry.entry.parent == Some(parent.entry.index));
    if let Some(child) = children {
        let child = entries.remove(child);
        answer.extend(populate_strand(child, entries));
    } else {
        let mut current_index = parent.entry.index;
        let mut siblings = Vec::new();
        let mut index = 0;
        while index < entries.len() {
            let entry = &entries[index];
            if entry.entry.depth == parent.entry.depth
                && entry.entry.parent == parent.entry.parent
                && entry.entry.index == current_index + 1
            {
                let entry = entries.remove(index);
                current_index += 1;
                let has_children = entries
                    .iter()
                    .any(|candidate| candidate.entry.parent == Some(entry.entry.index));
                if has_children {
                    siblings.extend(populate_strand(entry, entries));
                    break;
                }
                siblings.push(entry);
            } else {
                index += 1;
            }
        }
        answer.extend(siblings);
    }
    answer
}

pub(super) type StrandLayers = Vec<(usize, Vec<LocalTbsEntry>)>;

// Split local entries into strands, then group each strand by navigation depth.
fn separate_strands(mut entries: Vec<LocalTbsEntry>) -> Vec<StrandLayers> {
    let mut answer = Vec::new();
    while !entries.is_empty() {
        let top = entries.remove(0);
        let strand = populate_strand(top, &mut entries);
        let mut layers: StrandLayers = Vec::new();
        for entry in strand {
            if let Some((_, layer)) = layers
                .iter_mut()
                .find(|(depth, _)| *depth == entry.entry.depth)
            {
                layer.push(entry);
            } else {
                layers.push((entry.entry.depth, vec![entry]));
            }
        }
        answer.push(layers);
    }
    answer
}

pub(super) fn collect_indexing_data(
    entries: &[TbsEntry],
    text_record_lengths: &[usize],
) -> Result<Vec<Vec<StrandLayers>>> {
    // Intersect navigation spans with the fixed text-record boundaries before
    // building strands; TBS records describe these local intersections.
    let mut sorted_entries = entries.to_vec();
    sorted_entries.sort_by_key(|entry| entry.start);
    let mut data = Vec::with_capacity(text_record_lengths.len());
    let mut record_start = 0usize;
    for &record_length in text_record_lengths {
        let next_record_start = record_start.checked_add(record_length).ok_or_else(|| {
            crate::error::Error::Output("TBS record position overflow".to_owned())
        })?;
        let mut local_entries = Vec::new();
        for entry in &sorted_entries {
            let entry_end = entry.start.checked_add(entry.length).ok_or_else(|| {
                crate::error::Error::Output("TBS entry position overflow".to_owned())
            })?;
            if entry.start >= next_record_start {
                break;
            }
            if entry_end <= record_start {
                continue;
            }
            let start_offset = isize::try_from(entry.start)
                .and_then(|start| isize::try_from(record_start).map(|record| start - record))
                .map_err(|_| {
                    crate::error::Error::Output("TBS position exceeds isize".to_owned())
                })?;
            local_entries.push(fill_entry(entry, start_offset, record_length));
        }
        data.push(separate_strands(local_entries));
        record_start = next_record_start;
    }
    Ok(data)
}

pub(super) fn calculate_all_tbs(
    indexing_data: &[Vec<StrandLayers>],
    tbs_type: u8,
) -> std::result::Result<Vec<Vec<u8>>, TbsEncodingError> {
    // Convert the hierarchy-preserving intermediate form into per-record TBS
    // trailing bytes, leaving all byte-level encoding in this module.
    indexing_data
        .iter()
        .map(|strands| {
            let sequences = encode_strands_as_sequences(strands, tbs_type)?;
            sequences_to_bytes(&sequences)
        })
        .collect()
}

#[derive(Debug, Clone, Copy)]
struct SequenceExtra {
    spans: bool,
    tbs_type: Option<u8>,
    count: Option<usize>,
    forwards: bool,
}

impl SequenceExtra {
    const fn empty() -> Self {
        Self {
            spans: false,
            tbs_type: None,
            count: None,
            forwards: false,
        }
    }

    fn flags(self) -> u8 {
        u8::from(self.spans)
            | (u8::from(self.tbs_type.is_some()) << 1)
            | (u8::from(self.count.is_some()) << 2)
            | (u8::from(self.forwards) << 3)
    }
}

#[derive(Debug, Clone, Copy)]
struct Sequence {
    value: usize,
    extra: SequenceExtra,
}

fn encode_strands_as_sequences(
    strands: &[StrandLayers],
    tbs_type: u8,
) -> std::result::Result<Vec<Sequence>, TbsEncodingError> {
    // Turn strand/layer entries into compact sequences while retaining span,
    // type, sibling-count, and direction flags required by the reader.
    let first_entry = strands
        .iter()
        .flat_map(|strand| strand.iter().flat_map(|(_, entries)| entries))
        .next()
        .map(|entry| entry.entry.index);
    let mut answer = Vec::new();
    let mut last_index = None;
    for strand in strands {
        let mut strand_sequences = Vec::new();
        for entries in strand.iter().map(|(_, entries)| entries) {
            let last = entries.last().expect("TBS layer cannot be empty");
            let first = &entries[0];
            let mut extra = SequenceExtra::empty();
            if last.action == EntryAction::Spans {
                extra.spans = true;
            }
            if first_entry == Some(first.entry.index) {
                extra.tbs_type = Some(tbs_type);
            }
            if entries.len() > 1 {
                extra.count = Some(entries.len());
            }
            let mut index = first.entry.index - first.entry.parent.unwrap_or(0);
            if !answer.is_empty() && strand_sequences.is_empty() {
                let delta = last_index.expect("later strand has a previous index") as isize
                    - first.entry.index as isize;
                if delta < 0 {
                    if tbs_type == 5 {
                        index = delta.unsigned_abs();
                    } else {
                        return Err(TbsEncodingError::NegativeStrandIndex);
                    }
                } else {
                    index = delta as usize;
                    extra.forwards = true;
                }
            }
            last_index = Some(last.entry.index);
            strand_sequences.push(Sequence {
                value: index,
                extra,
            });
        }
        for index in 0..strand_sequences.len().saturating_sub(1) {
            if strand_sequences[index].extra.spans && strand_sequences[index + 1].extra.spans {
                strand_sequences[index].extra.spans = false;
            }
        }
        answer.extend(strand_sequences);
    }
    Ok(answer)
}

fn sequences_to_bytes(sequences: &[Sequence]) -> std::result::Result<Vec<u8>, TbsEncodingError> {
    let mut answer = Vec::new();
    for (index, sequence) in sequences.iter().enumerate() {
        let flag_size = if index == 0 { 3 } else { 4 };
        answer.extend(encode_tbs_sequence(*sequence, flag_size)?);
    }
    Ok(answer)
}

fn encode_tbs_sequence(
    sequence: Sequence,
    flag_size: u32,
) -> std::result::Result<Vec<u8>, TbsEncodingError> {
    let flags = u32::from(sequence.extra.flags());
    let value = u32::try_from(sequence.value)
        .map_err(|_| TbsEncodingError::ValueTooLarge)?
        .checked_shl(flag_size)
        .and_then(|value| value.checked_add(flags))
        .ok_or(TbsEncodingError::ValueTooLarge)?;
    let mut answer = encode_vwi(value);
    if let Some(tbs_type) = sequence.extra.tbs_type {
        answer.extend(encode_vwi(u32::from(tbs_type)));
    }
    if let Some(count) = sequence.extra.count {
        answer.push(u8::try_from(count).map_err(|_| TbsEncodingError::SiblingCountTooLarge)?);
    }
    if sequence.extra.spans {
        answer.extend(encode_vwi(0));
    }
    Ok(answer)
}

pub(super) fn tbs_error(error: TbsEncodingError) -> crate::error::Error {
    let message = match error {
        TbsEncodingError::NegativeStrandIndex => "negative TBS strand index",
        TbsEncodingError::ValueTooLarge => "TBS value exceeds u32",
        TbsEncodingError::SiblingCountTooLarge => "TBS sibling count exceeds one byte",
    };
    crate::error::Error::Output(message.to_owned())
}

pub(super) fn flatten_tbs_seeds(
    items: &[KindleNavigationItem],
    depth: usize,
    parent: Option<usize>,
    output: &mut Vec<TbsSeed>,
) {
    for item in items {
        if item.href.is_empty() {
            // Unlinked EPUB navigation headings are represented in the
            // synthetic TOC only; TBS entries require a real position target.
            flatten_tbs_seeds(&item.children, depth, parent, output);
            continue;
        }
        let index = output.len();
        output.push(TbsSeed {
            entry: TbsEntry {
                index,
                start: 0,
                length: 0,
                depth,
                parent,
            },
        });
        flatten_tbs_seeds(&item.children, depth + 1, Some(index), output);
    }
}
