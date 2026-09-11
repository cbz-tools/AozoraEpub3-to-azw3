//! Diagnostic characterization only; these observations are not primary release
//! coverage unless the unified audit cites a specific forward-looking E2E.

use std::io::{Cursor, Read};

use crate::support::{Azw3, zip_epub};
use aozoraepub3_to_azw3::{ConvertOptions, convert_bytes};
use zip::ZipArchive;

const BASELINE_MARKER: &str = "EPUB33_BASELINE_MARKER";

fn source_entry(epub: &[u8], path: &str) -> Vec<u8> {
    let mut archive = ZipArchive::new(Cursor::new(epub)).expect("open characterization EPUB");
    let mut entry = archive
        .by_name(path)
        .unwrap_or_else(|error| panic!("missing characterization source entry {path}: {error}"));
    let mut bytes = Vec::new();
    entry
        .read_to_end(&mut bytes)
        .expect("read characterization source entry");
    bytes
}

fn source_text(epub: &[u8], path: &str) -> String {
    let bytes = source_entry(epub, path);
    decode_text_bytes(&bytes)
}

fn decode_text_bytes(bytes: &[u8]) -> String {
    let (little_endian, offset) = if bytes.starts_with(&[0xff, 0xfe]) {
        (true, 2)
    } else if bytes.starts_with(&[0xfe, 0xff]) {
        (false, 2)
    } else {
        return String::from_utf8_lossy(bytes).into_owned();
    };
    let units = bytes[offset..]
        .chunks_exact(2)
        .map(|pair| {
            if little_endian {
                u16::from_le_bytes([pair[0], pair[1]])
            } else {
                u16::from_be_bytes([pair[0], pair[1]])
            }
        })
        .collect::<Vec<_>>();
    String::from_utf16_lossy(&units)
}

fn utf16_bytes(source: &str, little_endian: bool, bom: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    if bom {
        bytes.extend_from_slice(if little_endian {
            &[0xff, 0xfe]
        } else {
            &[0xfe, 0xff]
        });
    }
    for unit in source.encode_utf16() {
        let encoded = if little_endian {
            unit.to_le_bytes()
        } else {
            unit.to_be_bytes()
        };
        bytes.extend_from_slice(&encoded);
    }
    bytes
}

fn convert_result(epub: &[u8]) -> Result<Azw3, String> {
    convert_bytes(epub, &ConvertOptions::default())
        .map(Azw3::parse)
        .map_err(|error| error.to_string())
}

fn reconstructed_text(azw3: &Azw3) -> String {
    azw3.reconstructed_xhtml_sections()
        .into_iter()
        .map(|section| String::from_utf8_lossy(&section).into_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

fn index_entries(azw3: &Azw3, pointer: usize) -> usize {
    if pointer < azw3.record_count() && azw3.record(pointer).starts_with(b"INDX") {
        azw3.index_report(pointer).entry_count
    } else {
        0
    }
}

fn observe(
    ids: &str,
    fixture: &str,
    epub: Vec<u8>,
    required_success_marker: Option<&str>,
) -> Option<Azw3> {
    match convert_bytes(&epub, &ConvertOptions::default()) {
        Ok(bytes) => {
            let azw3 = Azw3::parse(bytes);
            let rawml_bytes = azw3.rawml();
            let rawml = String::from_utf8_lossy(&rawml_bytes);
            let marker = required_success_marker.unwrap_or(BASELINE_MARKER);
            let marker_observed = rawml.contains(marker);
            let channels = format!(
                "rawml-marker={marker_observed};fdst-ranges={};images={};ncx-entries={};guide-entries={}",
                azw3.fdst_ranges().len(),
                azw3.image_records().len(),
                index_entries(&azw3, azw3.mobi().ncx),
                index_entries(&azw3, azw3.mobi().guide),
            );
            println!(
                "EPUB33_OBSERVATION|ids={ids}|fixture={fixture}|result=success|semantic-marker={marker}:{marker_observed}|channels={channels}"
            );
            if required_success_marker.is_some() {
                assert!(
                    marker_observed,
                    "{ids}: baseline semantic marker was lost in a successful conversion"
                );
            }
            Some(azw3)
        }
        Err(error) => {
            println!(
                "EPUB33_OBSERVATION|ids={ids}|fixture={fixture}|result=error|semantic-marker=not-applicable|channels=none|error={error}"
            );
            None
        }
    }
}

fn utf16le(source: &str) -> Vec<u8> {
    let mut bytes = vec![0xff, 0xfe];
    for unit in source.encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    bytes
}

fn base_package(manifest: &str, spine: &str, metadata: &str) -> String {
    format!(
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/">{metadata}</metadata><manifest>{manifest}</manifest><spine>{spine}</spine></package>"#
    )
}

fn basic_metadata() -> &'static str {
    "<dc:title>EPUB 3.3 Characterization</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language>"
}

#[test]
fn encoding_boundaries_are_observable() {
    let package = base_package(
        r#"<item id="style" href="style.css" media-type="text/css"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#,
        r#"<itemref idref="body"/>"#,
        basic_metadata(),
    );
    let body = utf16le(
        r#"<?xml version="1.0" encoding="UTF-16"?><html xmlns="http://www.w3.org/1999/xhtml"><body><p>EPUB33_BASELINE_MARKER</p></body></html>"#,
    );
    let css = utf16le(r#"@charset "UTF-16"; body { color: red; }"#);
    let epub = zip_epub(&package, &[("body.xhtml", body), ("style.css", css)], None);
    assert!(source_entry(&epub, "body.xhtml").starts_with(&[0xff, 0xfe]));
    assert!(source_text(&epub, "body.xhtml").contains("encoding=\"UTF-16\""));
    assert!(source_entry(&epub, "style.css").starts_with(&[0xff, 0xfe]));
    assert!(source_text(&epub, "style.css").contains("@charset"));
    let _ = observe("G4-03..G4-06", "utf16-xhtml-css", epub, None);
}

#[test]
fn manifest_fallback_and_remote_resources_are_observable() {
    let fallback_package = base_package(
        r#"<item id="foreign" href="foreign.bin" media-type="application/octet-stream" fallback="body"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#,
        r#"<itemref idref="foreign"/>"#,
        basic_metadata(),
    );
    let fallback_epub = zip_epub(
        &fallback_package,
        &[
            ("foreign.bin", b"FOREIGN_RESOURCE_MARKER".to_vec()),
            (
                "body.xhtml",
                format!(
                    r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>{BASELINE_MARKER}</p></body></html>"#
                )
                .into_bytes(),
            ),
        ],
        None,
    );
    let fallback_source = source_text(&fallback_epub, "package.opf");
    assert!(fallback_source.contains("fallback=\"body\""));
    assert!(fallback_source.contains("media-type=\"application/octet-stream\""));
    let _ = observe(
        "G2-13..G2-14",
        "manifest-fallback-foreign-spine",
        fallback_epub,
        None,
    );

    let remote_package = base_package(
        r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/><item id="remote" href="https://example.invalid/remote.css" media-type="text/css" properties="remote-resources"/>"#,
        r#"<itemref idref="body"/>"#,
        basic_metadata(),
    );
    let remote_epub = zip_epub(
        &remote_package,
        &[(
            "body.xhtml",
            format!(
                r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>{BASELINE_MARKER}</p></body></html>"#
            )
            .into_bytes(),
        )],
        None,
    );
    let remote_source = source_text(&remote_epub, "package.opf");
    assert!(remote_source.contains("https://example.invalid/remote.css"));
    assert!(remote_source.contains("remote-resources"));
    let _ = observe("G2-15..G2-16", "remote-resources", remote_epub, None);
}

#[test]
fn svg_mathml_and_content_document_boundaries_are_observable() {
    let package = base_package(
        r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/><item id="svg" href="diagram.svg" media-type="image/svg+xml"/>"#,
        r#"<itemref idref="body"/><itemref idref="svg"/>"#,
        basic_metadata(),
    );
    let body = format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>{BASELINE_MARKER}</p><svg xmlns="http://www.w3.org/2000/svg" role="img"><title>INLINE_SVG_MARKER</title><circle cx="1" cy="1" r="1"/></svg><math xmlns="http://www.w3.org/1998/Math/MathML"><mi>MATHML_MARKER</mi></math></body></html>"#
    );
    let svg = br#"<svg xmlns="http://www.w3.org/2000/svg"><title>SPINE_SVG_MARKER</title><rect width="10" height="10"/></svg>"#;
    let epub = zip_epub(
        &package,
        &[
            ("body.xhtml", body.into_bytes()),
            ("diagram.svg", svg.to_vec()),
        ],
        None,
    );
    let source = source_text(&epub, "body.xhtml");
    assert!(source.contains("<svg"));
    assert!(source.contains("<math"));
    assert!(source_entry(&epub, "diagram.svg").starts_with(b"<svg"));
    let azw3 = observe(
        "G3-04,G3-05,G3-07",
        "inline-svg-mathml-svg-spine",
        epub,
        Some(BASELINE_MARKER),
    );
    if let Some(azw3) = azw3 {
        let rawml_bytes = azw3.rawml();
        let rawml = String::from_utf8_lossy(&rawml_bytes);
        println!(
            "EPUB33_DETAIL|ids=G3-04,G3-05,G3-07|inline-svg-marker={}|mathml-marker={}|spine-svg-marker={}|image-records={}",
            rawml.contains("INLINE_SVG_MARKER"),
            rawml.contains("MATHML_MARKER"),
            rawml.contains("SPINE_SVG_MARKER"),
            azw3.image_records().len(),
        );
    }
}

#[test]
fn navigation_page_list_and_ncx_boundaries_are_observable() {
    let package = base_package(
        r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>"#,
        r#"<itemref idref="body"/>"#,
        &format!(
            r#"{}<meta property="dcterms:modified">2026-09-08T00:00:00Z</meta>"#,
            basic_metadata()
        ),
    )
    .replace("<spine>", "<spine toc=\"ncx\">");
    let body = format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>{BASELINE_MARKER}</p><span epub:type="pagebreak" id="page-1">1</span><p>PAGE_BREAK_MARKER</p></body></html>"#
    );
    let nav = br#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="body.xhtml">TOC_MARKER</a></li></ol></nav><nav epub:type="page-list"><ol><li><a href="body.xhtml#page-1">PAGE_LIST_MARKER</a></li></ol></nav><nav epub:type="landmarks"><ol><li><a epub:type="bodymatter" href="body.xhtml">BODYMATTER_MARKER</a></li></ol></nav><nav epub:type="custom"><ol><li><a href="body.xhtml">CUSTOM_NAV_MARKER</a></li></ol></nav></body></html>"#;
    let ncx = br#"<?xml version="1.0"?><ncx><navMap><navPoint><navLabel><text>NCX_MARKER</text></navLabel><content src="body.xhtml"/></navPoint></navMap></ncx>"#;
    let epub = zip_epub(
        &package,
        &[
            ("body.xhtml", body.into_bytes()),
            ("nav.xhtml", nav.to_vec()),
            ("toc.ncx", ncx.to_vec()),
        ],
        None,
    );
    let nav_source = source_text(&epub, "nav.xhtml");
    assert!(nav_source.contains("epub:type=\"page-list\""));
    assert_eq!(nav_source.matches("<nav ").count(), 4);
    assert!(source_text(&epub, "package.opf").contains("application/x-dtbncx+xml"));
    let azw3 = observe(
        "G5-09,G5-10,G5-12,G5-16",
        "page-list-multiple-nav-plus-ncx",
        epub,
        Some(BASELINE_MARKER),
    );
    if let Some(azw3) = azw3 {
        assert!(azw3.index_report(azw3.mobi().ncx).entry_count >= 1);
        let rawml_bytes = azw3.rawml();
        let rawml = String::from_utf8_lossy(&rawml_bytes);
        assert!(rawml.contains("PAGE_BREAK_MARKER"));
        println!(
            "EPUB33_DETAIL|ids=G5-09,G5-10,G5-12,G5-16|toc-marker={}|page-list-marker={}|custom-nav-marker={}|ncx-marker={}|rawml-page-break={}",
            rawml.contains("TOC_MARKER"),
            rawml.contains("PAGE_LIST_MARKER"),
            rawml.contains("CUSTOM_NAV_MARKER"),
            rawml.contains("NCX_MARKER"),
            rawml.contains("PAGE_BREAK_MARKER"),
        );
    }
}

#[test]
fn font_encryption_and_obfuscation_are_observable() {
    let package = base_package(
        r#"<item id="style" href="style.css" media-type="text/css"/><item id="font" href="fonts/obfuscated.ttf" media-type="font/ttf"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#,
        r#"<itemref idref="body"/>"#,
        basic_metadata(),
    );
    let font = b"OBFUSCATED_FONT_PAYLOAD".to_vec();
    let encryption = br#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container"><EncryptedData><CipherData><CipherReference URI="OEBPS/fonts/obfuscated.ttf"/></CipherData></EncryptedData></encryption>"#;
    let epub = zip_epub(
        &package,
        &[
            ("style.css", b"@font-face { font-family: Obfuscated; src: url('fonts/obfuscated.ttf'); }".to_vec()),
            ("fonts/obfuscated.ttf", font.clone()),
            ("body.xhtml", format!(r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><link rel="stylesheet" href="style.css"/></head><body><p>{BASELINE_MARKER}</p></body></html>"#).into_bytes()),
            ("META-INF/encryption.xml", encryption.to_vec()),
        ],
        None,
    );
    assert!(source_text(&epub, "META-INF/encryption.xml").contains("obfuscated.ttf"));
    assert_eq!(&source_entry(&epub, "fonts/obfuscated.ttf"), &font);
    let azw3 = observe(
        "G1-06,G1-07,G6-21",
        "encrypted-obfuscated-font",
        epub,
        Some(BASELINE_MARKER),
    );
    if let Some(azw3) = azw3 {
        println!(
            "EPUB33_DETAIL|ids=G1-06,G1-07,G6-21|encrypted-font-bytes-preserved={}",
            bytes_contain(&azw3.bytes, &font),
        );
    }
}

#[test]
fn metadata_refinements_and_collection_are_observable() {
    let metadata = format!(
        r##"{basic}<dc:title id="title">Refined Title</dc:title><dc:creator id="creator">Refined Author</dc:creator><meta refines="#creator" property="role" scheme="marc:relators">aut</meta><meta refines="#title" property="file-as">Title, Refined</meta><meta refines="#title" property="title-type">main</meta><dc:collection id="collection">Series Marker</dc:collection>"##,
        basic = basic_metadata()
    );
    let package = base_package(
        r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#,
        r#"<itemref idref="body"/>"#,
        &metadata,
    );
    let epub = zip_epub(
        &package,
        &[("body.xhtml", format!(r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>{BASELINE_MARKER}</p></body></html>"#).into_bytes())],
        None,
    );
    let source = source_text(&epub, "package.opf");
    for marker in [
        "property=\"role\"",
        "property=\"file-as\"",
        "property=\"title-type\"",
        "<dc:collection",
    ] {
        assert!(source.contains(marker), "metadata source lost {marker}");
    }
    let azw3 = observe(
        "G2-07..G2-11",
        "metadata-refinements-collection",
        epub,
        Some(BASELINE_MARKER),
    );
    if let Some(azw3) = azw3 {
        assert_eq!(azw3.exth().text(503).as_deref(), Some("Refined Title"));
        println!(
            "EPUB33_DETAIL|ids=G2-07..G2-11|exth-title={:?}|exth-creator={:?}",
            azw3.exth().text(503),
            azw3.exth().text(100),
        );
    }
}

#[test]
fn rendition_spread_orientation_and_viewport_are_observable() {
    let metadata = format!(
        r#"{basic}<meta property="rendition:orientation">portrait</meta><meta property="rendition:spread">both</meta><meta property="rendition:flow">scrolled</meta><meta property="rendition:align-x">center</meta>"#,
        basic = basic_metadata()
    );
    let package = base_package(
        r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#,
        r#"<itemref idref="body" properties="rendition:orientation-landscape rendition:spread-left rendition:page-spread-right rendition:flow-paginated rendition:align-x-center rendition:layout-reflowable"/>"#,
        &metadata,
    );
    let body = format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><meta name="viewport" content="width=600,height=800"/></head><body><p>{BASELINE_MARKER}</p><p>VIEWPORT_MARKER</p></body></html>"#
    );
    let epub = zip_epub(&package, &[("body.xhtml", body.into_bytes())], None);
    let package_source = source_text(&epub, "package.opf");
    for marker in [
        "rendition:orientation",
        "rendition:spread",
        "rendition:flow",
        "rendition:align-x-center",
        "rendition:page-spread-right",
    ] {
        assert!(
            package_source.contains(marker),
            "rendition source lost {marker}"
        );
    }
    assert!(source_text(&epub, "body.xhtml").contains("viewport"));
    let azw3 = observe(
        "G6-04..G6-11,G6-14",
        "rendition-spread-orientation-viewport",
        epub,
        Some(BASELINE_MARKER),
    );
    if let Some(azw3) = azw3 {
        assert!(!azw3.fdst_ranges().is_empty());
        let rawml_bytes = azw3.rawml();
        let rawml = String::from_utf8_lossy(&rawml_bytes);
        println!(
            "EPUB33_DETAIL|ids=G6-04..G6-11,G6-14|exth-122={:?}|exth-525={:?}|exth-527={:?}|rawml-viewport={}",
            azw3.exth().text(122),
            azw3.exth().text(525),
            azw3.exth().text(527),
            rawml.contains("width=600,height=800"),
        );
    }
}

#[test]
fn language_bidi_and_css_direction_are_observable() {
    let package = base_package(
        r#"<item id="style" href="style.css" media-type="text/css"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#,
        r#"<itemref idref="body"/>"#,
        basic_metadata(),
    );
    let body = format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml" xml:lang="en" lang="ja" dir="rtl"><body><p>{BASELINE_MARKER}</p><p lang="ar" dir="rtl">LANG_SWITCH_MARKER</p><p xml:lang="he">XML_LANG_SWITCH_MARKER</p></body></html>"#
    );
    let css = b"html { direction: ltr; } body[dir=rtl] { direction: rtl; } .bidi { unicode-bidi: isolate; }".to_vec();
    let epub = zip_epub(
        &package,
        &[("body.xhtml", body.into_bytes()), ("style.css", css)],
        None,
    );
    let body_source = source_text(&epub, "body.xhtml");
    for marker in [
        "xml:lang=\"en\"",
        "lang=\"ja\"",
        "dir=\"rtl\"",
        "xml:lang=\"he\"",
    ] {
        assert!(
            body_source.contains(marker),
            "language/bidi source lost {marker}"
        );
    }
    assert!(source_text(&epub, "style.css").contains("unicode-bidi"));
    let azw3 = observe(
        "G3-10..G3-12,G7-18",
        "language-switch-bidi",
        epub,
        Some(BASELINE_MARKER),
    );
    if let Some(azw3) = azw3 {
        let rawml_bytes = azw3.rawml();
        let rawml = String::from_utf8_lossy(&rawml_bytes);
        assert!(rawml.contains("LANG_SWITCH_MARKER"));
        println!(
            "EPUB33_DETAIL|ids=G3-10..G3-12,G7-18|xml-lang-root={}|html-lang-root={}|dir-root={}|xml-lang-nested={}|css-direction={}",
            rawml.contains("xml:lang=\"en\""),
            rawml.contains("lang=\"ja\""),
            rawml.contains("dir=\"rtl\""),
            rawml.contains("xml:lang=\"he\""),
            rawml.contains("direction: rtl"),
        );
    }
}

#[test]
fn css_selectors_at_rules_and_variables_are_observable() {
    let package = base_package(
        r#"<item id="style" href="style.css" media-type="text/css"/><item id="imported" href="imported.css" media-type="text/css"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#,
        r#"<itemref idref="body"/>"#,
        basic_metadata(),
    );
    let css = br#"@import "imported.css"; :root { --epub33-color: red; } body > p + p ~ p[data-kind="special"] { color: var(--epub33-color); } p:first-child::before { content: "GENERATED_CONTENT_MARKER"; } @media (min-width: 1px) { body { page-break-before: always; } } @supports (display: grid) { body { --supports-marker: yes; } } .counter { counter-reset: chapter 1; counter-increment: chapter; } .counter::after { content: counter(chapter); } .data { background-image: url(data:image/png;base64,AAAA); }"#;
    let imported = b".imported { color: blue; }".to_vec();
    let body = format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><link rel="stylesheet" href="style.css"/></head><body><p class="counter">{BASELINE_MARKER}</p><p>CSS_SELECTOR_MARKER</p><p data-kind="special">CSS_ATTRIBUTE_MARKER</p></body></html>"#
    );
    let epub = zip_epub(
        &package,
        &[
            ("style.css", css.to_vec()),
            ("imported.css", imported),
            ("body.xhtml", body.into_bytes()),
        ],
        None,
    );
    let css_source = source_text(&epub, "style.css");
    for marker in [
        " > ",
        "+ p",
        "~ p",
        "[data-kind",
        ":first-child",
        "::before",
        "@media",
        "@supports",
        "@import",
        "var(",
        "counter(",
        "data:image",
        "page-break",
    ] {
        assert!(css_source.contains(marker), "CSS source lost {marker}");
    }
    let azw3 = observe(
        "G7-07..G7-12,G7-14,G7-16,G7-24..G7-28",
        "css-combinators-at-rules-generated-content-data-vars",
        epub,
        Some(BASELINE_MARKER),
    );
    if let Some(azw3) = azw3 {
        let rawml_bytes = azw3.rawml();
        let rawml = String::from_utf8_lossy(&rawml_bytes);
        assert!(rawml.contains("kindle:flow:"));
        assert!(rawml.contains("CSS_SELECTOR_MARKER"));
        println!(
            "EPUB33_DETAIL|ids=G7-07..G7-12,G7-14,G7-16,G7-24..G7-28|import={}|media={}|supports={}|custom-property={}|generated-content={}|counter={}|data-url={}|page-break={}",
            rawml.contains("@import"),
            rawml.contains("@media"),
            rawml.contains("@supports"),
            rawml.contains("--epub33-color"),
            rawml.contains("GENERATED_CONTENT_MARKER"),
            rawml.contains("counter("),
            rawml.contains("data:image"),
            rawml.contains("page-break-before"),
        );
    }
}

fn basic_body(marker: &str, extra: &str) -> Vec<u8> {
    format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>{marker}</p>{extra}</body></html>"#
    )
    .into_bytes()
}

fn conversion_observation(
    ids: &str,
    fixture: &str,
    result: Result<Azw3, String>,
    marker: Option<&str>,
) -> Result<Azw3, String> {
    match result {
        Ok(azw3) => {
            let reconstructed = reconstructed_text(&azw3);
            let marker_observed = marker.is_some_and(|value| reconstructed.contains(value));
            println!(
                "EPUB33_SECOND_OBSERVATION|ids={ids}|fixture={fixture}|result=success|sections={}|marker={:?}:{}|fdst-ranges={}|images={}",
                azw3.reconstructed_xhtml_sections().len(),
                marker,
                marker_observed,
                azw3.fdst_ranges().len(),
                azw3.image_records().len(),
            );
            Ok(azw3)
        }
        Err(error) => {
            println!(
                "EPUB33_SECOND_OBSERVATION|ids={ids}|fixture={fixture}|result=error|error={error}"
            );
            Err(error)
        }
    }
}

#[test]
fn encoding_matrix_remains_diagnostic() {
    let xhtml_cases = [
        (
            "utf16le-bom-declaration",
            utf16_bytes(
                r#"<?xml version="1.0" encoding="UTF-16"?><html xmlns="http://www.w3.org/1999/xhtml"><body><p>UTF16_XHTML_MARKER</p></body></html>"#,
                true,
                true,
            ),
        ),
        (
            "utf16le-bom-only",
            utf16_bytes(
                r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>UTF16_XHTML_MARKER</p></body></html>"#,
                true,
                true,
            ),
        ),
        (
            "utf16be-bom-declaration",
            utf16_bytes(
                r#"<?xml version="1.0" encoding="UTF-16"?><html xmlns="http://www.w3.org/1999/xhtml"><body><p>UTF16_XHTML_MARKER</p></body></html>"#,
                false,
                true,
            ),
        ),
        (
            "utf16be-bom-only",
            utf16_bytes(
                r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>UTF16_XHTML_MARKER</p></body></html>"#,
                false,
                true,
            ),
        ),
        (
            "utf16le-no-bom-declaration",
            utf16_bytes(
                r#"<?xml version="1.0" encoding="UTF-16"?><html xmlns="http://www.w3.org/1999/xhtml"><body><p>UTF16_XHTML_MARKER</p></body></html>"#,
                true,
                false,
            ),
        ),
        (
            "utf16be-no-bom-declaration",
            utf16_bytes(
                r#"<?xml version="1.0" encoding="UTF-16"?><html xmlns="http://www.w3.org/1999/xhtml"><body><p>UTF16_XHTML_MARKER</p></body></html>"#,
                false,
                false,
            ),
        ),
        (
            "utf8-bytes-utf16-declaration",
            br#"<?xml version="1.0" encoding="UTF-16"?><html xmlns="http://www.w3.org/1999/xhtml"><body><p>UTF16_XHTML_MARKER</p></body></html>"#.to_vec(),
        ),
    ];
    let package = base_package(
        r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#,
        r#"<itemref idref="body"/>"#,
        basic_metadata(),
    );
    for (fixture, body) in xhtml_cases {
        let epub = zip_epub(&package, &[("body.xhtml", body.clone())], None);
        let source = source_entry(&epub, "body.xhtml");
        assert_eq!(decode_text_bytes(&source), decode_text_bytes(&body));
        let result = conversion_observation(
            "G4-03,G4-05",
            fixture,
            convert_result(&epub),
            Some("UTF16_XHTML_MARKER"),
        );
        if fixture == "utf8-bytes-utf16-declaration" {
            let error = result.expect_err("mismatched XHTML encoding must fail explicitly");
            assert!(
                error.contains("encoding declaration"),
                "unexpected XHTML error: {error}"
            );
        } else {
            let azw3 =
                result.unwrap_or_else(|error| panic!("{fixture} unexpectedly failed: {error}"));
            assert!(reconstructed_text(&azw3).contains("UTF16_XHTML_MARKER"));
        }
    }

    let css_cases = [
        (
            "utf16le-css-bom",
            utf16_bytes(
                "@charset \"UTF-16\"; body { --epub33-css-marker: \"UTF16_CSS_MARKER\"; }",
                true,
                true,
            ),
        ),
        (
            "utf16be-css-bom",
            utf16_bytes(
                "@charset \"UTF-16\"; body { --epub33-css-marker: \"UTF16_CSS_MARKER\"; }",
                false,
                true,
            ),
        ),
        (
            "utf8-css-charset",
            br#"@charset "UTF-8"; body { --epub33-css-marker: "UTF16_CSS_MARKER"; }"#.to_vec(),
        ),
        (
            "utf16-css-charset",
            utf16_bytes(
                "@charset \"UTF-16\"; body { --epub33-css-marker: \"UTF16_CSS_MARKER\"; }",
                true,
                true,
            ),
        ),
        (
            "utf16le-css-no-bom-charset",
            utf16_bytes(
                "@charset \"UTF-16\"; body { --epub33-css-marker: \"UTF16_CSS_MARKER\"; }",
                true,
                false,
            ),
        ),
        (
            "utf16be-css-no-bom-charset",
            utf16_bytes(
                "@charset \"UTF-16\"; body { --epub33-css-marker: \"UTF16_CSS_MARKER\"; }",
                false,
                false,
            ),
        ),
    ];
    let package = base_package(
        r#"<item id="style" href="style.css" media-type="text/css"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#,
        r#"<itemref idref="body"/>"#,
        basic_metadata(),
    );
    for (fixture, css) in css_cases {
        let body = r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><link rel="stylesheet" href="style.css"/></head><body><p>UTF16_CSS_BASELINE_MARKER</p><p>UTF16_CSS_BODY_MARKER</p></body></html>"#
            .to_string()
            .into_bytes();
        let epub = zip_epub(
            &package,
            &[("style.css", css.clone()), ("body.xhtml", body)],
            None,
        );
        let source = source_entry(&epub, "style.css");
        assert_eq!(decode_text_bytes(&source), decode_text_bytes(&css));
        let result = conversion_observation(
            "G4-04,G4-06",
            fixture,
            convert_result(&epub),
            Some("UTF16_CSS_BASELINE_MARKER"),
        );
        let azw3 = result.unwrap_or_else(|error| panic!("{fixture} unexpectedly failed: {error}"));
        let rawml_bytes = azw3.rawml();
        let rawml = String::from_utf8_lossy(&rawml_bytes);
        println!(
            "EPUB33_SECOND_DETAIL|ids=G4-04,G4-06|fixture={fixture}|css-marker={}|css-nul={}|replacement={}",
            rawml.contains("UTF16_CSS_MARKER"),
            rawml_bytes.contains(&0),
            rawml.contains('\u{fffd}'),
        );
        assert!(rawml.contains("UTF16_CSS_BODY_MARKER"));
        assert!(
            rawml.contains("UTF16_CSS_MARKER"),
            "CSS marker lost for {fixture}"
        );
        assert!(
            !rawml.contains('\u{fffd}'),
            "CSS decode inserted replacement characters for {fixture}"
        );
    }

    let invalid_css =
        br#"@charset "ISO-8859-1"; body { --epub33-css-marker: "INVALID_CSS_MARKER"; }"#;
    let error = convert_result(&zip_epub(
        &package,
        &[
            ("style.css", invalid_css.to_vec()),
            (
                "body.xhtml",
                br#"<html xmlns="http://www.w3.org/1999/xhtml"><head><link rel="stylesheet" href="style.css"/></head><body><p>INVALID_CSS_BASELINE</p></body></html>"#.to_vec(),
            ),
        ],
        None,
    ))
    .expect_err("unsupported CSS encoding declaration must be rejected explicitly");
    assert!(error.contains("unsupported CSS encoding declaration"));
}

fn run_fallback_case(
    fixture: &str,
    package: String,
    files: Vec<(&str, Vec<u8>)>,
    expect_fallback_marker: bool,
    expect_foreign_marker: bool,
    expected_sections: usize,
) {
    let epub = zip_epub(&package, &files, None);
    let result = conversion_observation(
        "G2-13,G2-14",
        fixture,
        convert_result(&epub),
        Some("FALLBACK_MARKER"),
    );
    let azw3 = result.unwrap_or_else(|error| panic!("{fixture} unexpectedly failed: {error}"));
    let reconstructed = reconstructed_text(&azw3);
    let fallback_observed = reconstructed.contains("FALLBACK_MARKER");
    let foreign_observed = reconstructed.contains("FOREIGN_MARKER");
    println!(
        "EPUB33_SECOND_DETAIL|ids=G2-13,G2-14|fixture={fixture}|sections={}|baseline={}|fallback-marker={fallback_observed}|foreign-marker={foreign_observed}",
        azw3.reconstructed_xhtml_sections().len(),
        reconstructed.contains(BASELINE_MARKER),
    );
    assert_eq!(azw3.reconstructed_xhtml_sections().len(), expected_sections);
    assert!(reconstructed.contains(BASELINE_MARKER));
    assert_eq!(fallback_observed, expect_fallback_marker);
    assert_eq!(foreign_observed, expect_foreign_marker);
}

fn assert_invalid_fallback_case(
    fixture: &str,
    package: String,
    files: Vec<(&str, Vec<u8>)>,
    expected_error: &str,
) {
    let result = convert_result(&zip_epub(&package, &files, None));
    let error = result.expect_err("malformed fallback must not produce a successful AZW3");
    println!("EPUB33_SECOND_DETAIL|ids=G2-13,G2-14|fixture={fixture}|explicit-error={error}");
    assert!(
        error.contains(expected_error),
        "unexpected fallback error for {fixture}: {error}"
    );
}

#[test]
fn fallback_matrix_remains_diagnostic() {
    let foreign = b"FOREIGN_MARKER".to_vec();
    let body = basic_body(BASELINE_MARKER, "");
    let fallback = basic_body("FALLBACK_MARKER", "");

    let manifest = r#"<item id="foreign" href="foreign.bin" media-type="application/vnd.example.foreign" fallback="fallback"/><item id="fallback" href="fallback.xhtml" media-type="application/xhtml+xml"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#;
    let package = base_package(
        manifest,
        r#"<itemref idref="foreign"/><itemref idref="fallback"/><itemref idref="body"/>"#,
        basic_metadata(),
    );
    run_fallback_case(
        "foreign-spine-valid-fallback-also-in-spine",
        package,
        vec![
            ("foreign.bin", foreign.clone()),
            ("fallback.xhtml", fallback.clone()),
            ("body.xhtml", body.clone()),
        ],
        true,
        false,
        3,
    );

    let manifest = r#"<item id="foreign" href="foreign.bin" media-type="application/vnd.example.foreign" fallback="fallback"/><item id="fallback" href="fallback.xhtml" media-type="application/xhtml+xml"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#;
    let package = base_package(
        manifest,
        r#"<itemref idref="foreign"/><itemref idref="body"/>"#,
        basic_metadata(),
    );
    run_fallback_case(
        "foreign-spine-valid-fallback-neighboring-xhtml",
        package,
        vec![
            ("foreign.bin", foreign.clone()),
            ("fallback.xhtml", fallback.clone()),
            ("body.xhtml", body.clone()),
        ],
        true,
        false,
        2,
    );

    let manifest = r#"<item id="foreign-a" href="foreign-a.bin" media-type="application/vnd.example.foreign" fallback="foreign-b"/><item id="foreign-b" href="foreign-b.bin" media-type="application/octet-stream" fallback="fallback"/><item id="fallback" href="fallback.xhtml" media-type="application/xhtml+xml"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#;
    let package = base_package(
        manifest,
        r#"<itemref idref="foreign-a"/><itemref idref="body"/>"#,
        basic_metadata(),
    );
    run_fallback_case(
        "foreign-spine-two-level-fallback",
        package,
        vec![
            ("foreign-a.bin", foreign.clone()),
            ("foreign-b.bin", foreign.clone()),
            ("fallback.xhtml", fallback.clone()),
            ("body.xhtml", body.clone()),
        ],
        true,
        false,
        2,
    );

    let manifest = r#"<item id="foreign" href="foreign.bin" media-type="application/vnd.example.foreign" fallback="missing.xhtml"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#;
    let package = base_package(
        manifest,
        r#"<itemref idref="foreign"/><itemref idref="body"/>"#,
        basic_metadata(),
    );
    assert_invalid_fallback_case(
        "foreign-spine-missing-fallback-target",
        package,
        vec![
            ("foreign.bin", foreign.clone()),
            ("body.xhtml", body.clone()),
        ],
        "missing target",
    );

    let manifest = r#"<item id="foreign-a" href="foreign-a.bin" media-type="application/vnd.example.foreign" fallback="foreign-b"/><item id="foreign-b" href="foreign-b.bin" media-type="application/octet-stream" fallback="foreign-a"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#;
    let package = base_package(
        manifest,
        r#"<itemref idref="foreign-a"/><itemref idref="body"/>"#,
        basic_metadata(),
    );
    assert_invalid_fallback_case(
        "foreign-spine-fallback-cycle",
        package,
        vec![
            ("foreign-a.bin", foreign.clone()),
            ("foreign-b.bin", foreign.clone()),
            ("body.xhtml", body.clone()),
        ],
        "contains a cycle",
    );

    let manifest = r#"<item id="foreign" href="foreign.bin" media-type="image/svg+xml" fallback="fallback"/><item id="fallback" href="fallback.xhtml" media-type="application/xhtml+xml"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#;
    let package = base_package(
        manifest,
        r#"<itemref idref="foreign"/><itemref idref="body"/>"#,
        basic_metadata(),
    );
    run_fallback_case(
        "foreign-svg-non-spine-fallback",
        package,
        vec![
            ("foreign.bin", b"<svg>FOREIGN_MARKER</svg>".to_vec()),
            ("fallback.xhtml", fallback),
            ("body.xhtml", body),
        ],
        false,
        true,
        2,
    );
}

fn run_remote_case(
    fixture: &str,
    href: &str,
    media_type: &str,
    properties: &str,
    body_extra: &str,
    files: Vec<(&str, Vec<u8>)>,
    expect_success: bool,
) {
    let remote_item = format!(
        r#"<item id="remote" href="{href}" media-type="{media_type}" properties="{properties}"/>"#
    );
    let manifest = format!(
        r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>{remote_item}"#
    );
    let package = base_package(&manifest, r#"<itemref idref="body"/>"#, basic_metadata());
    let mut all_files = vec![("body.xhtml", basic_body(BASELINE_MARKER, body_extra))];
    all_files.extend(files);
    let result = conversion_observation(
        "G2-15,G2-16",
        fixture,
        convert_result(&zip_epub(&package, &all_files, None)),
        Some(BASELINE_MARKER),
    );
    match (expect_success, result) {
        (true, Ok(azw3)) => {
            let reconstructed = reconstructed_text(&azw3);
            assert!(reconstructed.contains(BASELINE_MARKER));
            if fixture.contains("css-referenced-from-xhtml")
                || fixture.contains("image-referenced-from-xhtml")
            {
                assert!(
                    reconstructed.contains(href),
                    "external resource reference was unexpectedly rewritten or dropped for {fixture}"
                );
            }
            if fixture.contains("font-referenced-from-css") {
                let rawml = azw3.rawml();
                assert!(
                    String::from_utf8_lossy(&rawml).contains(href),
                    "external CSS font reference was unexpectedly rewritten or dropped for {fixture}"
                );
            }
        }
        (true, Err(error)) => panic!("{fixture} unexpectedly failed: {error}"),
        (false, Err(error)) => panic!("{fixture} unexpectedly failed: {error}"),
        (false, Ok(azw3)) => assert!(reconstructed_text(&azw3).contains(BASELINE_MARKER)),
    }
}

#[test]
fn remote_resource_boundary_remains_diagnostic() {
    run_remote_case(
        "remote-css-manifest-only-https",
        "https://example.invalid/remote.css",
        "text/css",
        "remote-resources",
        "",
        Vec::new(),
        true,
    );
    run_remote_case(
        "remote-image-manifest-only-http",
        "http://example.invalid/remote.png",
        "image/png",
        "remote-resources",
        "",
        Vec::new(),
        true,
    );
    run_remote_case(
        "remote-font-manifest-only-scheme-relative",
        "//example.invalid/remote.ttf",
        "font/ttf",
        "remote-resources",
        "",
        Vec::new(),
        true,
    );
    run_remote_case(
        "remote-css-referenced-from-xhtml",
        "https://example.invalid/remote.css",
        "text/css",
        "remote-resources",
        r#"<link rel="stylesheet" href="https://example.invalid/remote.css"/>"#,
        Vec::new(),
        true,
    );
    run_remote_case(
        "remote-image-referenced-from-xhtml",
        "https://example.invalid/remote.png",
        "image/png",
        "remote-resources",
        r#"<img src="https://example.invalid/remote.png"/>"#,
        Vec::new(),
        true,
    );
    run_remote_case(
        "remote-font-referenced-from-css",
        "https://example.invalid/remote.ttf",
        "font/ttf",
        "remote-resources",
        r#"<style>@font-face { font-family: Remote; src: url("https://example.invalid/remote.ttf"); }</style>"#,
        Vec::new(),
        true,
    );
    run_remote_case(
        "remote-resources-property-local-only",
        "local.css",
        "text/css",
        "remote-resources",
        r#"<link rel="stylesheet" href="local.css"/>"#,
        vec![("local.css", b"body { color: red; }".to_vec())],
        true,
    );
    run_remote_case(
        "remote-resources-property-local-resources",
        "local.css",
        "text/css",
        "remote-resources",
        r#"<link rel="stylesheet" href="local.css"/><img src="local.png"/>"#,
        vec![
            ("local.css", b"body { color: red; }".to_vec()),
            ("local.png", vec![0x89, b'P', b'N', b'G']),
        ],
        true,
    );
}

fn bytes_contain(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
