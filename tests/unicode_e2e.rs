mod support;

use std::io::{Cursor, Read};

use support::{convert_epub, unicode_recipe};
use zip::ZipArchive;

#[test]
fn unicode_scalars_survive_source_xhtml_and_kf8_rawml_without_normalization() {
    // Audit coverage: F-05 exact scalar transport for IVS, combining marks,
    // and supplementary-plane characters.
    let epub = unicode_recipe();
    let source = epub_xhtml(&epub);
    let azw3 = convert_epub(epub);
    let rawml = String::from_utf8(azw3.rawml()).expect("Unicode RawML is UTF-8");

    let source_ivs = marked_scalars(&source, "UNICODE_IVS_BEGIN", "UNICODE_IVS_END");
    let rawml_ivs = marked_scalars(&rawml, "UNICODE_IVS_BEGIN", "UNICODE_IVS_END");
    assert_eq!(source_ivs, vec![0x845B, 0xE0100]);
    assert_eq!(rawml_ivs, source_ivs);

    let source_dakuten = marked_scalars(&source, "UNICODE_DAKUTEN_BEGIN", "UNICODE_DAKUTEN_END");
    let rawml_dakuten = marked_scalars(&rawml, "UNICODE_DAKUTEN_BEGIN", "UNICODE_DAKUTEN_END");
    assert_eq!(source_dakuten, vec![0x304B, 0x3099]);
    assert_eq!(rawml_dakuten, source_dakuten);
    assert_eq!(rawml_dakuten[1], '\u{3099}' as u32);

    let source_handakuten = marked_scalars(
        &source,
        "UNICODE_HANDAKUTEN_BEGIN",
        "UNICODE_HANDAKUTEN_END",
    );
    let rawml_handakuten =
        marked_scalars(&rawml, "UNICODE_HANDAKUTEN_BEGIN", "UNICODE_HANDAKUTEN_END");
    assert_eq!(source_handakuten, vec![0x306F, 0x309A]);
    assert_eq!(rawml_handakuten, source_handakuten);
    assert_eq!(rawml_handakuten[1], '\u{309A}' as u32);

    let source_supplementary = marked_scalars(
        &source,
        "UNICODE_SUPPLEMENTARY_BEGIN",
        "UNICODE_SUPPLEMENTARY_END",
    );
    let rawml_supplementary = marked_scalars(
        &rawml,
        "UNICODE_SUPPLEMENTARY_BEGIN",
        "UNICODE_SUPPLEMENTARY_END",
    );
    assert_eq!(source_supplementary, vec![0x20BB7]);
    assert_eq!(rawml_supplementary, source_supplementary);
    assert_eq!(rawml_supplementary[0], '\u{20BB7}' as u32);
}

fn epub_xhtml(epub: &[u8]) -> String {
    let mut archive = ZipArchive::new(Cursor::new(epub)).expect("open recipe EPUB ZIP");
    let mut text = String::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).expect("read recipe EPUB entry");
        if entry.name().ends_with(".xhtml") {
            entry
                .read_to_string(&mut text)
                .expect("decode recipe XHTML");
        }
    }
    text
}

fn marked_scalars(input: &str, start_marker: &str, end_marker: &str) -> Vec<u32> {
    let start = input
        .find(start_marker)
        .expect("Unicode start marker is present")
        + start_marker.len();
    let end = input[start..]
        .find(end_marker)
        .map(|offset| start + offset)
        .expect("Unicode end marker is present");
    input[start..end].chars().map(u32::from).collect()
}
