//! Primary audit coverage: G7-03 package-local inline-style URL projection.

// E2E-ID: E2E-CSS-04
// Audit: G7-03, G7-15

use crate::support::{Azw3, zip_epub};
use aozoraepub3_to_azw3::{ConvertOptions, convert_bytes};

const BASELINE_MARKER: &str = "EPUB33_P0_ROUND5_CSS_BASELINE";

fn package() -> &'static str {
    r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>EPUB 3.3 CSS Round5</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language></metadata><manifest><item id="style" href="style.css" media-type="text/css"/><item id="body" href="text/body.xhtml" media-type="application/xhtml+xml"/><item id="bg" href="images/bg.png" media-type="image/png"/><item id="font" href="fonts/font.ttf" media-type="font/ttf"/></manifest><spine><itemref idref="body"/></spine></package>"#
}

fn epub(inline_style: &str) -> Vec<u8> {
    let body = format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><link rel="stylesheet" href="../style.css"/></head><body><p style="{inline_style}">{BASELINE_MARKER}</p></body></html>"#
    );
    zip_epub(
        package(),
        &[
            ("style.css", b".base { color: black; }".to_vec()),
            ("text/body.xhtml", body.into_bytes()),
            ("images/bg.png", b"ROUND5_IMAGE_RESOURCE".to_vec()),
            ("fonts/font.ttf", b"ROUND5_FONT_RESOURCE".to_vec()),
        ],
        None,
    )
}

fn convert(epub: &[u8]) -> Azw3 {
    Azw3::parse(
        convert_bytes(epub, &ConvertOptions::default())
            .expect("Round5 inline CSS fixture should convert"),
    )
}

fn sections(azw3: &Azw3) -> String {
    azw3.reconstructed_xhtml_sections()
        .into_iter()
        .map(|section| String::from_utf8_lossy(&section).into_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn inline_local_urls_project_to_kindle_embeds() {
    // Audit coverage: G7-03, G7-15. Direct CSS/RawML assertions cover local
    // image/font URL projection into reachable kindle:embed references.
    for style in [
        "background-image:url('../images/bg.png')",
        "background-image:url(../images/bg.png)",
        "--font-source:url('../fonts/font.ttf')",
    ] {
        let azw3 = convert(&epub(style));
        let sections = sections(&azw3);
        assert!(sections.contains(BASELINE_MARKER));
        assert!(
            sections.contains("kindle:embed:"),
            "inline style URL was not rewritten: {sections}"
        );
        assert!(
            !sections.contains("../images/bg.png") && !sections.contains("../fonts/font.ttf"),
            "source-relative package URL remained after rewrite: {sections}"
        );
        assert_eq!(
            azw3.image_records().len(),
            1,
            "image resource was not serialized"
        );
    }
}

#[test]
fn inline_external_data_and_literal_urls_remain_unchanged() {
    // Audit coverage: G7-03, G7-15, G7-16. Direct CSS assertions cover the
    // non-package URL preservation boundary.
    for style in [
        "background-image:url('https://example.com/a.png')",
        "background-image:url(data:image/png;base64,AAAA)",
        "--literal:'url(foo.png)'; /* url('../images/bg.png') */",
    ] {
        let azw3 = convert(&epub(style));
        let sections = sections(&azw3);
        assert!(sections.contains(BASELINE_MARKER));
        assert!(
            !sections.contains("kindle:embed:"),
            "external/data/literal URL was rewritten unexpectedly: {sections}"
        );
    }
}
