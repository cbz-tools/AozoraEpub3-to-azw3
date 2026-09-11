#![allow(dead_code)]

use std::io::{Cursor, Write};

use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

mod conversion;
mod navigation;
#[allow(unused_imports)]
pub use conversion::*;
#[allow(unused_imports)]
pub use navigation::*;

#[derive(Debug, Clone)]
pub struct Azw3 {
    pub bytes: Vec<u8>,
    pub offsets: Vec<usize>,
}

impl Azw3 {
    pub fn parse(bytes: Vec<u8>) -> Self {
        assert!(bytes.len() >= 78, "AZW3 is shorter than a PalmDB header");
        let count = u16(&bytes, 76) as usize;
        assert!(count > 0, "AZW3 has no PalmDB records");
        let table_end = 78 + count * 8 + 2;
        assert!(table_end <= bytes.len(), "PalmDB record table is truncated");
        let offsets = (0..count)
            .map(|i| u32(&bytes, 78 + i * 8) as usize)
            .collect::<Vec<_>>();
        assert!(
            offsets[0] >= table_end,
            "first record overlaps PalmDB table"
        );
        assert!(
            offsets.windows(2).all(|pair| pair[0] <= pair[1]),
            "record offsets are not ordered"
        );
        assert!(
            offsets.iter().all(|offset| *offset <= bytes.len()),
            "record offset is outside file"
        );
        Self { bytes, offsets }
    }

    pub fn record(&self, index: usize) -> &[u8] {
        let end = if index + 1 < self.offsets.len() {
            self.offsets[index + 1]
        } else {
            self.bytes.len()
        };
        &self.bytes[self.offsets[index]..end]
    }

    pub fn text_payload(&self, index: usize) -> &[u8] {
        text_record_parts(self.record(index)).0
    }

    pub fn text_overlap(&self, index: usize) -> (&[u8], u8) {
        let record = self.record(index);
        let (tbs_start, _) = text_trailing_bounds(record);
        let marker = record[tbs_start - 1];
        let overlap_len = usize::from(marker & 3);
        let overlap_start = tbs_start - 1 - overlap_len;
        (&record[overlap_start..tbs_start - 1], marker)
    }

    pub fn record_count(&self) -> usize {
        self.offsets.len()
    }

    pub fn record_with_magic(&self, magic: &[u8]) -> Option<usize> {
        (0..self.record_count()).find(|index| self.record(*index).starts_with(magic))
    }

    pub fn record_zero(&self) -> &[u8] {
        self.record(0)
    }

    pub fn palm_doc(&self) -> PalmDoc {
        let record = self.record_zero();
        assert!(record.len() >= 16, "record zero has no PalmDOC header");
        PalmDoc {
            compression: u16(record, 0),
            text_length: u32(record, 4) as usize,
            text_records: u16(record, 8) as usize,
            record_size: u16(record, 10) as usize,
        }
    }

    pub fn mobi(&self) -> Mobi {
        let record = self.record_zero();
        assert_eq!(&record[16..20], b"MOBI", "record zero has no MOBI header");
        Mobi {
            first_non_text: u32(record, 16 + 0x40) as usize,
            first_image: u32(record, 16 + 0x5c) as usize,
            fdst: u32(record, 16 + 0xb0) as usize,
            fdst_flows: u32(record, 16 + 0xb4) as usize,
            fcis: u32(record, 16 + 0xb8) as usize,
            flis: u32(record, 16 + 0xc0) as usize,
            ncx: u32(record, 16 + 0xe4) as usize,
            indx: u32(record, 16 + 0xe8) as usize,
            skel: u32(record, 16 + 0xec) as usize,
            guide: u32(record, 16 + 0xf4) as usize,
            last_image: u16(record, 16 + 0xaa) as usize,
            title_offset: u32(record, 16 + 0x44) as usize,
            title_length: u32(record, 16 + 0x48) as usize,
        }
    }

    pub fn exth(&self) -> Exth {
        let record = self.record_zero();
        let start = 16 + 264;
        assert_eq!(
            &record[start..start + 4],
            b"EXTH",
            "record zero has no EXTH header"
        );
        let count = u32(record, start + 8) as usize;
        let mut cursor = start + 12;
        let mut values = Vec::with_capacity(count);
        for _ in 0..count {
            assert!(
                cursor + 8 <= record.len(),
                "EXTH record header is truncated"
            );
            let kind = u32(record, cursor);
            let length = u32(record, cursor + 4) as usize;
            assert!(
                length >= 8 && cursor + length <= record.len(),
                "EXTH record has invalid length"
            );
            values.push((kind, record[cursor + 8..cursor + length].to_vec()));
            cursor += length;
        }
        Exth { values }
    }

    pub fn rawml(&self) -> Vec<u8> {
        let pd = self.palm_doc();
        let mut rawml = Vec::with_capacity(pd.text_length);
        for index in 1..=pd.text_records {
            let (record, _) = text_record_parts(self.record(index));
            match pd.compression {
                1 => rawml.extend_from_slice(record),
                2 => palm_doc_decompress(record, &mut rawml),
                compression => panic!("unsupported PalmDOC compression {compression}"),
            }
        }
        rawml.truncate(pd.text_length);
        rawml
    }

    /// Reconstruct each XHTML skeleton by applying its document-local FRAG
    /// payloads at the serialized SKEL insertion positions. The PalmDOC text
    /// stream is SKEL followed by FRAG payloads, so `rawml()` alone is a
    /// serialization stream rather than one parseable document tree.
    pub fn reconstructed_xhtml_sections(&self) -> Vec<Vec<u8>> {
        let rawml = self.rawml();
        let mobi = self.mobi();
        let skeleton_rows = self.index_rows(mobi.skel);
        let fragment_rows = self.index_rows(mobi.indx);
        let skeletons = skeleton_rows
            .iter()
            .map(|row| decode_skeleton_row(row))
            .collect::<Vec<_>>();
        let mut fragments = fragment_rows
            .iter()
            .map(|row| decode_fragment_row(row))
            .collect::<Vec<_>>();
        fragments.sort_by_key(|fragment| (fragment.file_number, fragment.sequence));

        skeletons
            .iter()
            .enumerate()
            .map(|(file_number, skeleton)| {
                let skeleton_end = skeleton
                    .start
                    .checked_add(skeleton.length)
                    .expect("SKEL reconstruction range overflow");
                let mut section = rawml
                    .get(skeleton.start..skeleton_end)
                    .expect("SKEL reconstruction range is inside RawML")
                    .to_vec();
                let payload_base = skeleton_end;
                let file_fragments = fragments
                    .iter()
                    .filter(|fragment| fragment.file_number == file_number as u32)
                    .collect::<Vec<_>>();
                assert_eq!(
                    file_fragments.len(),
                    skeleton.fragment_count as usize,
                    "SKEL/FRAG fragment counts disagree"
                );
                for fragment in file_fragments {
                    let payload_end = fragment
                        .start
                        .checked_add(fragment.length)
                        .expect("FRAG payload range overflow");
                    let payload = rawml
                        .get(
                            payload_base + fragment.start as usize
                                ..payload_base + payload_end as usize,
                        )
                        .expect("FRAG payload range is inside RawML");
                    let insertion = fragment
                        .insert_position
                        .checked_sub(skeleton.start as u32)
                        .expect("FRAG insertion precedes its SKEL");
                    let insertion = insertion as usize;
                    assert!(
                        insertion <= section.len(),
                        "FRAG insertion is outside reconstructed SKEL"
                    );
                    section.splice(insertion..insertion, payload.iter().copied());
                }
                section
            })
            .collect()
    }

    pub fn resolve_position_href(&self, href: &str) -> (usize, usize) {
        let payload = href
            .strip_prefix("kindle:pos:fid:")
            .expect("position href has a Kindle fid prefix");
        let (sequence, offset) = payload
            .split_once(":off:")
            .expect("position href has a Kindle offset");
        let sequence = decode_kindle_base32(sequence);
        let offset = decode_kindle_base32(offset);
        let fragment = self
            .index_rows(self.mobi().indx)
            .into_iter()
            .map(|row| decode_fragment_row(&row))
            .find(|fragment| fragment.sequence == sequence)
            .expect("position href resolves to a FRAG entry");
        assert!(
            offset <= fragment.length,
            "position href offset exceeds its FRAG payload"
        );
        (fragment.file_number as usize, offset as usize)
    }

    fn index_rows(&self, pointer: usize) -> Vec<Vec<u8>> {
        let main = self.record(pointer);
        let detail_count = u32(main, 0x18) as usize;
        let mut rows = Vec::new();
        for detail_index in 0..detail_count {
            let detail = self.record(pointer + 1 + detail_index);
            let idxt = u32(detail, 0x14) as usize;
            let count = u32(detail, 0x18) as usize;
            for row_index in 0..count {
                let row = u16(detail, idxt + 4 + row_index * 2) as usize;
                let next_row = if row_index + 1 < count {
                    u16(detail, idxt + 4 + (row_index + 1) * 2) as usize
                } else {
                    idxt
                };
                rows.push(detail.get(row..next_row).expect("INDX row range").to_vec());
            }
        }
        rows
    }

    pub fn assert_text_trailing_data(&self) -> usize {
        let (_, total_tbs_bytes) = self.text_trailing_data_stats();
        assert!(
            total_tbs_bytes > 0,
            "text records contain no TBS/PositionMap data"
        );
        total_tbs_bytes
    }

    pub fn text_trailing_data_stats(&self) -> (usize, usize) {
        let pd = self.palm_doc();
        let mut tbs_records = 0;
        let mut total_tbs_bytes = 0;
        for index in 1..=pd.text_records {
            let (_, tbs) = text_record_parts(self.record(index));
            tbs_records += usize::from(!tbs.is_empty());
            total_tbs_bytes += tbs.len();
        }
        (tbs_records, total_tbs_bytes)
    }

    pub fn image_records(&self) -> Vec<(usize, Vec<u8>)> {
        let mobi = self.mobi();
        if mobi.first_image == u32::MAX as usize || mobi.last_image == u16::MAX as usize {
            return Vec::new();
        }
        (mobi.first_image..=mobi.last_image)
            .map(|i| (i, self.record(i).to_vec()))
            .collect()
    }

    pub fn index_report(&self, pointer: usize) -> IndexReport {
        let main = self.record(pointer);
        assert_eq!(
            &main[..4],
            b"INDX",
            "MOBI index pointer does not point to INDX"
        );
        let detail_count = u32(main, 0x18) as usize;
        let entry_count = u32(main, 0x24) as usize;
        assert!(
            pointer
                .checked_add(1 + detail_count)
                .is_some_and(|end| end <= self.record_count()),
            "INDX detail records exceed the PalmDB record table"
        );
        let main_idxt = u32(main, 0x14) as usize;
        assert!(
            main_idxt >= 0xc0 && main_idxt + 4 + detail_count * 2 <= main.len(),
            "INDX main routing IDXT is out of bounds"
        );
        let mut detail_entries = Vec::with_capacity(detail_count);
        for index in 0..detail_count {
            let detail = self.record(pointer + 1 + index);
            assert_eq!(
                &detail[..4],
                b"INDX",
                "INDX detail record is not contiguous"
            );
            let idxt = u32(detail, 0x14) as usize;
            let count = u32(detail, 0x18) as usize;
            assert!(
                idxt >= 0xc0 && idxt + 4 + count * 2 <= detail.len(),
                "INDX IDXT is out of bounds"
            );
            for row_index in 0..count {
                let row = u16(detail, idxt + 4 + row_index * 2) as usize;
                assert!(row < idxt, "INDX row points beyond IDXT");
                let length = *detail.get(row).expect("INDX row length");
                assert!(row + 1 + length as usize <= idxt, "INDX row crosses IDXT");
            }
            detail_entries.push(count);
        }
        for (index, expected_count) in detail_entries.iter().copied().enumerate() {
            let row = u16(main, main_idxt + 4 + index * 2) as usize;
            assert!(row < main_idxt, "INDX main routing row points beyond IDXT");
            let label_len = *main.get(row).expect("INDX main routing row length") as usize;
            let count_offset = row
                .checked_add(1 + label_len)
                .expect("INDX main routing row length overflow");
            assert!(
                count_offset + 2 <= main_idxt,
                "INDX main routing row crosses IDXT"
            );
            assert_eq!(
                u16(main, count_offset) as usize,
                expected_count,
                "INDX main routing count does not match detail record"
            );
        }
        IndexReport {
            entry_count,
            detail_count,
            detail_entries,
        }
    }

    pub fn guide_targets(&self) -> Vec<(String, u32, u32)> {
        let pointer = self.mobi().guide;
        let main = self.record(pointer);
        let detail_count = u32(main, 0x18) as usize;
        let mut targets = Vec::new();
        for detail_index in 0..detail_count {
            let detail = self.record(pointer + 1 + detail_index);
            let idxt = u32(detail, 0x14) as usize;
            let count = u32(detail, 0x18) as usize;
            for row_index in 0..count {
                let row = u16(detail, idxt + 4 + row_index * 2) as usize;
                let kind_length = *detail.get(row).expect("Guide row kind length") as usize;
                let kind_start = row + 1;
                let kind_end = kind_start + kind_length;
                let kind = String::from_utf8(detail[kind_start..kind_end].to_vec())
                    .expect("Guide row kind is UTF-8");
                let mut cursor = kind_end;
                let control = *detail.get(cursor).expect("Guide row control byte");
                cursor += 1;
                assert_eq!(control & 0x01, 0x01, "Guide row has no CTOC tag");
                let _ctoc_offset = read_vwi(detail, &mut cursor);
                assert_eq!(control & 0x02, 0x02, "Guide row has no position tag");
                let sequence = read_vwi(detail, &mut cursor);
                let offset = read_vwi(detail, &mut cursor);
                targets.push((kind, sequence, offset));
            }
        }
        targets
    }

    pub fn ncx_depths(&self) -> Vec<usize> {
        let ncx = self.mobi().ncx;
        let main = self.record(ncx);
        let detail_count = u32(main, 0x18) as usize;
        let mut depths = Vec::new();
        for detail_index in 0..detail_count {
            let detail = self.record(ncx + 1 + detail_index);
            let idxt = u32(detail, 0x14) as usize;
            let count = u32(detail, 0x18) as usize;
            for row_index in 0..count {
                let row = u16(detail, idxt + 4 + row_index * 2) as usize;
                let text_len = *detail.get(row).expect("NCX row text length") as usize;
                let mut cursor = row + 1 + text_len;
                let control = *detail.get(cursor).expect("NCX row control byte");
                cursor += 1;
                let mut depth = None;
                for (tag, mask, values_per_entry) in [
                    (1, 0x01, 1),
                    (2, 0x02, 1),
                    (3, 0x04, 1),
                    (4, 0x08, 1),
                    (21, 0x10, 1),
                    (22, 0x20, 1),
                    (23, 0x40, 1),
                    (6, 0x80, 2),
                ] {
                    if control & mask != 0 {
                        for value_index in 0..values_per_entry {
                            let value = read_vwi(detail, &mut cursor);
                            if tag == 4 && value_index == 0 {
                                depth = Some(value as usize);
                            }
                        }
                    }
                }
                depths.push(depth.expect("NCX row has no depth tag"));
            }
        }
        depths
    }

    pub fn fdst_ranges(&self) -> Vec<(usize, usize)> {
        let record = self.record(self.mobi().fdst);
        assert_eq!(&record[..4], b"FDST");
        let count = u32(record, 8) as usize;
        assert_eq!(
            record.len(),
            12 + count * 8,
            "FDST has invalid range geometry"
        );
        (0..count)
            .map(|i| {
                (
                    u32(record, 12 + i * 8) as usize,
                    u32(record, 16 + i * 8) as usize,
                )
            })
            .collect()
    }

    pub fn assert_control_records(&self) {
        let mobi = self.mobi();
        assert_eq!(&self.record(mobi.fcis)[..4], b"FCIS");
        assert_eq!(&self.record(mobi.flis)[..4], b"FLIS");
        assert_eq!(&self.record(mobi.fdst)[..4], b"FDST");
        assert_eq!(&self.record(mobi.skel)[..4], b"INDX");
        assert_eq!(&self.record(mobi.ncx)[..4], b"INDX");
        assert_eq!(&self.record(mobi.guide)[..4], b"INDX");
        assert_eq!(&self.record(mobi.indx)[..4], b"INDX");
        for pointer in [mobi.indx, mobi.skel, mobi.ncx, mobi.guide] {
            let report = self.index_report(pointer);
            assert!(report.detail_count > 0, "INDX has no detail records");
            assert_eq!(report.entry_count, report.detail_entries.iter().sum());
        }
    }
}

fn text_record_parts(record: &[u8]) -> (&[u8], &[u8]) {
    // The writer appends UTF-8 overlap, a marker, TBS, and a backwards VWI
    // size to every text record. Decode the size from the end so compressed
    // payload bytes are never mistaken for PalmDOC opcodes.
    assert!(!record.is_empty(), "empty PalmDOC text record");
    let (tbs_start, vwi_start) = text_trailing_bounds(record);
    let overlap_len = usize::from(record[tbs_start - 1] & 3);
    let payload_end = tbs_start - 1 - overlap_len;
    assert!(payload_end <= tbs_start, "UTF-8 overlap crosses TBS marker");
    (&record[..payload_end], &record[tbs_start..vwi_start])
}

fn text_trailing_bounds(record: &[u8]) -> (usize, usize) {
    assert!(!record.is_empty(), "empty PalmDOC text record");
    let mut value = 0usize;
    let mut shift = 0;
    let mut cursor = record.len();
    loop {
        cursor -= 1;
        let byte = record[cursor];
        value |= usize::from(byte & 0x7f) << shift;
        if byte & 0x80 != 0 {
            break;
        }
        shift += 7;
        assert!(shift < usize::BITS as usize, "TBS size VWI is too long");
        assert!(cursor > 0, "TBS size VWI is unterminated");
    }
    assert!(value <= record.len(), "TBS size is outside record");
    let tbs_start = record.len() - value;
    assert!(tbs_start > 0, "TBS entry has no marker");
    (tbs_start, cursor)
}

#[derive(Debug, Clone, Copy)]
pub struct PalmDoc {
    pub compression: u16,
    pub text_length: usize,
    pub text_records: usize,
    pub record_size: usize,
}
#[derive(Debug, Clone, Copy)]
pub struct Mobi {
    pub first_non_text: usize,
    pub first_image: usize,
    pub fdst: usize,
    pub fdst_flows: usize,
    pub fcis: usize,
    pub flis: usize,
    pub ncx: usize,
    pub indx: usize,
    pub skel: usize,
    pub guide: usize,
    pub last_image: usize,
    pub title_offset: usize,
    pub title_length: usize,
}
#[derive(Debug, Clone)]
pub struct Exth {
    pub values: Vec<(u32, Vec<u8>)>,
}
impl Exth {
    pub fn get(&self, kind: u32) -> Option<&[u8]> {
        self.values
            .iter()
            .find(|(candidate, _)| *candidate == kind)
            .map(|(_, value)| value.as_slice())
    }
    pub fn u32(&self, kind: u32) -> Option<u32> {
        self.get(kind)
            .and_then(|value| value.try_into().ok())
            .map(u32::from_be_bytes)
    }
    pub fn text(&self, kind: u32) -> Option<String> {
        self.get(kind)
            .map(|value| String::from_utf8_lossy(value).into_owned())
    }
}
#[derive(Debug, Clone)]
pub struct IndexReport {
    pub entry_count: usize,
    pub detail_count: usize,
    pub detail_entries: Vec<usize>,
}

#[derive(Debug, Clone, Copy)]
struct DecodedSkeleton {
    fragment_count: u32,
    start: usize,
    length: usize,
}

#[derive(Debug, Clone, Copy)]
struct DecodedFragment {
    insert_position: u32,
    file_number: u32,
    sequence: u32,
    start: u32,
    length: u32,
}

fn decode_skeleton_row(row: &[u8]) -> DecodedSkeleton {
    let (text, mut cursor) = index_row_header(row);
    let key = String::from_utf8_lossy(text);
    assert!(key.starts_with("SKEL"), "SKEL row key is invalid");
    let control = read_index_control(row, &mut cursor);
    let tag_1_occurrences = usize::from(control & 0x03);
    let tag_1 = (0..tag_1_occurrences)
        .map(|_| read_vwi(row, &mut cursor))
        .collect::<Vec<_>>();
    let tag_6_occurrences = usize::from((control & 0x0c) >> 2);
    let tag_6 = (0..tag_6_occurrences * 2)
        .map(|_| read_vwi(row, &mut cursor))
        .collect::<Vec<_>>();
    assert!(
        !tag_1.is_empty() && tag_6.len() >= 2,
        "SKEL row tags are incomplete"
    );
    DecodedSkeleton {
        fragment_count: tag_1[0],
        start: tag_6[0] as usize,
        length: tag_6[1] as usize,
    }
}

fn decode_fragment_row(row: &[u8]) -> DecodedFragment {
    let (text, mut cursor) = index_row_header(row);
    let insert_position = String::from_utf8_lossy(text)
        .parse::<u32>()
        .expect("FRAG insertion key is numeric");
    let control = read_index_control(row, &mut cursor);
    assert_eq!(control & 0x0f, 0x0f, "FRAG row tags are incomplete");
    let _ctoc = read_vwi(row, &mut cursor);
    let file_number = read_vwi(row, &mut cursor);
    let sequence = read_vwi(row, &mut cursor);
    let start = read_vwi(row, &mut cursor);
    let length = read_vwi(row, &mut cursor);
    DecodedFragment {
        insert_position,
        file_number,
        sequence,
        start,
        length,
    }
}

fn decode_kindle_base32(value: &str) -> u32 {
    value.bytes().fold(0u32, |decoded, byte| {
        let digit = match byte {
            b'0'..=b'9' => u32::from(byte - b'0'),
            b'A'..=b'V' => u32::from(byte - b'A') + 10,
            _ => panic!("invalid Kindle base32 digit"),
        };
        decoded
            .checked_mul(32)
            .and_then(|value| value.checked_add(digit))
            .expect("Kindle base32 value overflow")
    })
}

fn index_row_header(row: &[u8]) -> (&[u8], usize) {
    let text_length = *row.first().expect("INDX row has a text length") as usize;
    let text_end = 1 + text_length;
    (row.get(1..text_end).expect("INDX row text"), text_end)
}

fn read_index_control(row: &[u8], cursor: &mut usize) -> u8 {
    let control = *row.get(*cursor).expect("INDX row control byte");
    *cursor += 1;
    control
}

fn palm_doc_decompress(input: &[u8], output: &mut Vec<u8>) {
    let mut cursor = 0;
    while cursor < input.len() {
        let byte = input[cursor];
        cursor += 1;
        match byte {
            0x00..=0x08 => {
                let count = byte as usize;
                assert!(cursor + count <= input.len());
                output.extend_from_slice(&input[cursor..cursor + count]);
                cursor += count;
            }
            0x09..=0x7f => output.push(byte),
            0x80..=0xbf => {
                assert!(cursor < input.len(), "PalmDOC back-reference is truncated");
                let next = input[cursor];
                cursor += 1;
                let distance = ((byte as usize & 0x3f) << 5) | (next as usize >> 3);
                let length = (next & 0x07) as usize + 3;
                assert!(
                    distance > 0 && distance <= output.len(),
                    "PalmDOC back-reference is invalid"
                );
                let start = output.len() - distance;
                for index in 0..length {
                    let value = output[start + index];
                    output.push(value);
                }
            }
            0xc0..=0xff => {
                output.push(b' ');
                output.push(byte ^ 0x80);
            }
        }
    }
}

fn u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes(bytes[offset..offset + 2].try_into().expect("u16 bounds"))
}
fn u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(bytes[offset..offset + 4].try_into().expect("u32 bounds"))
}

fn read_vwi(bytes: &[u8], cursor: &mut usize) -> u32 {
    let mut value = 0;
    loop {
        let byte = *bytes.get(*cursor).expect("VWI is truncated");
        *cursor += 1;
        value = (value << 7) | u32::from(byte & 0x7f);
        if byte & 0x80 != 0 {
            return value;
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CoverRecipe {
    pub cover_page: bool,
    pub cover_navigation: bool,
    pub cover_first: bool,
    pub ordinary_svg: bool,
    pub padding_images: usize,
}

pub fn cover_recipe(recipe: CoverRecipe) -> Vec<u8> {
    const PADDING_IMAGE_HREFS: [&str; 16] = [
        "images/padding-00.png",
        "images/padding-01.png",
        "images/padding-02.png",
        "images/padding-03.png",
        "images/padding-04.png",
        "images/padding-05.png",
        "images/padding-06.png",
        "images/padding-07.png",
        "images/padding-08.png",
        "images/padding-09.png",
        "images/padding-10.png",
        "images/padding-11.png",
        "images/padding-12.png",
        "images/padding-13.png",
        "images/padding-14.png",
        "images/padding-15.png",
    ];
    assert!(
        recipe.padding_images <= PADDING_IMAGE_HREFS.len(),
        "cover recipe padding image count exceeds checked-in test resources"
    );
    let padding_manifest = PADDING_IMAGE_HREFS
        .iter()
        .take(recipe.padding_images)
        .enumerate()
        .map(|(index, href)| {
            format!("<item id=\"padding-{index:02}\" href=\"{href}\" media-type=\"image/png\"/>")
        })
        .collect::<String>();
    let cover_page_manifest = if recipe.cover_page {
        "<item id=\"cover-page\" href=\"cover.xhtml\" media-type=\"application/xhtml+xml\" properties=\"svg\"/>"
    } else {
        ""
    };
    let cover_spine = if recipe.cover_page {
        "<itemref idref=\"cover-page\" linear=\"no\"/>"
    } else {
        ""
    };
    let svg_manifest = if recipe.ordinary_svg {
        "<item id=\"svg-child\" href=\"svg-child.xhtml\" media-type=\"application/xhtml+xml\" properties=\"svg\"/>"
    } else {
        ""
    };
    let svg_spine = if recipe.ordinary_svg {
        "<itemref idref=\"svg-child\" linear=\"yes\"/>"
    } else {
        ""
    };
    let cover_manifest = if recipe.cover_first {
        format!(
            "<item id=\"cover\" href=\"images/cover.png\" media-type=\"image/png\" properties=\"cover-image\"/><item id=\"child\" href=\"images/child.png\" media-type=\"image/png\"/>{padding_manifest}"
        )
    } else {
        format!(
            "<item id=\"child\" href=\"images/child.png\" media-type=\"image/png\"/>{padding_manifest}<item id=\"cover\" href=\"images/cover.png\" media-type=\"image/png\" properties=\"cover-image\"/>"
        )
    };
    let cover_nav = if recipe.cover_navigation {
        "<li><a href=\"cover.xhtml\">Cover</a><ol><li><a href=\"body.xhtml\">Body child</a></li></ol></li>"
    } else {
        "<li><a href=\"body.xhtml\">Body</a></li>"
    };
    let cover_landmark = if recipe.cover_page {
        "<li><a epub:type=\"cover\" href=\"cover.xhtml\">Cover</a></li>"
    } else {
        ""
    };
    let package = format!(
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Recipe Cover</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language><meta name="cover" content="cover"/></metadata><manifest><item id="style" href="style.css" media-type="text/css"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>{cover_page_manifest}{svg_manifest}{cover_manifest}<item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/></manifest><spine>{cover_spine}{svg_spine}<itemref idref="body" linear="yes"/><itemref idref="nav" linear="yes"/></spine></package>"#
    );
    let nav = format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol>{cover_nav}</ol></nav><nav epub:type="landmarks"><ol>{cover_landmark}<li><a epub:type="bodymatter" href="body.xhtml">Body</a></li></ol></nav></body></html>"#
    );
    let cover = br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><svg xmlns="http://www.w3.org/2000/svg"><image href="images/cover.png"/></svg><p>COVER_PAGE_SENTINEL</p></body></html>"#;
    let svg_child = br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><image xlink:href="images/child.png"/></svg><p>SVG_NON_COVER_SENTINEL</p></body></html>"#;
    let body = br#"<html class="hltr" xmlns="http://www.w3.org/1999/xhtml"><head><link rel="stylesheet" href="style.css"/></head><body><h1>BODY_PRESERVED</h1><img src="images/child.png"/></body></html>"#;
    let mut files = vec![
        (
            "style.css",
            b"body { writing-mode: horizontal-tb; }".to_vec(),
        ),
        ("body.xhtml", body.to_vec()),
        ("nav.xhtml", nav.into_bytes()),
        ("images/cover.png", png(660, 940, [40, 80, 140, 255])),
        ("images/child.png", png(120, 80, [180, 60, 40, 255])),
    ];
    for (index, href) in PADDING_IMAGE_HREFS
        .iter()
        .take(recipe.padding_images)
        .enumerate()
    {
        files.push((*href, png(24 + index as u32, 24, [20, 120, 80, 255])));
    }
    let extra = if recipe.cover_page {
        Some(("cover.xhtml", cover.to_vec()))
    } else if recipe.ordinary_svg {
        Some(("svg-child.xhtml", svg_child.to_vec()))
    } else {
        None
    };
    zip_epub(&package, &files, extra)
}

pub fn cover_bodymatter_landmark_recipe(missing_landmark_target: bool) -> Vec<u8> {
    let landmark_target = if missing_landmark_target {
        "missing.xhtml"
    } else {
        "cover.xhtml"
    };
    let package = r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/ops" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Cover Landmark Recipe</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language><meta name="cover" content="cover-image"/></metadata><manifest><item id="cover-image" href="images/cover.png" media-type="image/png" properties="cover-image"/><item id="cover" href="cover.xhtml" media-type="application/xhtml+xml"/><item id="page1" href="page1.xhtml" media-type="application/xhtml+xml"/><item id="page2" href="page2.xhtml" media-type="application/xhtml+xml"/><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/></manifest><spine><itemref idref="cover" linear="no"/><itemref idref="page1" linear="yes"/><itemref idref="page2" linear="yes"/></spine></package>"#;
    let nav = format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="page1.xhtml">Page one</a></li><li><a href="page2.xhtml">Page two</a></li></ol></nav><nav epub:type="landmarks"><ol><li><a epub:type="bodymatter" href="{landmark_target}">Bodymatter</a></li><li><a epub:type="text" href="{landmark_target}">Text</a></li><li><a epub:type="body" href="{landmark_target}">Body</a></li><li><a epub:type="start" href="{landmark_target}">Start</a></li></ol></nav></body></html>"#
    );
    let cover = br#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body epub:type="cover"><p>COVER_XHTML_SENTINEL</p><img src="images/cover.png"/></body></html>"#;
    let page1 =
        br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>PAGE1_SENTINEL</p></body></html>"#;
    let page2 =
        br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>PAGE2_SENTINEL</p></body></html>"#;
    let files = vec![
        ("nav.xhtml", nav.into_bytes()),
        ("page1.xhtml", page1.to_vec()),
        ("page2.xhtml", page2.to_vec()),
        ("images/cover.png", png(660, 940, [40, 80, 140, 255])),
    ];
    zip_epub(package, &files, Some(("cover.xhtml", cover.to_vec())))
}

#[derive(Debug, Clone, Copy)]
pub enum FixedLayoutDeclaration {
    FixedLayoutTrue,
    RenditionPrePaginated,
}

pub fn fixed_layout_comic_recipe(declaration: FixedLayoutDeclaration) -> Vec<u8> {
    let fixed_layout_metadata = match declaration {
        FixedLayoutDeclaration::FixedLayoutTrue => r#"<meta name="fixed-layout">true</meta>"#,
        FixedLayoutDeclaration::RenditionPrePaginated => {
            r#"<meta property="rendition:layout">pre-paginated</meta>"#
        }
    };
    let package = format!(
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Fixed Layout Comic</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language><meta name="cover" content="image1"/>{fixed_layout_metadata}<meta name="book-type" content="comic"/><meta name="orientation-lock" content="none"/><meta name="original-resolution" content="1125x1600"/><meta property="primary-writing-mode" content="horizontal-rl"/></metadata><manifest><item id="image1" href="image1.jpg" media-type="image/jpeg" properties="cover-image"/><item id="image2" href="image2.jpg" media-type="image/jpeg"/><item id="cover" href="cover.xhtml" media-type="application/xhtml+xml"/><item id="page1" href="page1.xhtml" media-type="application/xhtml+xml"/><item id="page2" href="page2.xhtml" media-type="application/xhtml+xml"/><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/></manifest><spine page-progression-direction="rtl"><itemref idref="cover" linear="yes"/><itemref idref="page1" linear="yes"/><itemref idref="page2" linear="yes"/></spine></package>"#
    );
    let nav = br#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="page1.xhtml">Page one</a></li><li><a href="page2.xhtml">Page two</a></li></ol></nav><nav epub:type="landmarks"><ol><li><a epub:type="bodymatter" href="cover.xhtml">Bodymatter</a></li></ol></nav></body></html>"#;
    let cover = br#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body epub:type="cover"><p>COVER_FIXED_LAYOUT_SENTINEL</p><img src="image1.jpg"/></body></html>"#;
    let page1 = br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>FIXED_PAGE1_SENTINEL</p><img src="image1.jpg"/></body></html>"#;
    let page2 = br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>FIXED_PAGE2_SENTINEL</p><img src="image2.jpg"/></body></html>"#;
    let files = vec![
        ("cover.xhtml", cover.to_vec()),
        ("page1.xhtml", page1.to_vec()),
        ("page2.xhtml", page2.to_vec()),
        ("nav.xhtml", nav.to_vec()),
        ("image1.jpg", jpeg(400, 600, [40, 80, 140, 255])),
        ("image2.jpg", jpeg(320, 480, [180, 60, 40, 255])),
    ];
    zip_epub(&package, &files, None)
}

/// Independent micro-fixture for A-15/A-16 and C-05..C-12/E-14/F-09.
/// The package deliberately has no publication-level layout; only the middle
/// spine item carries the pre-paginated override.
pub fn mixed_layout_fixed_page_recipe() -> Vec<u8> {
    let package = r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/ops" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Mixed Item Layout Recipe</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language></metadata><manifest><item id="style" href="style.css" media-type="text/css"/><item id="image" href="page.png" media-type="image/png"/><item id="section1" href="section1.xhtml" media-type="application/xhtml+xml"/><item id="fixed" href="fixed-page.xhtml" media-type="application/xhtml+xml"/><item id="section2" href="section2.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="section1" linear="yes"/><itemref idref="fixed" linear="yes" properties="rendition:layout-pre-paginated"/><itemref idref="section2" linear="yes"/></spine></package>"#;
    let section1 = br#"<html xmlns="http://www.w3.org/1999/xhtml"><head><link rel="stylesheet" href="style.css"/></head><body><p>SECTION1_REFLOWABLE</p></body></html>"#;
    let fixed = br#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:xlink="http://www.w3.org/1999/xlink"><head><link rel="stylesheet" href="style.css"/><meta name="viewport" content="width=400, height=600"/></head><body><div class="page"><svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="100%" height="100%" viewBox="0 0 400 600"><image width="400" height="600" xlink:href="page.png"/></svg></div></body></html>"#;
    let section2 = br#"<html xmlns="http://www.w3.org/1999/xhtml"><head><link rel="stylesheet" href="style.css"/></head><body><p>SECTION2_REFLOWABLE</p></body></html>"#;
    let files = vec![
        ("style.css", b"body { margin: 0; }".to_vec()),
        ("section1.xhtml", section1.to_vec()),
        ("fixed-page.xhtml", fixed.to_vec()),
        ("section2.xhtml", section2.to_vec()),
        ("page.png", png(400, 600, [80, 120, 180, 255])),
    ];
    zip_epub(package, &files, None)
}

pub fn unicode_recipe() -> Vec<u8> {
    let ivs = "\u{845B}\u{E0100}";
    let combining_dakuten = "\u{304B}\u{3099}";
    let combining_handakuten = "\u{306F}\u{309A}";
    let supplementary = "\u{20BB7}";
    let package = r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Unicode Recipe</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>ja</dc:language></metadata><manifest><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="body" linear="yes"/></spine></package>"#;
    let body = format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>UNICODE_IVS_BEGIN{ivs}UNICODE_IVS_END</p><p>UNICODE_DAKUTEN_BEGIN{combining_dakuten}UNICODE_DAKUTEN_END</p><p>UNICODE_HANDAKUTEN_BEGIN{combining_handakuten}UNICODE_HANDAKUTEN_END</p><p>UNICODE_SUPPLEMENTARY_BEGIN{supplementary}UNICODE_SUPPLEMENTARY_END</p></body></html>"#
    );
    zip_epub(package, &[("body.xhtml", body.into_bytes())], None)
}

fn png(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    let image = RgbaImage::from_pixel(width, height, Rgba(color));
    let mut output = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut output, ImageFormat::Png)
        .expect("encode fixture PNG");
    output.into_inner()
}

fn jpeg(width: u32, height: u32, color: [u8; 4]) -> Vec<u8> {
    let image = RgbaImage::from_pixel(width, height, Rgba(color));
    let mut output = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut output, ImageFormat::Jpeg)
        .expect("encode fixture JPEG");
    output.into_inner()
}

pub fn zip_epub(
    package: &str,
    files: &[(&str, Vec<u8>)],
    extra: Option<(&str, Vec<u8>)>,
) -> Vec<u8> {
    let mut output = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(&mut output);
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file("mimetype", stored).unwrap();
    zip.write_all(b"application/epub+zip").unwrap();
    zip.start_file("META-INF/container.xml", stored).unwrap();
    zip.write_all(
        br#"<container><rootfiles><rootfile full-path="package.opf"/></rootfiles></container>"#,
    )
    .unwrap();
    zip.start_file("package.opf", stored).unwrap();
    zip.write_all(package.as_bytes()).unwrap();
    for (name, data) in files {
        zip.start_file(*name, stored).unwrap();
        zip.write_all(data).unwrap();
    }
    if let Some((name, data)) = extra {
        zip.start_file(name, stored).unwrap();
        zip.write_all(&data).unwrap();
    }
    zip.finish().unwrap();
    output.into_inner()
}
