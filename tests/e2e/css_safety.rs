//! Primary audit coverage: G7-07, G7-09, G7-10, G7-25, G7-26.

use crate::support::{Azw3, zip_epub};
use aozoraepub3_to_azw3::{ConvertOptions, convert_bytes};

const BASELINE_MARKER: &str = "EPUB33_P0_ROUND4_CSS_BASELINE";

fn package(has_child_css: bool) -> String {
    let child = if has_child_css {
        r#"<item id="child" href="child.css" media-type="text/css"/>"#
    } else {
        ""
    };
    format!(
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/ops" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>EPUB 3.3 CSS Round4</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language></metadata><manifest><item id="style" href="style.css" media-type="text/css"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>{child}</manifest><spine><itemref idref="body"/></spine></package>"#
    )
}

fn epub(
    css: &str,
    inline_style: Option<&str>,
    style_element: Option<&str>,
    child_css: Option<&str>,
) -> Vec<u8> {
    let inline_style = inline_style
        .map(|value| format!(r#" style="{value}""#))
        .unwrap_or_default();
    let style_element = style_element
        .map(|value| format!(r#"<style>{value}</style>"#))
        .unwrap_or_default();
    let body = format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><link rel="stylesheet" href="style.css"/>{style_element}</head><body><h1>Heading</h1><p{inline_style}>{BASELINE_MARKER}</p><p>Second paragraph</p></body></html>"#
    );
    let mut files = vec![
        ("style.css", css.as_bytes().to_vec()),
        ("body.xhtml", body.into_bytes()),
    ];
    if let Some(child_css) = child_css {
        files.push(("child.css", child_css.as_bytes().to_vec()));
    }
    zip_epub(&package(child_css.is_some()), &files, None)
}

fn convert_result(epub: &[u8]) -> Result<Azw3, String> {
    convert_bytes(epub, &ConvertOptions::default())
        .map(Azw3::parse)
        .map_err(|error| error.to_string())
}

fn assert_unsupported(epub: &[u8], expected: &str) {
    let error = convert_result(epub).expect_err("unsupported CSS must not produce an AZW3");
    assert!(
        error.to_ascii_lowercase().contains("unsupported"),
        "expected an UnsupportedEpub error, got: {error}"
    );
    assert!(
        error
            .to_ascii_lowercase()
            .contains(&expected.to_ascii_lowercase()),
        "expected error containing {expected:?}, got: {error}"
    );
}

fn assert_success(epub: &[u8]) {
    let azw3 = convert_result(epub).expect("supported CSS must continue to convert");
    let sections = azw3
        .reconstructed_xhtml_sections()
        .into_iter()
        .map(|section| String::from_utf8_lossy(&section).into_owned())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(sections.contains(BASELINE_MARKER));
}

// E2E-ID: E2E-CSS-02
// Audit: G7-09, G7-10, G7-25, G7-26
#[test]
fn unsupported_css_semantics_reject_with_feature_errors() {
    // Audit coverage: G7-09, G7-10, G7-25, G7-26. Direct feature-tagged
    // errors establish the documented CSS safe-reject boundaries.
    for (css, expected) in [
        ("h1 + p { color: red; }", "sibling combinator"),
        ("h1 ~ p { color: red; }", "sibling combinator"),
        ("p:first-child { color: red; }", "pseudo-class"),
        ("li:nth-child(2) { color: red; }", "pseudo-class"),
        (".note::before { content: \"NOTE\"; }", "pseudo-element"),
        (".note::after { content: \"END\"; }", "pseudo-element"),
        ("p::first-letter { color: red; }", "pseudo-element"),
        ("p::first-line { color: red; }", "pseudo-element"),
        ("p:before { content: \"NOTE\"; }", "pseudo-element"),
        ("p:after { content: \"END\"; }", "pseudo-element"),
        ("body { counter-reset: chapter; }", "counter"),
        ("h1 { counter-increment: chapter; }", "counter"),
        ("p { color: counter(chapter); }", "counter"),
        ("p { color: counters(chapter, '.'); }", "counter"),
        (".note { content: \"NOTE\"; }", "generated content"),
    ] {
        assert_unsupported(&epub(css, None, None, None), expected);
    }
}

// E2E-ID: E2E-CSS-01
// Audit: G7-01, G7-17
#[test]
fn css_input_surfaces_are_checked_for_unsupported_semantics() {
    // Audit coverage: G7-09, G7-10. Direct style-surface assertions ensure
    // safety checks apply consistently across stylesheet and inline inputs.
    assert_unsupported(
        &epub(
            "body { color: black; }",
            None,
            Some("h1 + p { color: red; }"),
            None,
        ),
        "sibling combinator",
    );
    assert_unsupported(
        &epub(
            "body { color: black; }",
            Some("counter-reset: chapter"),
            None,
            None,
        ),
        "counter",
    );
}

#[test]
fn css_scanner_ignores_comments_strings_urls_and_safe_boundaries() {
    // Diagnostic boundary coverage for G7-09/G7-25/G7-26 false positives;
    // retained as supporting evidence, not a separate primary owner.
    let false_positive_free = concat!(
        "/* h1 + p ::before :first-child counter(x) */\n",
        "body { background-image: url(\"a+b.png\"); ",
        "--literal: \":first-child counter(x)\"; }"
    );
    assert_success(&epub(false_positive_free, None, None, None));

    for (css, forbidden_error) in [
        (".literal { content: \"a + b\"; }", "sibling combinator"),
        (".literal { content: \":first-child\"; }", "pseudo-class"),
    ] {
        let generated_error = convert_result(&epub(css, None, None, None))
            .expect_err("generated content must reject");
        assert!(generated_error.contains("generated content"));
        assert!(!generated_error.contains(forbidden_error));
    }

    let safe_css = concat!(
        "a:link { color: green; } ",
        "@media amzn-kf8 { .media { color: red; } } ",
        "@import \"child.css\"; ",
        ".page { page-break-before: always; page-break-inside: avoid; } ",
        ".writing { writing-mode: vertical-rl; direction: rtl; } ",
        ".neutral { content: normal; } ",
        "li:nth-of-type(2n+1) { color: olive; } ",
        "@font-face { font-family: Round4; src: url(\"font.ttf\"); }"
    );
    assert_success(&epub(safe_css, None, None, Some(".child { color: blue; }")));
}

#[test]
fn generated_content_and_counter_functions_do_not_hide_in_strings() {
    // Audit coverage: G7-25, G7-26. Direct rejection/acceptance assertions
    // distinguish active semantics from strings and URLs.
    let literal_values = concat!(
        ".literal { ",
        "--pseudo: \":first-child\"; ",
        "--counter: \"counter(x)\"; ",
        "background-image: url(\"nth-child.png\"); ",
        "color: red; }"
    );
    assert_success(&epub(literal_values, None, None, None));

    assert_unsupported(
        &epub(
            ".generated { content: counters(chapter, '.'); }",
            None,
            None,
            None,
        ),
        "generated content",
    );
    assert_unsupported(
        &epub(".generated { content/**/: \"NOTE\"; }", None, None, None),
        "generated content",
    );
    assert_unsupported(
        &epub(".counter { counter/**/-reset: chapter; }", None, None, None),
        "counter",
    );
}
