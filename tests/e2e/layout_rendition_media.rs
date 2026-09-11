//! Primary audit coverage: G2-25, G3-07, G6-14.

use crate::support::{Azw3, zip_epub};
use aozoraepub3_to_azw3::{ConvertOptions, convert_bytes};

const BASELINE_MARKER: &str = "EPUB33_P0_ROUND3_BASELINE";

fn base_package(manifest: &str, spine: &str, metadata: &str) -> String {
    format!(
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/ops" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>EPUB 3.3 P0 round three</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language>{metadata}</metadata><manifest>{manifest}</manifest><spine>{spine}</spine></package>"#
    )
}

fn convert_result(epub: &[u8]) -> Result<Azw3, String> {
    convert_bytes(epub, &ConvertOptions::default())
        .map(Azw3::parse)
        .map_err(|error| error.to_string())
}

fn reconstructed_sections(azw3: &Azw3) -> String {
    azw3.reconstructed_xhtml_sections()
        .into_iter()
        .map(|section| String::from_utf8_lossy(&section).into_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

fn xhtml(body: &str) -> Vec<u8> {
    format!(r#"<html xmlns="http://www.w3.org/1999/xhtml"><body>{body}</body></html>"#).into_bytes()
}

fn mathml_epub(manifest_properties: &str, body: &str) -> Vec<u8> {
    let package = base_package(
        &format!(
            r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"{manifest_properties}/><item id="fallback" href="fallback.png" media-type="image/png"/>"#
        ),
        r#"<itemref idref="body"/>"#,
        "",
    );
    zip_epub(
        &package,
        &[
            ("body.xhtml", xhtml(body)),
            ("fallback.png", b"MATHML_FALLBACK_IMAGE".to_vec()),
        ],
        None,
    )
}

// E2E-ID: E2E-FXL-07
// Audit: G2-25, G3-07
#[test]
fn mathml_property_content_and_fallback_are_safely_rejected() {
    // Audit coverage: G2-25. Direct error and no-fallback-output assertions
    // establish explicit MathML safe rejection.
    let mathml = r#"<p>FORMULA_BEGIN<math xmlns="http://www.w3.org/1998/Math/MathML"><mi>x</mi></math>FORMULA_END</p><img src="fallback.png" alt="Equation fallback"/>"#;
    for (properties, fixture) in [
        (r#" properties="mathml""#, "manifest-property-and-content"),
        ("", "content-without-manifest-property"),
    ] {
        let error = convert_result(&mathml_epub(properties, mathml))
            .expect_err("MathML must not be transported into KF8");
        assert!(error.contains("MathML"), "{fixture}: {error}");
        assert!(
            error.to_ascii_lowercase().contains("unsupported"),
            "{fixture}: {error}"
        );
    }

    let property_only = mathml_epub(r#" properties="mathml""#, "<p>PROPERTY_ONLY</p>");
    let property_only_error = convert_result(&property_only)
        .expect_err("a declared but unsupported MathML manifest property must reject");
    assert!(property_only_error.contains("MathML"));
    assert!(
        property_only_error
            .to_ascii_lowercase()
            .contains("unsupported")
    );
}

// E2E-ID: E2E-FXL-06
// Audit: G3-07
#[test]
fn mathml_namespace_detection_rejects_only_mathml_namespace() {
    // Audit coverage: G3-07. Direct namespace boundary assertions prevent
    // false-positive rejection of ordinary/custom XML namespaces.
    let prefixed = mathml_epub(
        "",
        r#"<p>PREFIXED_FORMULA_BEGIN<m:math xmlns:m="http://www.w3.org/1998/Math/MathML"><m:mi>x</m:mi></m:math>PREFIXED_FORMULA_END</p>"#,
    );
    let prefixed_error = convert_result(&prefixed).expect_err("prefixed MathML must reject");
    assert!(prefixed_error.contains("MathML"));
    assert!(prefixed_error.to_ascii_lowercase().contains("unsupported"));

    for body in [
        r#"<p>ORDINARY_MATH_LOCAL_NAME</p><math>not MathML</math>"#,
        r#"<p>CUSTOM_NAMESPACE_LOCAL_NAME</p><m:math xmlns:m="urn:example:custom-math"><m:mi>x</m:mi></m:math>"#,
    ] {
        let epub = mathml_epub("", body);
        let azw3 = convert_result(&epub).expect("non-MathML namespace must remain supported");
        let sections = reconstructed_sections(&azw3);
        assert!(
            sections.contains("ORDINARY_MATH_LOCAL_NAME")
                || sections.contains("CUSTOM_NAMESPACE_LOCAL_NAME")
        );
    }
}

fn fixed_page(viewport: Option<&str>, marker: &str) -> Vec<u8> {
    fixed_page_with_viewports(viewport.into_iter().collect::<Vec<_>>().as_slice(), marker)
}

fn fixed_page_with_viewports(viewports: &[&str], marker: &str) -> Vec<u8> {
    let viewport = viewports
        .iter()
        .map(|content| format!(r#"<meta name="viewport" content="{content}"/>"#))
        .collect::<String>();
    format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><head>{viewport}</head><body><p>{BASELINE_MARKER}</p><svg xmlns="http://www.w3.org/2000/svg"><text>{marker}</text></svg></body></html>"#
    )
    .into_bytes()
}

fn mixed_epub(pages: &[(&str, &str)], publication_metadata: &str) -> Vec<u8> {
    let manifest = pages
        .iter()
        .enumerate()
        .map(|(index, (href, _))| {
            format!(r#"<item id="page{index}" href="{href}" media-type="application/xhtml+xml"/>"#)
        })
        .collect::<String>();
    let spine = pages
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let layout = if index == 0 {
                r#" properties="rendition:layout-pre-paginated""#
            } else {
                ""
            };
            format!(r#"<itemref idref="page{index}"{layout}/>"#)
        })
        .collect::<String>();
    let package = base_package(&manifest, &spine, publication_metadata);
    let files = pages
        .iter()
        .map(|(href, viewport)| (*href, fixed_page(Some(*viewport), href)))
        .collect::<Vec<_>>();
    zip_epub(&package, &files, None)
}

fn fixed_epub(pages: &[(&str, Option<&str>)], original_resolution: Option<&str>) -> Vec<u8> {
    let metadata = format!(
        r#"<meta property="rendition:layout">pre-paginated</meta>{}"#,
        original_resolution
            .map(|value| format!(r#"<meta name="original-resolution" content="{value}"/>"#))
            .unwrap_or_default()
    );
    let manifest = pages
        .iter()
        .enumerate()
        .map(|(index, (href, _))| {
            format!(r#"<item id="page{index}" href="{href}" media-type="application/xhtml+xml"/>"#)
        })
        .collect::<String>();
    let spine = pages
        .iter()
        .enumerate()
        .map(|(index, _)| format!(r#"<itemref idref="page{index}"/>"#))
        .collect::<String>();
    let package = base_package(&manifest, &spine, &metadata);
    let files = pages
        .iter()
        .map(|(href, viewport)| (*href, fixed_page(*viewport, href)))
        .collect::<Vec<_>>();
    zip_epub(&package, &files, None)
}

// E2E-ID: E2E-FXL-03
// Audit: G6-14
#[test]
fn fixed_viewport_is_document_local_and_preserves_rawml() {
    // Audit coverage: G6-14. Fixed pages may vary independently; viewport
    // markup remains XHTML transport and never derives EXTH 126.
    let mixed = mixed_epub(
        &[
            ("fixed.xhtml", "width=1200,height=600"),
            ("reflow.xhtml", "width=600,height=1200"),
        ],
        "",
    );
    let mixed_azw3 = convert_result(&mixed).expect("mixed fixed-item viewport must convert");
    assert_eq!(mixed_azw3.exth().text(126), None);

    let varying = fixed_epub(
        &[
            ("page1.xhtml", Some("width=1200,height=600")),
            ("page2.xhtml", Some("height=900,width=600")),
        ],
        None,
    );
    let varying_azw3 = convert_result(&varying).expect("varying fixed-page viewports must convert");
    assert_eq!(varying_azw3.exth().text(126), None);
    let rawml_bytes = varying_azw3.rawml();
    let rawml = String::from_utf8_lossy(&rawml_bytes);
    assert!(rawml.contains("width=1200,height=600"));
    assert!(rawml.contains("height=900,width=600"));

    let explicit = fixed_epub(
        &[("body.xhtml", Some("width=1200,height=600"))],
        Some("1125x1600"),
    );
    let explicit_azw3 =
        convert_result(&explicit).expect("explicit original-resolution must remain supported");
    assert_eq!(explicit_azw3.exth().text(126).as_deref(), Some("1125x1600"));
}

// E2E-ID: E2E-FXL-04
// Audit: G6-14
#[test]
fn viewport_grammar_and_duplicate_declarations_are_supported() {
    // Audit coverage: G6-14. Separators, device dimensions, and repeated
    // document-local declarations remain transportable without EXTH 126.
    for (content, marker) in [
        ("width=1200; height=600", "semicolon"),
        ("width = 1200\t height = 600", "ascii-whitespace"),
        (
            "width=device-width,height=device-height",
            "device-dimensions",
        ),
    ] {
        let epub = fixed_epub(&[("body.xhtml", Some(content))], None);
        let azw3 = convert_result(&epub).expect("supported viewport grammar must convert");
        assert_eq!(
            azw3.exth().text(126),
            None,
            "{marker} must remain document-local"
        );
        let rawml_bytes = azw3.rawml();
        let rawml = String::from_utf8_lossy(&rawml_bytes);
        assert!(
            rawml.contains(content),
            "{marker} viewport must be transported"
        );
    }

    let package = base_package(
        r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#,
        r#"<itemref idref="body" properties="rendition:layout-pre-paginated"/>"#,
        r#"<meta property="rendition:layout">pre-paginated</meta>"#,
    );
    let body = format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><meta name="viewport" content="width=1200,height=600"/><meta name="viewport" content="height=900;width=600"/></head><body><p>{BASELINE_MARKER}</p><p>DUPLICATE_VIEWPORT</p><svg xmlns="http://www.w3.org/2000/svg"><text>DUPLICATE_PAGE</text></svg></body></html>"#
    )
    .into_bytes();
    let duplicate = zip_epub(&package, &[("body.xhtml", body)], None);
    let duplicate_azw3 = convert_result(&duplicate).expect("duplicate viewport metas must convert");
    assert_eq!(duplicate_azw3.exth().text(126), None);
    let rawml_bytes = duplicate_azw3.rawml();
    let rawml = String::from_utf8_lossy(&rawml_bytes);
    assert!(rawml.contains("width=1200,height=600"));
    assert!(rawml.contains("height=900;width=600"));

    for (content, expected) in [
        ("width=1200", "both width and height"),
        ("width=12px,height=600", "positive numbers"),
        ("width=1200,width=1200,height=600", "duplicated"),
    ] {
        let epub = fixed_epub(&[("body.xhtml", Some(content))], None);
        let error = convert_result(&epub).expect_err("invalid viewport must reject");
        assert!(error.contains("viewport"));
        assert!(error.contains(expected), "{content}: {error}");
    }
}

// E2E-ID: E2E-FXL-05
// Audit: G6-14
#[test]
fn reflowable_viewport_does_not_emit_exth126() {
    // Audit coverage: G6-14. Direct absence assertion covers the reflowable
    // viewport boundary only.
    let package = base_package(
        r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#,
        r#"<itemref idref="body"/>"#,
        "",
    );
    let body = format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><meta name="viewport" content="width=1200,height=600"/></head><body><p>{BASELINE_MARKER}</p><p>REFLOWABLE_VIEWPORT</p></body></html>"#
    )
    .into_bytes();
    let epub = zip_epub(&package, &[("body.xhtml", body)], None);
    let azw3 = convert_result(&epub).expect("reflowable viewport must remain supported");
    assert_eq!(azw3.exth().text(126), None);
    assert!(reconstructed_sections(&azw3).contains("REFLOWABLE_VIEWPORT"));
}
