//! Primary audit coverage: D-01..D-05, D-15, D-16; applicable B/C/E geometry.

use crate::support::{CRIME_EPUB, SOVEREIGN_EPUB, convert_fixture};

// E2E-ID: E2E-BINARY-02
// Audit: G6-01; B-01..B-07, B-18..B-22, C-01..C-03, D-01..D-10, D-12, D-14..D-15, E-01..E-03, E-11, F-05..F-08, F-10..F-14 where exercised
#[test]
fn crime_and_punishment_exercises_large_rawml_and_index_geometry() {
    // Audit coverage: B-01..B-07, B-18..B-22; C-01..C-03; D-01..D-10,
    // D-12, D-14..D-15; E-01..E-03, E-11; F-05..F-08 and F-10..F-14
    // where exercised. B-11 and D-13 observations remain diagnostic only.
    // The checked-in source has one detail per index; multi-detail routing is
    // intentionally not claimed here. Record-boundary stress is exercised.
    let (input, azw3) = convert_fixture(CRIME_EPUB);
    assert_eq!(input.len(), 673_750, "checked-in generated EPUB changed");
    assert_eq!(
        azw3.bytes.len(),
        1_881_846,
        "measured AZW3 geometry changed (C-15 span AIDs, D-11 FCIS shape, and CSS empty-declaration removal are intentional)"
    );
    assert_eq!(azw3.record_count(), 576);

    let pd = azw3.palm_doc();
    assert_eq!(pd.compression, 2);
    assert_eq!(pd.text_records, 558);
    assert_eq!(pd.record_size, 4096);
    assert!(pd.text_length > 2_000_000);
    assert_eq!(
        azw3.record(559),
        &[0, 0],
        "PalmDB text/non-text bridge boundary"
    );
    assert_eq!(azw3.mobi().first_non_text, 560);
    assert!(azw3.offsets.windows(2).all(|pair| pair[0] <= pair[1]));
    assert_eq!(azw3.offsets[0], 78 + 576 * 8 + 2);
    assert!(azw3.record(1).len() <= pd.record_size);
    assert!(azw3.record(557).len() <= pd.record_size);

    let rawml = azw3.rawml();
    assert_eq!(rawml.len(), pd.text_length);
    let (tbs_records, tbs_bytes) = azw3.text_trailing_data_stats();
    assert!(tbs_records > 0, "TBS contains no non-empty entries");
    assert!(tbs_bytes > 0, "PositionMap/TBS data was lost");
    let position_map_entries = rawml
        .windows(b" aid=\"".len())
        .filter(|window| *window == b" aid=\"")
        .count();
    assert!(
        position_map_entries > 0,
        "PositionMap AID entries were lost"
    );
    println!(
        "Crime and Punishment geometry: PositionMap AID entries={position_map_entries}, TBS non-empty records={tbs_records}, TBS bytes={tbs_bytes}"
    );
    assert!(rawml.windows(b"<html".len()).count() >= 8);
    assert!(rawml.windows(b"kindle:flow:".len()).count() >= 8);
    assert!(rawml.windows(b"kindle:pos:fid:".len()).count() >= 1);

    let mobi = azw3.mobi();
    assert_eq!(mobi.fdst_flows, 8, "documented FDST flow geometry");
    assert_eq!(azw3.fdst_ranges().len(), 8);
    let ranges = azw3.fdst_ranges();
    assert_eq!(ranges[0].0, 0);
    assert_eq!(ranges.last().unwrap().1, rawml.len());
    assert!(ranges.windows(2).all(|pair| pair[0].1 == pair[1].0));
    azw3.assert_control_records();

    let frag = azw3.index_report(mobi.indx);
    assert_eq!(frag.detail_count, 1);
    assert_eq!(frag.entry_count, 313);
    assert_eq!(frag.detail_entries, vec![313]);
    let skel = azw3.index_report(mobi.skel);
    assert_eq!(skel.detail_count, 1);
    assert_eq!(skel.entry_count, 8);
    assert_eq!(skel.detail_entries, vec![8]);
    let ncx = azw3.index_report(mobi.ncx);
    assert_eq!(ncx.detail_count, 1);
    assert_eq!(ncx.entry_count, 6);
    assert_eq!(ncx.detail_entries, vec![6]);
    let guide = azw3.index_report(mobi.guide);
    assert_eq!(guide.detail_count, 1);
    assert_eq!(guide.entry_count, 1);
    assert_eq!(guide.detail_entries, vec![1]);

    let exth = azw3.exth();
    assert_eq!(exth.u32(125), Some(0));
    assert_eq!(exth.u32(116), Some(317));
    assert_eq!(exth.text(503), Some("罪と罰".to_owned()));
    assert_eq!(exth.u32(201), None);
    assert_eq!(exth.u32(202), None);
    assert_eq!(mobi.first_image, u32::MAX as usize);
    assert_eq!(mobi.last_image, u16::MAX as usize);
    assert!(mobi.title_offset + mobi.title_length <= azw3.record_zero().len());
}

// E2E-ID: E2E-BINARY-03
// Audit: D-16
#[test]
fn sovereign_stars_exercises_multi_detail_indx_geometry() {
    // Audit coverage: D-16. The fixture must cross the producer's detail-record
    // threshold; this is not evidence for diagnostic D-13.
    // The fixture is
    // generated through the canonical AozoraEpub3 workflow and contains the
    // dedicated stress section in its source asset.
    let (input, azw3) = convert_fixture(SOVEREIGN_EPUB);
    assert!(!input.is_empty());

    let mobi = azw3.mobi();
    assert!(
        [
            mobi.first_non_text,
            mobi.skel,
            mobi.guide,
            mobi.ncx,
            mobi.fdst,
            mobi.flis,
            mobi.fcis,
        ]
        .windows(2)
        .all(|pair| pair[0] < pair[1]),
        "D-16 control-record pointers are not ordered"
    );
    let frag = azw3.index_report(mobi.first_non_text);
    println!(
        "D-16 observed input_bytes={}, record_count={}, frag_detail_record_bytes={}",
        input.len(),
        azw3.record_count(),
        azw3.record(mobi.first_non_text + 1).len()
    );
    assert!(
        frag.detail_count >= 2,
        "D-16 requires a multi-detail INDX; observed entry_count={}, detail_count={}, detail_entries={:?}",
        frag.entry_count,
        frag.detail_count,
        frag.detail_entries
    );
    assert_eq!(
        frag.entry_count,
        frag.detail_entries.iter().sum(),
        "detail records do not reconstruct the main entry count"
    );
    assert!(
        frag.detail_entries.iter().all(|count| *count > 0),
        "multi-detail INDX contains an empty detail range"
    );
    println!(
        "D-16 FRAG INDX: entry_count={}, detail_count={}, detail_entries={:?}",
        frag.entry_count, frag.detail_count, frag.detail_entries
    );

    azw3.assert_control_records();
    for (name, pointer) in [
        ("NCX", mobi.ncx),
        ("FRAG", mobi.first_non_text),
        ("SKEL", mobi.skel),
        ("GUIDE", mobi.guide),
    ] {
        let report = azw3.index_report(pointer);
        assert!(
            report.entry_count > 0,
            "{name} INDX has no reconstructed entries"
        );
        assert!(
            pointer + 1 + report.detail_count <= azw3.record_count(),
            "{name} INDX detail records exceed PalmDB record bounds"
        );
    }
}
