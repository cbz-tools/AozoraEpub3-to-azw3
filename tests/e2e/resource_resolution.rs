//! Primary audit coverage: G1-05, G2-06, G2-24, G8-01..G8-05.

use crate::support::{Azw3, zip_epub};
use aozoraepub3_to_azw3::{ConvertOptions, convert_bytes};

const BODY_MARKER: &str = "ROUND7B_BODY_MARKER";

fn package(metadata: &str, manifest: &str, spine: &str) -> String {
    format!(
        r#"<package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/">{metadata}</metadata><manifest>{manifest}</manifest><spine>{spine}</spine></package>"#
    )
}

fn metadata() -> &'static str {
    r#"<dc:title>Round7-B Title</dc:title><dc:creator>Round7-B Author</dc:creator><dc:language>en</dc:language>"#
}

fn body(extra: &str) -> Vec<u8> {
    format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>{BODY_MARKER}</p>{extra}</body></html>"#
    )
    .into_bytes()
}

fn rawml(azw3: &Azw3) -> String {
    String::from_utf8_lossy(&azw3.rawml()).into_owned()
}

fn convert_result(epub: &[u8]) -> Result<Azw3, String> {
    convert_bytes(epub, &ConvertOptions::default())
        .map(Azw3::parse)
        .map_err(|error| error.to_string())
}

// E2E-ID: E2E-OCF-04
// Audit: G1-05
#[test]
fn ocf_href_matrix_resolves_local_zip_names() {
    // Audit coverage: G1-05.
    let manifest = r#"
        <item id="body" href="OPS/chapters/body.xhtml" media-type="application/xhtml+xml"/>
        <item id="relative" href="OPS/chapters/relative.css" media-type="text/css"/>
        <item id="dot" href="OPS/chapters/dot.css" media-type="text/css"/>
        <item id="parent" href="OPS/styles/parent.css" media-type="text/css"/>
        <item id="fragment" href="OPS/styles/fragment.css" media-type="text/css"/>
        <item id="space" href="OPS/styles/space%20name.css" media-type="text/css"/>
        <item id="encoded-utf8" href="OPS/styles/%E6%97%A5%E6%9C%AC%E8%AA%9E.css" media-type="text/css"/>
        <item id="unicode" href="OPS/styles/ユニコード.css" media-type="text/css"/>
        <item id="nested" href="OPS/styles/nested/deep.css" media-type="text/css"/>
        <item id="literal-percent20" href="OPS/styles/literal-%2520.css" media-type="text/css"/>
        <item id="case" href="OPS/styles/Case.css" media-type="text/css"/>
        <item id="case-ref" href="OPS/styles/case-ref.css" media-type="text/css"/>
    "#;
    let spine = r#"<itemref idref="body"/>"#;
    let hrefs = r#"
        <link rel="stylesheet" href="relative.css"/>
        <link rel="stylesheet" href="./dot.css"/>
        <link rel="stylesheet" href="../styles/parent.css"/>
        <link rel="stylesheet" href="../styles/fragment.css?query=1#fragment"/>
        <link rel="stylesheet" href="../styles/space%20name.css"/>
        <link rel="stylesheet" href="../styles/%E6%97%A5%E6%9C%AC%E8%AA%9E.css"/>
        <link rel="stylesheet" href="../styles/ユニコード.css"/>
        <link rel="stylesheet" href="../styles/nested/../nested/deep.css"/>
        <link rel="stylesheet" href="../styles/literal-%2520.css"/>
        <link rel="stylesheet" href="../styles/case-ref.css"/>
    "#;
    let css = |marker: &str| format!("body {{ color: red; }} /* {marker} */").into_bytes();
    let epub = zip_epub(
        &package(metadata(), manifest, spine),
        &[
            ("OPS/chapters/body.xhtml", body(hrefs)),
            ("OPS/chapters/relative.css", css("G1_RELATIVE_MARKER")),
            ("OPS/chapters/dot.css", css("G1_DOT_MARKER")),
            ("OPS/styles/parent.css", css("G1_PARENT_MARKER")),
            ("OPS/styles/fragment.css", css("G1_FRAGMENT_MARKER")),
            ("OPS/styles/space name.css", css("G1_SPACE_MARKER")),
            ("OPS/styles/日本語.css", css("G1_ENCODED_UTF8_MARKER")),
            ("OPS/styles/ユニコード.css", css("G1_UNICODE_MARKER")),
            ("OPS/styles/nested/deep.css", css("G1_NESTED_MARKER")),
            (
                "OPS/styles/literal-%20.css",
                css("G1_LITERAL_PERCENT20_MARKER"),
            ),
            (
                "OPS/styles/literal- .css",
                css("G1_DECODED_SPACE_COLLISION_MARKER"),
            ),
            ("OPS/styles/Case.css", css("G1_CASE_MISMATCH_MARKER")),
            (
                "OPS/styles/case-ref.css",
                b"@import \"CASE.css\"; body { color: blue; }".to_vec(),
            ),
        ],
        None,
    );
    let result = convert_result(&epub);
    println!(
        "ROUND7B_OBSERVATION|id=G1-05|result={}",
        result
            .as_ref()
            .map(|_| "success")
            .unwrap_or("explicit-error")
    );
    let azw3 = result.expect("G1-05 local href matrix must convert");
    let output = rawml(&azw3);
    assert!(output.contains(BODY_MARKER));
    for marker in [
        "G1_RELATIVE_MARKER",
        "G1_DOT_MARKER",
        "G1_PARENT_MARKER",
        "G1_FRAGMENT_MARKER",
        "G1_SPACE_MARKER",
        "G1_ENCODED_UTF8_MARKER",
        "G1_UNICODE_MARKER",
        "G1_NESTED_MARKER",
        "G1_LITERAL_PERCENT20_MARKER",
    ] {
        assert!(
            output.contains(marker),
            "resolved CSS marker missing: {marker}"
        );
    }
    assert!(!output.contains("G1_DECODED_SPACE_COLLISION_MARKER"));
    assert!(output.contains("CASE.css"));
    assert!(!output.contains("G1_CASE_MISMATCH_MARKER"));
    println!(
        "ROUND7B_SAFE_PROJECTION|id=G1-05|resolved=relative-dot-parent-fragment-query-space-percent20-percent2520-literal-percent20-collision-percent-utf8-unicode-nested|case-sensitive=true|normalized-path-not-decoded-twice=true"
    );
}

// E2E-ID: E2E-META-01
// Audit: G2-06
#[test]
fn dublin_core_projection_and_safe_omission() {
    // Audit coverage: G2-06.
    let metadata = format!(
        "{}<dc:publisher>  Publisher Value  </dc:publisher><dc:description> Description Value </dc:description><dc:contributor>  Contributor Value  </dc:contributor><dc:coverage>Optional Coverage</dc:coverage><dc:date>Optional Date</dc:date><dc:format>Optional Format</dc:format><dc:relation>Optional Relation</dc:relation><dc:rights>Optional Rights</dc:rights><dc:source>Optional Source</dc:source><dc:subject>Optional Subject</dc:subject><dc:type>Optional Type</dc:type>",
        metadata()
    );
    let package = package(
        &metadata,
        r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#,
        r#"<itemref idref="body"/>"#,
    );
    let epub = zip_epub(
        &package,
        &[("body.xhtml", body("<p>READING_ORDER_MARKER</p>"))],
        None,
    );
    let result = convert_result(&epub);
    println!(
        "ROUND7B_OBSERVATION|id=G2-06|result={}",
        result
            .as_ref()
            .map(|_| "success")
            .unwrap_or("explicit-error")
    );
    let azw3 = result.expect("G2-06 metadata matrix must convert");
    assert_eq!(azw3.exth().text(101).as_deref(), Some("Publisher Value"));
    assert_eq!(azw3.exth().text(103).as_deref(), Some("Description Value"));
    assert_eq!(azw3.exth().text(108).as_deref(), Some("Contributor Value"));
    let sections = azw3.reconstructed_xhtml_sections();
    let sections = String::from_utf8_lossy(&sections.concat()).into_owned();
    assert!(sections.contains(BODY_MARKER));
    assert!(sections.contains("READING_ORDER_MARKER"));
    let output = rawml(&azw3);
    let omitted_values = [
        "Optional Coverage",
        "Optional Date",
        "Optional Format",
        "Optional Relation",
        "Optional Rights",
        "Optional Source",
        "Optional Subject",
        "Optional Type",
    ];
    for marker in omitted_values {
        assert!(
            !output.contains(marker),
            "unprojected metadata leaked: {marker}"
        );
        assert!(
            azw3.bytes
                .windows(marker.len())
                .all(|window| window != marker.as_bytes()),
            "unprojected metadata reached AZW3 bytes: {marker}"
        );
    }
    println!(
        "ROUND7B_SAFE_OMISSION|id=G2-06|projected=publisher-101-description-103-contributor-108|omitted=coverage-date-format-relation-rights-source-subject-type|whitespace=trimmed"
    );
}

// E2E-ID: E2E-SCRIPT-01
// Audit: G2-24, G3-27, G8-01, G8-02, G8-03, G8-05
#[test]
fn script_and_form_boundary_is_explicit() {
    // Audit coverage: G2-24, G8-01..G8-03, G8-05.
    let cases = [
        (
            "marker-only",
            "scripted",
            "<p>MARKER_ONLY_SAFE</p>",
            true,
            None,
        ),
        (
            "inline-script",
            "scripted",
            "<script>INLINE_SCRIPT_MARKER</script>",
            false,
            Some("inline script"),
        ),
        (
            "inline-script-missing-property",
            "",
            "<script>INLINE_SCRIPT_MISSING_PROPERTY_MARKER</script>",
            false,
            Some("inline script"),
        ),
        (
            "external-script",
            "scripted",
            "<script src=\"https://example.invalid/app.js\"></script>",
            false,
            Some("external script"),
        ),
        (
            "form-missing-property",
            "",
            "<form><input name=\"q\"/></form>",
            false,
            Some("HTML form"),
        ),
        (
            "form-with-property",
            "scripted",
            "<form><input name=\"q\"/></form>",
            false,
            Some("HTML form"),
        ),
        (
            "json-data-block",
            "scripted",
            "<script type=\"application/ld+json\">{\"@context\":\"https://schema.org\"}</script><p>DATA_BLOCK_SAFE</p>",
            true,
            None,
        ),
        (
            // Keep the malformed raw scanner sequence inside CDATA so the
            // enclosing XHTML remains parseable and reaches the boundary check.
            "malformed-cross-boundary-style-script",
            "scripted",
            "<style><![CDATA[<script type=\"application/ld+json\"></style><style></script><form><input/></form></style>]]></style>",
            false,
            Some("HTML form"),
        ),
    ];
    for (fixture, properties, extra, succeeds, expected_error) in cases {
        let manifest = format!(
            r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml" properties="{properties}"/>"#
        );
        let package = package(metadata(), &manifest, r#"<itemref idref="body"/>"#);
        let epub = zip_epub(&package, &[("body.xhtml", body(extra))], None);
        let result = convert_result(&epub);
        println!(
            "ROUND7B_OBSERVATION|id=G2-24/G8|fixture={fixture}|result={}",
            result
                .as_ref()
                .map(|_| "success")
                .unwrap_or("explicit-error")
        );
        match (succeeds, result, expected_error) {
            (true, Ok(azw3), None) => {
                assert!(rawml(&azw3).contains(BODY_MARKER));
            }
            (false, Err(error), Some(expected)) => {
                assert!(error.contains("G2-24/G8"), "feature tag missing: {error}");
                assert!(
                    error.contains(expected),
                    "specific boundary missing: {error}"
                );
            }
            (true, result, _) => panic!("{fixture} unexpectedly failed: {result:?}"),
            (false, result, _) => panic!("{fixture} unexpectedly converted: {result:?}"),
        }
    }
    println!(
        "ROUND7B_SAFE_BOUNDARY|id=G2-24/G8|marker-only-and-json-data-block=accepted|inline-external-script-and-form=explicit-unsupported"
    );
}

// E2E-ID: E2E-SCRIPT-02
// Audit: G2-24, G3-05, G8-01, G8-02, G8-03, G8-05
#[test]
fn direct_svg_script_is_safely_rejected() {
    // Audit coverage: G3-05, G8-01..G8-03, G8-05.
    let cases = [
        (
            "svg-inline-script",
            r#"<svg xmlns="http://www.w3.org/2000/svg"><script>SVG_INLINE_SCRIPT_MARKER</script></svg>"#,
            "inline script",
        ),
        (
            "svg-href-script",
            r#"<svg xmlns="http://www.w3.org/2000/svg"><script href="app.js"/></svg>"#,
            "external script",
        ),
        (
            "svg-xlink-href-script",
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"><script xlink:href="app.js"/></svg>"#,
            "external script",
        ),
        (
            "svg-form",
            r#"<svg xmlns="http://www.w3.org/2000/svg"><foreignObject><form><input name="q"/></form></foreignObject></svg>"#,
            "HTML form",
        ),
    ];
    for (fixture, svg, expected_error) in cases {
        let package = package(
            metadata(),
            r#"<item id="svg" href="diagram.svg" media-type="image/svg+xml"/>"#,
            r#"<itemref idref="svg"/>"#,
        );
        let epub = zip_epub(&package, &[("diagram.svg", svg.as_bytes().to_vec())], None);
        let error = convert_result(&epub).expect_err("scripted SVG must not convert");
        assert!(error.contains("G2-24/G8"), "feature tag missing: {error}");
        assert!(
            error.contains(expected_error),
            "specific SVG boundary missing for {fixture}: {error}"
        );
        println!(
            "ROUND7B_SAFE_BOUNDARY|id=G8-01/G8-02/G8-05|fixture={fixture}|direct-svg=explicitly-rejected-before-wrapper"
        );
    }
}

// E2E-ID: E2E-SCRIPT-03
// Audit: G2-24, G8-04
#[test]
fn script_fallback_is_safely_rejected() {
    // Audit coverage: G8-04.
    let manifest = r#"
        <item id="foreign" href="foreign.bin" media-type="application/vnd.example.foreign" fallback="scripted"/>
        <item id="scripted" href="scripted.xhtml" media-type="application/xhtml+xml"/>
    "#;
    let package = package(metadata(), manifest, r#"<itemref idref="foreign"/>"#);
    let epub = zip_epub(
        &package,
        &[
            ("foreign.bin", b"FOREIGN_SCRIPT_FALLBACK".to_vec()),
            (
                "scripted.xhtml",
                body("<script>FALLBACK_SCRIPT_MARKER</script>").to_vec(),
            ),
        ],
        None,
    );
    let error = convert_result(&epub).expect_err("script fallback must not convert");
    assert!(error.contains("G2-24/G8"), "feature tag missing: {error}");
    assert!(error.contains("inline script"), "boundary missing: {error}");
    println!("ROUND7B_SAFE_BOUNDARY|id=G8-04|scripted-fallback=explicitly-rejected-before-AZW3");
}
