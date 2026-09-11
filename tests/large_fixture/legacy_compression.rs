//! Primary audit coverage: B-02, B-05a, and applicable text-record geometry.

use std::collections::HashMap;

use crate::support;
use crate::support::CRIME_EPUB;

// E2E-ID: E2E-BINARY-01
// Audit: B-02, B-04, B-05a; applicable Layer D text-record geometry
#[test]
fn palmdoc_payloads_remain_byte_identical_to_the_legacy_encoder() {
    // Audit coverage: B-02, B-04, B-05a and directly exercised text-record
    // compression/UTF-8-overlap geometry. This does not claim B-11 parity.
    let input = std::fs::read(CRIME_EPUB).expect("read the canonical large fixture");
    let compressed = support::convert_epub(input.clone());
    let uncompressed = support::convert_epub_uncompressed(input);
    let source = uncompressed.rawml();
    let compressed_pd = compressed.palm_doc();
    let uncompressed_pd = uncompressed.palm_doc();

    assert_eq!(source.len(), compressed_pd.text_length);
    assert_eq!(compressed_pd.text_records, uncompressed_pd.text_records);
    let source_chunks = source.chunks(4096).collect::<Vec<_>>();
    for (record_index, chunk) in source_chunks.iter().enumerate() {
        let expected = legacy_compress(chunk);
        assert_eq!(
            compressed.text_payload(record_index + 1),
            expected,
            "PalmDOC payload differs for record {record_index}"
        );

        let expected_overlap = record_utf8_overlap(&source_chunks, record_index);
        let (actual_overlap, marker) = compressed.text_overlap(record_index + 1);
        assert_eq!(
            actual_overlap, expected_overlap,
            "UTF-8 overlap differs for record {record_index}"
        );
        assert_eq!(
            marker,
            expected_overlap.len() as u8,
            "UTF-8 overlap marker differs for record {record_index}"
        );
    }
}

fn record_utf8_overlap<'a>(chunks: &'a [&'a [u8]], record_index: usize) -> &'a [u8] {
    let Some(next) = chunks.get(record_index + 1) else {
        return &[];
    };
    let previous = chunks[record_index];
    let previous_start = previous.len().saturating_sub(3);
    let previous_tail = &previous[previous_start..];
    let next_head = &next[..next.len().min(3)];
    let mut combined = [0u8; 6];
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
            return &next_head[..end - boundary];
        }
    }
    &[]
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

fn legacy_compress(source: &[u8]) -> Vec<u8> {
    let mut encoded = Vec::with_capacity(source.len());
    let mut previous = (3..=10)
        .map(|_| HashMap::<Vec<u8>, usize>::new())
        .collect::<Vec<_>>();
    let mut cursor = 0usize;
    while cursor < source.len() {
        for length in 3..=10 {
            if cursor >= length {
                previous[length - 3]
                    .insert(source[cursor - length..cursor].to_vec(), cursor - length);
            }
        }
        if cursor > 10 && source.len() - cursor > 10 {
            let mut match_position = None;
            let mut match_length = 0usize;
            for length in (3..=10).rev() {
                let needle = source[cursor..cursor + length].to_vec();
                if let Some(&position) = previous[length - 3].get(&needle) {
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
        let mut literal = vec![byte];
        while cursor < source.len() && literal.len() < 8 {
            let next = source[cursor];
            if next == 0 || (next > 8 && next < 0x80) {
                break;
            }
            literal.push(next);
            cursor += 1;
        }
        encoded.push(literal.len() as u8);
        encoded.extend_from_slice(&literal);
    }
    encoded
}
