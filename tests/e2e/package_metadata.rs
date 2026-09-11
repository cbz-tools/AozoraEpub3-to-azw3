//! Primary audit coverage: A-02 and G2-07..G2-14, G5-10, G5-16.

use crate::support;
use crate::support::Azw3;
use aozoraepub3_to_azw3::{ConvertOptions, convert_bytes};

fn convert(epub: Vec<u8>) -> Azw3 {
    Azw3::parse(
        convert_bytes(&epub, &ConvertOptions::default())
            .unwrap_or_else(|error| panic!("EPUB 3.3 P0 fixture failed: {error}")),
    )
}

fn convert_error(epub: Vec<u8>) -> String {
    convert_bytes(&epub, &ConvertOptions::default())
        .expect_err("EPUB 3.3 malformed P0 fixture unexpectedly converted")
        .to_string()
}

fn body(marker: &str, extra: &str) -> Vec<u8> {
    format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>{marker}</p>{extra}</body></html>"#
    )
    .into_bytes()
}

fn package(metadata: &str, manifest: &str, spine: &str) -> String {
    format!(
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/">{metadata}</metadata><manifest>{manifest}</manifest><spine>{spine}</spine></package>"#
    )
}

// E2E-ID: E2E-META-02
// Audit: G2-07, G2-08, G2-09, G2-10, G2-11; A-02
#[test]
fn metadata_refinements_project_without_collection_leakage() {
    // Audit coverage: A-02; G2-07..G2-11. Direct EXTH/body assertions prove
    // recognized refinements and collection isolation.
    let metadata = r##"
        <dc:title id="title-main">Main Title</dc:title>
        <dc:title id="title-subtitle">Subtitle Title</dc:title>
        <meta refines="#title-main" property="title-type">main</meta>
        <meta refines="#title-main" property="file-as">Main, Title</meta>
        <dc:creator id="creator-author">Alice Author</dc:creator>
        <meta refines="#creator-author" property="role" scheme="marc:relators">aut</meta>
        <meta refines="#creator-author" property="file-as">Author, Alice</meta>
        <dc:creator id="creator-editor">Bob Editor</dc:creator>
        <meta refines="#creator-editor" property="role" scheme="marc:relators">edt</meta>
        <meta refines="#creator-editor" property="file-as">Editor, Bob</meta>
        <dc:publisher id="publisher">Press Name</dc:publisher>
        <meta refines="#publisher" property="file-as">Name, Press</meta>
        <dc:title id="collection-title">Series Title</dc:title>
        <meta refines="#collection-title" property="belongs-to-collection">Series Name</meta>
        <meta refines="#collection-title" property="collection-type">series</meta>
        <meta refines="#collection-title" property="group-position">2</meta>
        <dc:language>en</dc:language>
    "##;
    let manifest = r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#;
    let epub = support::zip_epub(
        &package(metadata, manifest, r#"<itemref idref="body"/>"#),
        &[("body.xhtml", body("METADATA_BODY_MARKER", ""))],
        None,
    );
    let azw3 = convert(epub);
    let exth = azw3.exth();
    assert_eq!(exth.text(503).as_deref(), Some("Main Title"));
    assert_eq!(exth.text(508).as_deref(), Some("Main, Title"));
    assert_eq!(exth.text(100).as_deref(), Some("Alice Author"));
    assert_eq!(exth.text(517).as_deref(), Some("Author, Alice"));
    assert_eq!(exth.text(101).as_deref(), Some("Press Name"));
    assert_eq!(exth.text(522).as_deref(), Some("Name, Press"));
    let contributors = exth
        .values
        .iter()
        .filter(|(kind, _)| *kind == 108)
        .map(|(_, value)| String::from_utf8_lossy(value).into_owned())
        .collect::<Vec<_>>();
    assert_eq!(contributors, vec!["Bob Editor"]);
    assert!(
        exth.values
            .iter()
            .all(|(_, value)| String::from_utf8_lossy(value) != "Series Name")
    );
    assert!(azw3.reconstructed_xhtml_sections().iter().any(|section| {
        section
            .windows(b"METADATA_BODY_MARKER".len())
            .any(|window| window == b"METADATA_BODY_MARKER")
    }));
}

// E2E-ID: E2E-META-03
// Audit: G2-13, G2-14
#[test]
fn fallback_chain_projects_content_and_rejects_malformed_chains() {
    // Audit coverage: G2-13..G2-14. Direct success, omission, missing-target,
    // cycle, and unsupported-terminal assertions cover the fallback boundary.
    let metadata = r#"<dc:title>Fallback P0</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language>"#;
    let manifest = r#"<item id="foreign-a" href="foreign-a.bin" media-type="application/vnd.example.foreign" fallback="foreign-b"/><item id="foreign-b" href="foreign-b.bin" media-type="application/octet-stream" fallback="fallback"/><item id="fallback" href="fallback.xhtml" media-type="application/xhtml+xml"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#;
    let epub = support::zip_epub(
        &package(
            metadata,
            manifest,
            r#"<itemref idref="foreign-a"/><itemref idref="body"/>"#,
        ),
        &[
            ("foreign-a.bin", b"FOREIGN_BYTES".to_vec()),
            ("foreign-b.bin", b"FOREIGN_BYTES".to_vec()),
            ("fallback.xhtml", body("FALLBACK_CHAIN_MARKER", "")),
            ("body.xhtml", body("FALLBACK_NEIGHBOR_MARKER", "")),
        ],
        None,
    );
    let azw3 = convert(epub);
    let sections = azw3.reconstructed_xhtml_sections();
    let text = sections
        .iter()
        .map(|section| String::from_utf8_lossy(section))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("FALLBACK_CHAIN_MARKER"));
    assert!(text.contains("FALLBACK_NEIGHBOR_MARKER"));
    assert!(!text.contains("FOREIGN_BYTES"));
    assert_eq!(sections.len(), 2);

    let missing = r#"<item id="foreign" href="foreign.bin" media-type="application/vnd.example.foreign" fallback="missing"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#;
    let error = convert_error(support::zip_epub(
        &package(
            metadata,
            missing,
            r#"<itemref idref="foreign"/><itemref idref="body"/>"#,
        ),
        &[
            ("foreign.bin", b"FOREIGN_BYTES".to_vec()),
            ("body.xhtml", body("BODY", "")),
        ],
        None,
    ));
    assert!(error.contains("fallback chain") && error.contains("missing target"));

    let cycle = r#"<item id="foreign-a" href="foreign-a.bin" media-type="application/vnd.example.foreign" fallback="foreign-b"/><item id="foreign-b" href="foreign-b.bin" media-type="application/octet-stream" fallback="foreign-a"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#;
    let error = convert_error(support::zip_epub(
        &package(
            metadata,
            cycle,
            r#"<itemref idref="foreign-a"/><itemref idref="body"/>"#,
        ),
        &[
            ("foreign-a.bin", b"A".to_vec()),
            ("foreign-b.bin", b"B".to_vec()),
            ("body.xhtml", body("BODY", "")),
        ],
        None,
    ));
    assert!(error.contains("fallback chain") && error.contains("cycle"));

    let unsupported_terminal = r#"<item id="foreign" href="foreign.bin" media-type="application/vnd.example.foreign" fallback="unsupported"/><item id="unsupported" href="unsupported.bin" media-type="application/octet-stream"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#;
    let error = convert_error(support::zip_epub(
        &package(
            metadata,
            unsupported_terminal,
            r#"<itemref idref="foreign"/><itemref idref="body"/>"#,
        ),
        &[
            ("foreign.bin", b"FOREIGN_BYTES".to_vec()),
            ("unsupported.bin", b"UNSUPPORTED_BYTES".to_vec()),
            ("body.xhtml", body("BODY", "")),
        ],
        None,
    ));
    assert!(error.contains("fallback chain") && error.contains("unsupported media type"));

    let non_spine = r#"<item id="foreign" href="foreign.bin" media-type="application/vnd.example.foreign" fallback="fallback"/><item id="fallback" href="fallback.xhtml" media-type="application/xhtml+xml"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#;
    let azw3 = convert(support::zip_epub(
        &package(metadata, non_spine, r#"<itemref idref="body"/>"#),
        &[
            ("foreign.bin", b"FOREIGN_BYTES".to_vec()),
            ("fallback.xhtml", body("NON_SPINE_FALLBACK", "")),
            ("body.xhtml", body("NON_SPINE_BODY", "")),
        ],
        None,
    ));
    let text = azw3
        .reconstructed_xhtml_sections()
        .into_iter()
        .map(|section| String::from_utf8_lossy(&section).into_owned())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("NON_SPINE_BODY"));
    assert!(!text.contains("NON_SPINE_FALLBACK"));
    assert_eq!(azw3.reconstructed_xhtml_sections().len(), 1);
}

// E2E-ID: E2E-META-05
// Audit: G3-05
#[test]
fn svg_content_document_is_lowered_into_kf8_flow() {
    // Audit coverage: G3-05. Direct reconstructed-flow markers prove safe
    // SVG spine content is lowered alongside ordinary XHTML content.
    let metadata = r#"<dc:title>SVG P0</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language>"#;
    let manifest = r#"<item id="svg" href="diagram.svg" media-type="image/svg+xml"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#;
    let epub = support::zip_epub(
        &package(metadata, manifest, r#"<itemref idref="svg"/><itemref idref="body"/>"#),
        &[
            ("diagram.svg", br#"<svg xmlns="http://www.w3.org/2000/svg"><title>SVG_SPINE_P0_MARKER</title><rect width="10" height="10"/></svg>"#.to_vec()),
            ("body.xhtml", body("SVG_BODY_MARKER", "")),
        ],
        None,
    );
    let azw3 = convert(epub);
    let text = azw3
        .reconstructed_xhtml_sections()
        .into_iter()
        .map(|section| String::from_utf8_lossy(&section).into_owned())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("SVG_SPINE_P0_MARKER"));
    assert!(text.contains("SVG_BODY_MARKER"));
}

// E2E-ID: E2E-META-04
// Audit: G5-09, G5-10, G5-12, G5-16
#[test]
fn navigation_channels_and_page_targets_are_separate() {
    // Audit coverage: G5-09, G5-10, G5-12, G5-16. Direct page-list, NCX,
    // landmark, custom-channel omission, and position-link assertions.
    let metadata = r#"<dc:title>Navigation P0</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language>"#;
    let manifest = r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/>"#;
    let package = package(metadata, manifest, r#"<itemref idref="body"/>"#)
        .replace("<spine>", "<spine toc=\"ncx\">");
    let epub = support::zip_epub(
        &package,
        &[
            ("body.xhtml", body("NAV_BODY_MARKER", r#"<span epub:type="pagebreak" id="page-1">1</span>"#)),
            ("nav.xhtml", br#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="body.xhtml">TOC_CHANNEL_MARKER</a></li></ol></nav><nav epub:type="page-list"><ol><li><a href="body.xhtml#page-1">PAGE_LIST_CHANNEL_MARKER</a></li></ol></nav><nav epub:type="landmarks"><ol><li><a epub:type="bodymatter" href="body.xhtml">LANDMARK_CHANNEL_MARKER</a></li></ol></nav><nav epub:type="custom"><ol><li><a href="body.xhtml">CUSTOM_CHANNEL_MARKER</a></li></ol></nav></body></html>"#.to_vec()),
            ("toc.ncx", br#"<?xml version="1.0"?><ncx><navMap><navPoint><navLabel><text>NCX_CHANNEL_MARKER</text></navLabel><content src="body.xhtml"/></navPoint></navMap></ncx>"#.to_vec()),
        ],
        None,
    );
    let azw3 = convert(epub);
    let sections = azw3.reconstructed_xhtml_sections();
    let text = sections
        .iter()
        .map(|section| String::from_utf8_lossy(section))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(text.contains("PAGE_LIST_CHANNEL_MARKER"));
    assert!(text.contains("NAV_BODY_MARKER"));
    assert!(text.contains("pagebreak"));
    let page_list_section = sections
        .iter()
        .map(|section| String::from_utf8_lossy(section).into_owned())
        .find(|section| section.contains("PAGE_LIST_CHANNEL_MARKER"))
        .expect("page-list projection section");
    assert!(page_list_section.contains("kindle:pos:fid:"));
    assert!(String::from_utf8_lossy(&azw3.bytes).contains("NCX_CHANNEL_MARKER"));
    assert!(!text.contains("CUSTOM_CHANNEL_MARKER"));
    assert!(azw3.index_report(azw3.mobi().ncx).entry_count >= 1);
    assert!(!azw3.guide_targets().is_empty());
}
