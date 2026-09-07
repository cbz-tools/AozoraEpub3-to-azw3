use crate::error::{Error, Result};

const INDEX_HEADER_LEN: usize = 0xc0;
const INDEX_HEADER_WORDS: usize = 13;
const TAGX_HEADER_LEN: usize = 12;
const IDXT_ENTRY_LEN: usize = 2;
const MAX_INDEX_TEXT_LEN: usize = u8::MAX as usize;

#[derive(Debug, Clone)]
pub(crate) struct RawIndexEntry {
    pub(crate) text: Vec<u8>,
    pub(crate) tags: Vec<(u8, Vec<u32>)>,
}

/// A TAGX descriptor: tag number, values per occurrence, and the packed
/// control-byte mask.  Some Kindle indexes use more than one control bit for
/// a repeated value group (for example SKEL tag 1 and tag 6); treating every
/// mask as a single bit loses that occurrence count.
pub(crate) type TagDefinition = (u8, u8, u8);

pub(crate) type EncodedIndex = (Vec<u8>, Vec<Vec<u8>>);
pub(crate) type EncodedIndexWithCtoc = (Vec<u8>, Vec<Vec<u8>>, Vec<Vec<u8>>);

const CTOC_PAGE_LEN: usize = 0x1_0000;
const MAX_CTOC_RECORD_LEN: usize = u16::MAX as usize;

#[derive(Debug, Clone)]
struct DetailRouting {
    first_entry: usize,
    entry_count: usize,
    bytes: Vec<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IndexKind {
    Fragment,
    Skeleton,
    Ncx,
    Guide,
}

/// Encode the main/detail INDX pair consumed by KindleUnpack's MobiIndex.
pub(crate) fn encode_index_pair(
    entries: &[RawIndexEntry],
    tags: &[TagDefinition],
    nctoc: u32,
) -> Result<EncodedIndex> {
    if entries.is_empty() {
        return Err(Error::Output(
            "INDX pair requires at least one detail entry".to_owned(),
        ));
    }
    if entries.len() > u32::MAX as usize {
        return Err(Error::Output("too many INDX entries".to_owned()));
    }
    if tags.len() > 8 {
        return Err(Error::Output("INDX supports at most 8 tags".to_owned()));
    }
    validate_tags(tags)?;
    validate_entries(entries, tags)?;
    let kind = index_kind(tags, nctoc);
    let detail_rows = entries
        .iter()
        .map(|entry| encode_detail_row(entry, tags))
        .collect::<Result<Vec<_>>>()?;
    let details = encode_details(&detail_rows, entries.len())?;
    let main = encode_main(entries, &details, nctoc, tags, kind)?;
    Ok((
        main,
        details.into_iter().map(|detail| detail.bytes).collect(),
    ))
}

fn encode_main(
    entries: &[RawIndexEntry],
    details: &[DetailRouting],
    nctoc: u32,
    tags: &[TagDefinition],
    kind: IndexKind,
) -> Result<Vec<u8>> {
    let entry_count = u32::try_from(entries.len())
        .map_err(|_| Error::Output("too many INDX entries".to_owned()))?;
    let detail_count = u32::try_from(details.len())
        .map_err(|_| Error::Output("INDX detail-record count exceeds u32".to_owned()))?;
    let mut bytes = main_index_header(entry_count, detail_count, nctoc);
    bytes.resize(INDEX_HEADER_LEN, 0);
    // The standalone KF8 routing contract stores the fixed-header offset at
    // primary +0xb4; its value is +0xc0. Main and detail use the same final
    // routing key, while detail keeps the 0xffffffff encoding sentinel and
    // zero total-count shape expected by the reference readers.
    bytes[0xb4..0xb8].copy_from_slice(&(INDEX_HEADER_LEN as u32).to_be_bytes());
    let tagx_length = TAGX_HEADER_LEN
        .checked_add(
            (tags.len() + 1)
                .checked_mul(4)
                .ok_or_else(|| Error::Output("INDX TAGX descriptor length overflow".to_owned()))?,
        )
        .ok_or_else(|| Error::Output("INDX TAGX length overflow".to_owned()))?;
    bytes.extend_from_slice(b"TAGX");
    bytes.extend_from_slice(
        &u32::try_from(tagx_length)
            .map_err(|_| Error::Output("INDX TAGX length exceeds u32".to_owned()))?
            .to_be_bytes(),
    );
    bytes.extend_from_slice(&1u32.to_be_bytes());
    for (tag, values_per_entry, mask) in tags.iter().copied() {
        bytes.extend_from_slice(&[tag, values_per_entry, mask, 0]);
    }
    bytes.extend_from_slice(&[0, 0, 0, 1]);
    // Main INDX records contain one metadata grouping row per detail.  Its
    // label is index-specific: a generic KF8 row loses the distinction between
    // the FRAG, SKEL, NCX, and Guide schemas and is not used by the standalone
    // reference shape.
    let mut row_positions = Vec::with_capacity(details.len());
    for detail in details {
        let end = detail
            .first_entry
            .checked_add(detail.entry_count)
            .ok_or_else(|| Error::Output("INDX detail range overflow".to_owned()))?;
        let detail_entries = entries.get(detail.first_entry..end).ok_or_else(|| {
            Error::Output("INDX detail metadata does not cover source entries".to_owned())
        })?;
        let label = grouping_label(kind, detail_entries)?;
        let entry_count = u16::try_from(detail.entry_count)
            .map_err(|_| Error::Output("INDX grouping count exceeds u16".to_owned()))?;
        let row_position = u16::try_from(bytes.len())
            .map_err(|_| Error::Output("INDX grouping row offset exceeds u16".to_owned()))?;
        row_positions.push(row_position);
        bytes.push(
            u8::try_from(label.len())
                .map_err(|_| Error::Output("INDX grouping label exceeds u8 length".to_owned()))?,
        );
        bytes.extend_from_slice(&label);
        bytes.extend_from_slice(&entry_count.to_be_bytes());
        while bytes.len() % 4 != 0 {
            bytes.push(0);
        }
    }
    let idxt_position = bytes.len();
    bytes.extend_from_slice(b"IDXT");
    for row_position in row_positions {
        bytes.extend_from_slice(&row_position.to_be_bytes());
    }
    bytes[20..24].copy_from_slice(&(idxt_position as u32).to_be_bytes());
    while bytes.len() % 4 != 0 {
        bytes.push(0);
    }
    Ok(bytes)
}

fn index_kind(tags: &[TagDefinition], nctoc: u32) -> IndexKind {
    if tags == [(2, 1, 1), (3, 1, 2), (4, 1, 4), (6, 2, 8)] {
        IndexKind::Fragment
    } else if tags == [(1, 1, 3), (6, 2, 12)] {
        IndexKind::Skeleton
    } else if nctoc > 0 && tags == [(1, 1, 1), (6, 2, 2)] {
        IndexKind::Guide
    } else {
        IndexKind::Ncx
    }
}

fn grouping_label(kind: IndexKind, entries: &[RawIndexEntry]) -> Result<Vec<u8>> {
    match kind {
        // Main routing labels are the final detail-row keys verbatim.  The
        // producers establish the schema-specific canonical encoding.
        IndexKind::Fragment => entries
            .last()
            .map(|entry| entry.text.clone())
            .ok_or_else(|| Error::Output("FRAG index cannot be empty".to_owned())),
        IndexKind::Skeleton => entries
            .last()
            .map(|entry| entry.text.clone())
            .ok_or_else(|| Error::Output("SKEL index cannot be empty".to_owned())),
        IndexKind::Ncx => entries
            .last()
            .map(|entry| entry.text.clone())
            .ok_or_else(|| Error::Output("NCX index cannot be empty".to_owned())),
        // Guide follows the same final-detail-key convention; local reference
        // records use the emitted guide kind, normally "toc".
        IndexKind::Guide => Ok(entries
            .last()
            .map(|entry| entry.text.clone())
            .filter(|label| !label.is_empty())
            .unwrap_or_else(|| b"toc".to_vec())),
    }
}

/// Encode CTOC strings into the page-addressed records used by KF8. Index
/// entries store a global offset whose page base advances by 0x10000 for each
/// CTOC record; keep each string whole so KindleUnpack can parse every record
/// independently. Non-final pages are padded to that same address stride so
/// readers that concatenate the records resolve the offsets too.
pub(crate) fn encode_ctoc_entries<'a, I>(entries: I) -> Result<(Vec<u32>, Vec<Vec<u8>>)>
where
    I: IntoIterator<Item = &'a [u8]>,
{
    let mut offsets = Vec::new();
    let mut chunks = vec![Vec::new()];
    for entry in entries {
        let length = u32::try_from(entry.len())
            .map_err(|_| Error::Output("CTOC entry exceeds u32".to_owned()))?;
        let encoded_length = encode_vwi(length)
            .len()
            .checked_add(entry.len())
            .ok_or_else(|| Error::Output("CTOC entry length overflow".to_owned()))?;
        if encoded_length > MAX_CTOC_RECORD_LEN {
            return Err(Error::Output(
                "CTOC entry cannot fit in a u16-offset record".to_owned(),
            ));
        }
        let current_len = chunks
            .last()
            .map(Vec::len)
            .ok_or_else(|| Error::Output("CTOC record list is empty".to_owned()))?;
        if current_len
            .checked_add(encoded_length)
            .ok_or_else(|| Error::Output("CTOC record length overflow".to_owned()))?
            > MAX_CTOC_RECORD_LEN
        {
            chunks.push(Vec::new());
        }
        let page_base = chunks
            .len()
            .checked_sub(1)
            .and_then(|page| page.checked_mul(CTOC_PAGE_LEN))
            .and_then(|base| base.checked_add(chunks.last().map(Vec::len)?))
            .ok_or_else(|| Error::Output("CTOC offset overflow".to_owned()))?;
        offsets.push(
            u32::try_from(page_base)
                .map_err(|_| Error::Output("CTOC offset exceeds u32".to_owned()))?,
        );
        let chunk = chunks
            .last_mut()
            .ok_or_else(|| Error::Output("CTOC record list is empty".to_owned()))?;
        chunk.extend_from_slice(&encode_vwi(length));
        chunk.extend_from_slice(entry);
    }
    if chunks.len() > 1 {
        let final_chunk_index = chunks.len() - 1;
        for chunk in &mut chunks[..final_chunk_index] {
            chunk.resize(CTOC_PAGE_LEN, 0);
        }
    }
    Ok((offsets, chunks))
}

fn encode_detail_row(entry: &RawIndexEntry, tags: &[TagDefinition]) -> Result<Vec<u8>> {
    if entry.text.len() > MAX_INDEX_TEXT_LEN {
        return Err(Error::Output(
            "INDX entry text exceeds 255 bytes".to_owned(),
        ));
    }
    let mut row = Vec::with_capacity(entry.text.len() + 2);
    row.push(
        u8::try_from(entry.text.len())
            .map_err(|_| Error::Output("INDX entry text exceeds 255 bytes".to_owned()))?,
    );
    row.extend_from_slice(&entry.text);
    let mut control = 0u8;
    for (tag, values_per_entry, mask) in tags {
        let Some((_, values)) = entry.tags.iter().find(|(entry_tag, _)| entry_tag == tag) else {
            continue;
        };
        let occurrences = values.len() / usize::from(*values_per_entry);
        let shift = mask.trailing_zeros();
        control |= (u8::try_from(occurrences)
            .map_err(|_| Error::Output(format!("INDX tag {tag} occurrence count exceeds u8")))?
            << shift)
            & mask;
    }
    row.push(control);
    for (tag, _, _) in tags {
        if let Some((_, values)) = entry.tags.iter().find(|(entry_tag, _)| entry_tag == tag) {
            for value in values {
                row.extend_from_slice(&encode_vwi(*value));
            }
        }
    }
    Ok(row)
}

fn encode_details(detail_rows: &[Vec<u8>], entry_count: usize) -> Result<Vec<DetailRouting>> {
    let mut details = Vec::new();
    let mut first_entry = 0usize;
    while first_entry < detail_rows.len() {
        let mut row_count = 0usize;
        let mut row_bytes = 0usize;
        while first_entry + row_count < detail_rows.len() {
            let row_len = detail_rows[first_entry + row_count].len();
            let next_row_bytes = row_bytes
                .checked_add(row_len)
                .ok_or_else(|| Error::Output("INDX detail row length overflow".to_owned()))?;
            let next_row_count = row_count
                .checked_add(1)
                .ok_or_else(|| Error::Output("INDX detail row count overflow".to_owned()))?;
            let idxt_start = INDEX_HEADER_LEN
                .checked_add(next_row_bytes)
                .ok_or_else(|| Error::Output("INDX IDXT offset overflow".to_owned()))?;
            let record_len = idxt_start
                .checked_add(4)
                .and_then(|value| value.checked_add(next_row_count.checked_mul(IDXT_ENTRY_LEN)?))
                .ok_or_else(|| Error::Output("INDX detail record overflow".to_owned()))?;
            if u16::try_from(idxt_start).is_err() || u16::try_from(record_len).is_err() {
                break;
            }
            row_bytes = next_row_bytes;
            row_count = next_row_count;
        }
        if row_count == 0 {
            return Err(Error::Output(format!(
                "INDX entry {first_entry} cannot fit in a u16-offset detail record"
            )));
        }
        let detail = encode_detail_chunk(
            &detail_rows[first_entry..first_entry + row_count],
            first_entry,
        )?;
        details.push(DetailRouting {
            first_entry,
            entry_count: row_count,
            bytes: detail,
        });
        first_entry += row_count;
    }
    if details.is_empty()
        || details
            .iter()
            .map(|detail| detail.entry_count)
            .sum::<usize>()
            != entry_count
    {
        return Err(Error::Output(
            "INDX detail chunks do not cover all entries".to_owned(),
        ));
    }
    Ok(details)
}

fn encode_detail_chunk(detail_rows: &[Vec<u8>], first_entry: usize) -> Result<Vec<u8>> {
    let entry_count = u32::try_from(detail_rows.len())
        .map_err(|_| Error::Output("too many INDX entries".to_owned()))?;
    let mut bytes = detail_index_header(entry_count);
    bytes.resize(INDEX_HEADER_LEN, 0);
    let mut positions = Vec::with_capacity(detail_rows.len());
    for row in detail_rows {
        let position = u16::try_from(bytes.len()).map_err(|_| {
            Error::Output(format!("INDX entry {first_entry} exceeds u16 row offsets"))
        })?;
        positions.push(position);
        bytes.extend_from_slice(row);
    }
    let idxt_start = u16::try_from(bytes.len())
        .map_err(|_| Error::Output("INDX IDXT offset exceeds u16 row offsets".to_owned()))?;
    let record_len = usize::from(idxt_start)
        .checked_add(4)
        .and_then(|value| value.checked_add(positions.len().checked_mul(IDXT_ENTRY_LEN)?))
        .ok_or_else(|| Error::Output("INDX detail record overflow".to_owned()))?;
    u16::try_from(record_len)
        .map_err(|_| Error::Output("INDX detail record exceeds u16 offsets".to_owned()))?;
    bytes.extend_from_slice(b"IDXT");
    for position in positions {
        bytes.extend_from_slice(&position.to_be_bytes());
    }
    bytes[20..24].copy_from_slice(&u32::from(idxt_start).to_be_bytes());
    Ok(bytes)
}

/// Primary/header INDX semantics: one routing row, UTF-8 key encoding, the
/// contiguous detail-record count at offset 0x18, and the total entry count
/// at offset 0x24.
fn main_index_header(entry_count: u32, detail_count: u32, nctoc: u32) -> Vec<u8> {
    index_header_words([
        INDEX_HEADER_LEN as u32,
        0,
        0,
        2,
        0,
        detail_count,
        65001,
        0xffff_ffff,
        entry_count,
        0,
        0,
        0,
        nctoc,
    ])
}

/// Detail/data INDX semantics: the detail row count is carried at offset
/// 0x18, while offset 0x1c is the null encoding sentinel and offset 0x24 is
/// the zero total-count field used by the reference writers.  The IDXT
/// offset is filled after rows are encoded.
fn detail_index_header(entry_count: u32) -> Vec<u8> {
    index_header_words([
        INDEX_HEADER_LEN as u32,
        0,
        1,
        0,
        0,
        entry_count,
        0xffff_ffff,
        0xffff_ffff,
        0,
        0,
        0,
        0,
        0,
    ])
}

fn index_header_words(values: [u32; INDEX_HEADER_WORDS]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(INDEX_HEADER_WORDS * 4);
    bytes.extend_from_slice(b"INDX");
    for value in values {
        bytes.extend_from_slice(&value.to_be_bytes());
    }
    bytes
}

pub(crate) fn encode_vwi(value: u32) -> Vec<u8> {
    let mut chunks = vec![value & 0x7f];
    let mut remaining = value >> 7;
    while remaining != 0 {
        chunks.push(remaining & 0x7f);
        remaining >>= 7;
    }
    chunks.reverse();
    let last = chunks.len() - 1;
    chunks
        .into_iter()
        .enumerate()
        .map(|(index, chunk)| (chunk as u8) | if index == last { 0x80 } else { 0 })
        .collect()
}

fn validate_tags(tags: &[TagDefinition]) -> Result<()> {
    let mut masks = 0u8;
    for (index, (tag, values_per_entry, mask)) in tags.iter().enumerate() {
        if tags[..index].iter().any(|(known, _, _)| known == tag) {
            return Err(Error::Output(format!("INDX tag {tag} is repeated")));
        }
        if *values_per_entry == 0 {
            return Err(Error::Output(
                "INDX tag must carry at least one value".to_owned(),
            ));
        }
        let normalized_mask = *mask >> mask.trailing_zeros();
        if *mask == 0 || mask.count_ones() > 2 || (normalized_mask & (normalized_mask + 1)) != 0 {
            return Err(Error::Output(
                "INDX tag masks must be contiguous one- or two-bit fields".to_owned(),
            ));
        }
        if masks & mask != 0 {
            return Err(Error::Output("INDX tag masks must be unique".to_owned()));
        }
        masks |= mask;
        if mask.count_ones() == 1 && *values_per_entry != 1 {
            // A one-bit field can describe one occurrence only.  Two-value
            // tags such as NCX tag 6 use a one-bit presence field, but still
            // carry both values in that single occurrence.
            continue;
        }
        if index >= 8 {
            return Err(Error::Output(
                "INDX control byte has too many tags".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_entries(entries: &[RawIndexEntry], tags: &[TagDefinition]) -> Result<()> {
    for entry in entries {
        for (tag, values) in &entry.tags {
            let Some((_, values_per_entry, mask)) = tags.iter().find(|(known, _, _)| known == tag)
            else {
                return Err(Error::Output(format!("INDX entry uses unknown tag {tag}")));
            };
            if values.is_empty() || values.len() % usize::from(*values_per_entry) != 0 {
                return Err(Error::Output(format!(
                    "INDX tag {tag} has {} values, not a multiple of {values_per_entry}",
                    values.len()
                )));
            }
            let occurrences = values.len() / usize::from(*values_per_entry);
            let capacity = usize::from(*mask >> mask.trailing_zeros());
            if mask.count_ones() == 1 && occurrences != 1 {
                return Err(Error::Output(format!(
                    "INDX one-bit tag {tag} must have one occurrence"
                )));
            }
            if mask.count_ones() > 1 && occurrences >= capacity {
                return Err(Error::Output(format!(
                    "INDX multi-bit tag {tag} occurrence count is reserved"
                )));
            }
        }
        for (tag, _, _) in tags {
            if entry
                .tags
                .iter()
                .filter(|(entry_tag, _)| entry_tag == tag)
                .count()
                > 1
            {
                return Err(Error::Output(format!("INDX entry repeats tag {tag}")));
            }
        }
    }
    Ok(())
}
