//! Diagnostic characterization only; not a future CSS support contract.

use crate::support::{Azw3, zip_epub};
use aozoraepub3_to_azw3::{ConvertOptions, convert_bytes};

const BASELINE_MARKER: &str = "EPUB33_P2_CSS_BASELINE_MARKER";

#[derive(Clone, Copy)]
struct CssCase {
    id: &'static str,
    css: &'static str,
    inline_style: Option<&'static str>,
    child_css: Option<&'static str>,
    image_resource: bool,
    font_resource: bool,
}

impl CssCase {
    const fn stylesheet(
        id: &'static str,
        css: &'static str,
        child_css: Option<&'static str>,
    ) -> Self {
        Self {
            id,
            css,
            inline_style: None,
            child_css,
            image_resource: false,
            font_resource: false,
        }
    }

    const fn inline(id: &'static str, style: &'static str, image_resource: bool) -> Self {
        Self {
            id,
            css: "body { color: black; }",
            inline_style: Some(style),
            child_css: None,
            image_resource,
            font_resource: false,
        }
    }
}

#[test]
fn css_characterization_matrix_observes_writer_transport() {
    let cases = [
        CssCase::inline("CSS-01", "margin-left:2em;color:red", false),
        CssCase::inline("CSS-02", "background-image:url('../images/bg.png')", true),
        CssCase::stylesheet("CSS-03", "h1 + p { color: red; }", None),
        CssCase::stylesheet("CSS-04", "h1 ~ p { color: blue; }", None),
        CssCase::stylesheet("CSS-05", "a:link { color: green; }", None),
        CssCase::stylesheet("CSS-06", "p:first-child { color: purple; }", None),
        CssCase::stylesheet("CSS-07", "li:nth-child(2) { color: orange; }", None),
        CssCase::stylesheet("CSS-27", "div p { color: teal; }", None),
        CssCase::stylesheet("CSS-28", "div > p { color: maroon; }", None),
        CssCase::stylesheet("CSS-29", "[data-kind=\"x\"] { color: navy; }", None),
        CssCase::stylesheet("CSS-08", ".note::before { content: \"NOTE: \"; }", None),
        CssCase::stylesheet("CSS-09", ".note::after { content: \" END\"; }", None),
        CssCase::stylesheet("CSS-10", "@media amzn-kf8 { .media { color: red; } }", None),
        CssCase::stylesheet("CSS-11", "@media screen { .media { color: blue; } }", None),
        CssCase::stylesheet(
            "CSS-12",
            "@supports (display:block) { .supports { color: green; } }",
            None,
        ),
        CssCase::stylesheet(
            "CSS-13",
            "@import \"child.css\"; .imported { color: red; }",
            Some(".child { color: blue; }"),
        ),
        CssCase::stylesheet(
            "CSS-14",
            "@import url(\"child.css\") amzn-kf8; .imported { color: red; }",
            Some(".child { color: blue; }"),
        ),
        CssCase::stylesheet(
            "CSS-15",
            "@import \"child.css\"; .cycle { color: red; }",
            Some("@import \"style.css\"; .child { color: blue; }"),
        ),
        CssCase {
            id: "CSS-16",
            css: ".asset { background-image:url('../images/bg.png'); }",
            inline_style: None,
            child_css: None,
            image_resource: true,
            font_resource: false,
        },
        CssCase {
            id: "CSS-17",
            css: "@font-face { font-family: P2Fixture; src:url('../fonts/font.ttf'); } .font { font-family:P2Fixture; }",
            inline_style: None,
            child_css: None,
            image_resource: false,
            font_resource: true,
        },
        CssCase::stylesheet(
            "CSS-18",
            ".data { background-image:url(data:image/png;base64,AAAA); }",
            None,
        ),
        CssCase::stylesheet("CSS-19", ".break { page-break-before: always; }", None),
        CssCase::stylesheet("CSS-20", ".break { break-before: page; }", None),
        CssCase::stylesheet("CSS-21", ".keep { page-break-inside: avoid; }", None),
        CssCase::stylesheet(
            "CSS-22",
            "body { counter-reset: chapter; } h1 { counter-increment: chapter; }",
            None,
        ),
        CssCase::stylesheet(
            "CSS-23",
            ".chapter::before { content: counter(chapter); }",
            None,
        ),
        CssCase::stylesheet("CSS-24", ":root { --main-color: black; }", None),
        CssCase::stylesheet(
            "CSS-25",
            ":root { --main-color: black; } body { color: var(--main-color); }",
            None,
        ),
        CssCase::stylesheet(
            "CSS-26",
            ":root { --display-mode: none; } .hidden { display: var(--display-mode); }",
            None,
        ),
    ];

    assert_eq!(cases.len(), 29);
    for case in cases {
        characterize_case(case);
    }
}

fn characterize_case(case: CssCase) {
    let image_manifest = if case.image_resource {
        r#"<item id="bg" href="images/bg.png" media-type="image/png"/>"#
    } else {
        ""
    };
    let font_manifest = if case.font_resource {
        r#"<item id="font" href="fonts/font.ttf" media-type="font/ttf"/>"#
    } else {
        ""
    };
    let child_manifest = if case.child_css.is_some() {
        r#"<item id="child" href="child.css" media-type="text/css"/>"#
    } else {
        ""
    };
    let package = format!(
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>EPUB 3.3 P2 CSS characterization</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language></metadata><manifest><item id="style" href="style.css" media-type="text/css"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>{child_manifest}{image_manifest}{font_manifest}</manifest><spine><itemref idref="body"/></spine></package>"#
    );
    let inline_style = case
        .inline_style
        .map(|style| format!(" style=\"{style}\""))
        .unwrap_or_default();
    let body = format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><link rel="stylesheet" href="style.css"/></head><body><h1>Heading</h1><p class="note chapter media supports imported child cycle data asset font hidden" data-kind="x"{inline_style}>{BASELINE_MARKER}</p><p>Second paragraph</p><ul><li>One</li><li>Two</li></ul><div><p>Nested paragraph</p></div><a href="https://example.invalid/">Link</a></body></html>"#
    );
    let mut files = vec![
        (
            "style.css",
            format!("/* {} */\n{}", case.id, case.css).into_bytes(),
        ),
        ("body.xhtml", body.into_bytes()),
    ];
    if let Some(child_css) = case.child_css {
        files.push((
            "child.css",
            format!("/* {} child */\n{child_css}", case.id).into_bytes(),
        ));
    }
    if case.image_resource {
        files.push(("images/bg.png", b"P2_CSS_IMAGE_RESOURCE".to_vec()));
    }
    if case.font_resource {
        files.push(("fonts/font.ttf", b"P2_CSS_FONT_RESOURCE".to_vec()));
    }

    let epub = zip_epub(&package, &files, None);
    let source_css = format!("/* {} */\n{}", case.id, case.css);
    assert!(source_css.contains(case.id));

    match convert_bytes(&epub, &ConvertOptions::default()) {
        Ok(bytes) => {
            let azw3 = Azw3::parse(bytes);
            let rawml_bytes = azw3.rawml();
            let rawml = String::from_utf8_lossy(&rawml_bytes);
            let css_flows = azw3
                .fdst_ranges()
                .into_iter()
                .skip(1)
                .filter_map(|(start, end)| rawml_bytes.get(start..end))
                .map(String::from_utf8_lossy)
                .collect::<Vec<_>>()
                .join("\n");
            let reconstructed = azw3
                .reconstructed_xhtml_sections()
                .into_iter()
                .map(|section| String::from_utf8_lossy(&section).into_owned())
                .collect::<Vec<_>>()
                .join("\n");
            let css_marker = css_flows.contains(case.id);
            let css_fragment_survives = css_observable_fragment(case.id)
                .is_some_and(|fragment| css_flows.contains(fragment));
            let baseline_marker = reconstructed.contains(BASELINE_MARKER);
            let rawml_css_marker = rawml.contains(case.id) || css_flows.contains(case.id);
            let flow_reference = reconstructed.contains("kindle:flow:");
            let embed_reference = css_flows.contains("kindle:embed:");
            let image_serialized = !azw3.image_records().is_empty();
            let local_css_url_preserved = reconstructed.contains("../images/bg.png")
                || css_flows.contains("../images/bg.png");
            let style_attribute_preserved = reconstructed.contains(" style=\"");
            let inline_declaration_preserved = case
                .inline_style
                .is_some_and(|style| reconstructed.contains(style));
            let import_source_preserved = css_flows.contains("@import");
            let import_flow_rewritten = css_flows.contains("url(kindle:flow:");
            let data_url_preserved = css_flows.contains("data:image");
            println!(
                "EPUB33_P2_CSS|case={}|result=success|baseline={}|css-marker={}|css-fragment-survives={}|rawml-css-marker={}|flow-reference={}|embed-reference={}|image-record={}|style-attribute-preserved={}|inline-declaration-preserved={}|local-css-url-preserved={}|import-source-preserved={}|import-flow-rewritten={}|data-url-preserved={}",
                case.id,
                baseline_marker,
                css_marker,
                css_fragment_survives,
                rawml_css_marker,
                flow_reference,
                embed_reference,
                image_serialized,
                style_attribute_preserved,
                inline_declaration_preserved,
                local_css_url_preserved,
                import_source_preserved,
                import_flow_rewritten,
                data_url_preserved,
            );
            assert!(
                baseline_marker,
                "{}: successful conversion lost the XHTML baseline marker",
                case.id
            );
        }
        Err(error) => {
            println!(
                "EPUB33_P2_CSS|case={}|result=error|error={}",
                case.id, error
            );
        }
    }
}

fn css_observable_fragment(id: &str) -> Option<&'static str> {
    Some(match id {
        "CSS-03" => "h1 + p",
        "CSS-04" => "h1 ~ p",
        "CSS-05" => "a:link",
        "CSS-06" => "p:first-child",
        "CSS-07" => "li:nth-child(2)",
        "CSS-27" => "div p",
        "CSS-28" => "div > p",
        "CSS-29" => "[data-kind=\"x\"]",
        "CSS-08" => ".note::before",
        "CSS-09" => ".note::after",
        "CSS-10" => "@media amzn-kf8",
        "CSS-11" => "@media screen",
        "CSS-12" => "@supports",
        "CSS-13" | "CSS-14" | "CSS-15" => "@import",
        "CSS-16" => "background-image",
        "CSS-17" => "@font-face",
        "CSS-18" => "data:image",
        "CSS-19" => "page-break-before",
        "CSS-20" => "break-before",
        "CSS-21" => "page-break-inside",
        "CSS-22" => "counter-reset",
        "CSS-23" => "counter(",
        "CSS-24" | "CSS-25" => "--main-color",
        "CSS-26" => "--display-mode",
        _ => return None,
    })
}
