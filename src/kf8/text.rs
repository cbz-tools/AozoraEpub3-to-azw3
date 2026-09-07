#[derive(Debug)]
pub struct TextRecord {
    pub data: Vec<u8>,
    multibyte_overlap: Vec<u8>,
}

impl TextRecord {
    /// Split one continuous RawML/CSS stream at fixed decoded 4096-byte
    /// boundaries. The boundary is global across sections and flows; it is
    /// never reset per section because PalmDOC decoded text and RawML use one
    /// continuous global stream. FRAG tag-6 `start`/`length` are separate
    /// document-local fragment-payload-stream coordinates.
    /// Build fixed decoded records directly from the logical stream chunks.
    /// This avoids materializing a second concatenated RawML Vec just to split
    /// it back into the same 4096-byte records.
    pub fn split_chunks(chunks: &[&[u8]]) -> Vec<Self> {
        let total_length = chunks.iter().map(|chunk| chunk.len()).sum::<usize>();
        if total_length == 0 {
            return Vec::new();
        }
        let record_count = total_length.div_ceil(PALMDOC_RECORD_SIZE);
        let mut records = Vec::with_capacity(record_count);
        let mut current = Vec::with_capacity(PALMDOC_RECORD_SIZE);
        for chunk in chunks {
            let mut cursor = 0;
            while cursor < chunk.len() {
                let remaining = PALMDOC_RECORD_SIZE - current.len();
                let take = remaining.min(chunk.len() - cursor);
                current.extend_from_slice(&chunk[cursor..cursor + take]);
                cursor += take;
                if current.len() == PALMDOC_RECORD_SIZE {
                    records.push(Self {
                        data: current,
                        multibyte_overlap: Vec::new(),
                    });
                    current = Vec::with_capacity(PALMDOC_RECORD_SIZE);
                }
            }
        }
        if !current.is_empty() {
            records.push(Self {
                data: current,
                multibyte_overlap: Vec::new(),
            });
        }
        for index in 0..records.len().saturating_sub(1) {
            records[index].multibyte_overlap =
                utf8_overlap_between(&records[index].data, &records[index + 1].data);
        }
        records
    }

    /// Append the KF8 trailing-data entries after the compressed payload.
    /// Physical order is payload, UTF-8 overlap marker/data, then TBS and its
    /// backward size. MobiHeader extra_data_flags `0x3` selects both entries.
    pub fn into_trailing_data_with_compression(
        self,
        indexing_tbs: &[u8],
        compressor: Option<&mut PalmDocCompressor>,
    ) -> Vec<u8> {
        let TextRecord {
            data,
            multibyte_overlap,
        } = self;
        let mut encoded = if let Some(compressor) = compressor {
            let mut encoded = Vec::with_capacity(data.len());
            compressor.compress(&data, &mut encoded);
            encoded
        } else {
            data
        };
        debug_assert!(multibyte_overlap.len() <= 3);
        encoded.extend_from_slice(&multibyte_overlap);
        // The marker's low two bits encode the complete multibyte-entry size
        // minus one. The marker-only form is therefore zero; with one to
        // three overlap bytes the marker values are one to three.
        encoded.push(
            u8::try_from(multibyte_overlap.len()).expect("UTF-8 overlap exceeds marker width"),
        );
        encoded.extend_from_slice(indexing_tbs);
        let trailing_length = trailing_entry_size(indexing_tbs.len());
        encoded.extend_from_slice(&encode_backward_vwi(trailing_length));
        encoded
    }
}

const MATCH_TABLE_SIZE: usize = 1 << 16;
const MATCH_TABLE_MASK: usize = MATCH_TABLE_SIZE - 1;

#[derive(Debug, Clone, Copy)]
struct MatchSlot {
    hash: u32,
    position: u32,
    length: u8,
    generation: u32,
}

impl MatchSlot {
    const EMPTY: Self = Self {
        hash: 0,
        position: 0,
        length: 0,
        generation: 0,
    };
}

/// Allocation-free-after-initialization PalmDOC dictionary.
///
/// The original encoder allocated eight HashMaps and a temporary Vec key for
/// every visited position. PalmDOC records are only 4096 bytes, so a single
/// fixed open-addressed table is both smaller in the hot path and sufficient
/// for every possible distinct (length, 3..=10-byte) key in one record. The
/// stored position is still checked byte-for-byte after hashing, so collisions
/// cannot change the stream.
#[derive(Debug)]
pub struct PalmDocCompressor {
    table: Vec<MatchSlot>,
    generation: u32,
}

impl PalmDocCompressor {
    pub fn new() -> Self {
        Self {
            table: vec![MatchSlot::EMPTY; MATCH_TABLE_SIZE],
            generation: 0,
        }
    }

    fn begin_record(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        if self.generation == 0 {
            self.table.fill(MatchSlot::EMPTY);
            self.generation = 1;
        }
    }

    fn hash(source: &[u8], start: usize, length: usize) -> u32 {
        let mut hash = 2_166_136_261u32;
        for &byte in &source[start..start + length] {
            hash = (hash ^ u32::from(byte)).wrapping_mul(16_777_619);
        }
        hash
    }

    fn table_index(hash: u32) -> usize {
        (hash as usize).wrapping_mul(2_654_435_761) & MATCH_TABLE_MASK
    }

    fn find(&self, source: &[u8], start: usize, length: usize, hash: u32) -> Option<usize> {
        let mut index = Self::table_index(hash);
        loop {
            let slot = self.table[index];
            if slot.generation != self.generation {
                return None;
            }
            if slot.hash == hash
                && slot.length as usize == length
                && source[slot.position as usize..slot.position as usize + length]
                    == source[start..start + length]
            {
                return Some(slot.position as usize);
            }
            index = (index + 1) & MATCH_TABLE_MASK;
        }
    }

    fn insert(&mut self, source: &[u8], start: usize, length: usize, hash: u32) {
        let mut index = Self::table_index(hash);
        loop {
            let slot = self.table[index];
            if slot.generation != self.generation {
                self.table[index] = MatchSlot {
                    hash,
                    position: u32::try_from(start).expect("PalmDOC source fits in u32"),
                    length: u8::try_from(length).expect("PalmDOC match length fits in u8"),
                    generation: self.generation,
                };
                return;
            }
            if slot.hash == hash
                && slot.length as usize == length
                && source[slot.position as usize..slot.position as usize + length]
                    == source[start..start + length]
            {
                self.table[index].position =
                    u32::try_from(start).expect("PalmDOC source fits in u32");
                return;
            }
            index = (index + 1) & MATCH_TABLE_MASK;
        }
    }

    pub fn compress(&mut self, source: &[u8], encoded: &mut Vec<u8>) {
        self.begin_record();
        let mut cursor = 0usize;
        while cursor < source.len() {
            for length in 3..=10 {
                if cursor >= length {
                    let position = cursor - length;
                    let hash = Self::hash(source, position, length);
                    self.insert(source, position, length, hash);
                }
            }
            if cursor > 10 && source.len() - cursor > 10 {
                let mut match_position = None;
                let mut match_length = 0usize;
                for length in (3..=10).rev() {
                    let hash = Self::hash(source, cursor, length);
                    if let Some(position) = self.find(source, cursor, length, hash) {
                        if cursor - position <= 2047 {
                            match_position = Some(position);
                            match_length = length;
                            break;
                        }
                    }
                }
                if let Some(position) = match_position {
                    let distance = cursor - position;
                    let code =
                        0x8000u16 | (((distance as u16) << 3) & 0x3ff8) | (match_length as u16 - 3);
                    encoded.extend_from_slice(&code.to_be_bytes());
                    cursor += match_length;
                    continue;
                }
            }

            let byte = source[cursor];
            cursor += 1;
            if byte == b' ' && cursor < source.len() && matches!(source[cursor], 0x40..=0x7f) {
                encoded.push(source[cursor] ^ 0x80);
                cursor += 1;
                continue;
            }

            if byte == 0 || (byte > 8 && byte < 0x80) {
                encoded.push(byte);
                continue;
            }

            let mut literal = [0u8; 8];
            let mut literal_length = 1usize;
            literal[0] = byte;
            while cursor < source.len() && literal_length < literal.len() {
                let next = source[cursor];
                if next == 0 || (next > 8 && next < 0x80) {
                    break;
                }
                literal[literal_length] = next;
                literal_length += 1;
                cursor += 1;
            }
            encoded.push(literal_length as u8);
            encoded.extend_from_slice(&literal[..literal_length]);
        }
    }
}

impl Default for PalmDocCompressor {
    fn default() -> Self {
        Self::new()
    }
}

const PALMDOC_RECORD_SIZE: usize = 4096;

fn utf8_overlap_between(previous: &[u8], next: &[u8]) -> Vec<u8> {
    let previous_start = previous.len().saturating_sub(3);
    let mut combined = [0u8; 6];
    let previous_tail = &previous[previous_start..];
    let next_head = &next[..next.len().min(3)];
    combined[..previous_tail.len()].copy_from_slice(previous_tail);
    combined[previous_tail.len()..previous_tail.len() + next_head.len()].copy_from_slice(next_head);
    for start in 0..previous_tail.len() {
        let Some(length) = utf8_leading_byte_length(combined[start]) else {
            continue;
        };
        let end = start + length;
        let boundary = previous_tail.len();
        if start < boundary
            && boundary < end
            && end <= boundary + next_head.len()
            && std::str::from_utf8(&combined[start..end]).is_ok()
        {
            return combined[boundary..end].to_vec();
        }
    }
    Vec::new()
}

fn utf8_leading_byte_length(byte: u8) -> Option<usize> {
    match byte {
        0x00..=0x7f => Some(1),
        0xc0..=0xdf => Some(2),
        0xe0..=0xef => Some(3),
        0xf0..=0xf7 => Some(4),
        _ => None,
    }
}

fn trailing_entry_size(tbs_length: usize) -> usize {
    let mut size = tbs_length
        .checked_add(1)
        .expect("TBS trailing entry size overflow");
    loop {
        let next = tbs_length
            .checked_add(backward_vwi_length(size))
            .expect("TBS trailing entry size overflow");
        if next == size {
            return size;
        }
        size = next;
    }
}

fn backward_vwi_length(value: usize) -> usize {
    let mut length = 1;
    let mut remaining = value >> 7;
    while remaining != 0 {
        length += 1;
        remaining >>= 7;
    }
    length
}

fn encode_backward_vwi(mut value: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(backward_vwi_length(value));
    loop {
        bytes.push((value & 0x7f) as u8);
        value >>= 7;
        if value == 0 {
            break;
        }
    }
    if let Some(last) = bytes.last_mut() {
        *last |= 0x80;
    }
    bytes.reverse();
    bytes
}
