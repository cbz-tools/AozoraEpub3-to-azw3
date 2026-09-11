//! Diagnostic characterization only; not a future CSS support contract.

use crate::support::{Azw3, zip_epub};
use aozoraepub3_to_azw3::{ConvertOptions, convert_bytes};

const BASELINE_MARKER: &str = "EPUB33_P2_CSS_CAPABILITY_BASELINE";

struct Case {
    id: &'static str,
    css: &'static str,
    observable: &'static str,
}

#[test]
fn remaining_css_capability_boundaries_are_observable() {
    let cases = [
        Case {
            id: "G7-06-descendant",
            css: "div p { color: red; }",
            observable: "div p",
        },
        Case {
            id: "G7-05-child-boundary",
            css: "div > p { color: red; }",
            observable: "div > p",
        },
        Case {
            id: "G7-08-attribute",
            css: "[data-kind=\"x\"] { color: red; }",
            observable: "[data-kind=\"x\"]",
        },
        Case {
            id: "G7-12-supports",
            css: " .base { display: block; } @supports (display:grid) { .base { display: none; } }",
            observable: "@supports",
        },
        Case {
            id: "G7-16-data-png",
            css: ".asset { background-image: url(data:image/png;base64,AAAA); }",
            observable: "data:image/png",
        },
        Case {
            id: "G7-16-data-jpeg",
            css: ".asset { background-image: url(data:image/jpeg;base64,AAAA); }",
            observable: "data:image/jpeg",
        },
        Case {
            id: "G7-16-data-svg",
            css: ".asset { background-image: url(data:image/svg+xml,%3Csvg%3E%3C/svg%3E); }",
            observable: "data:image/svg+xml",
        },
        Case {
            id: "G7-16-data-other",
            css: ".asset { background-image: url(data:application/octet-stream;base64,AAAA); }",
            observable: "data:application/octet-stream",
        },
        Case {
            id: "G7-27-declaration-only",
            css: ":root { --main-color: black; }",
            observable: "--main-color",
        },
        Case {
            id: "G7-28-var-consumption",
            css: ":root { --display-mode: none; } .hidden { display: var(--display-mode); }",
            observable: "var(--display-mode)",
        },
    ];

    for case in cases {
        characterize(case);
    }
}

fn characterize(case: Case) {
    let epub = zip_epub(
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>EPUB 3.3 CSS capability characterization</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language></metadata><manifest><item id="style" href="style.css" media-type="text/css"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="body"/></spine></package>"#,
        &[
            (
                "style.css",
                format!("/* {} */\n{}", case.id, case.css).into_bytes(),
            ),
            (
                "body.xhtml",
                format!(r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><link rel="stylesheet" href="style.css"/></head><body><div><p class="base hidden asset" data-kind="x">{BASELINE_MARKER}</p></div></body></html>"#).into_bytes(),
            ),
        ],
        None,
    );

    match convert_bytes(&epub, &ConvertOptions::default()) {
        Ok(bytes) => {
            let azw3 = Azw3::parse(bytes);
            let rawml_bytes = azw3.rawml();
            let sections = azw3
                .reconstructed_xhtml_sections()
                .into_iter()
                .map(|section| String::from_utf8_lossy(&section).into_owned())
                .collect::<Vec<_>>()
                .join("\n");
            let rawml = String::from_utf8_lossy(&rawml_bytes);
            let css_flows = azw3
                .fdst_ranges()
                .into_iter()
                .skip(1)
                .filter_map(|(start, end)| rawml_bytes.get(start..end))
                .map(String::from_utf8_lossy)
                .collect::<Vec<_>>()
                .join("\n");
            let source_survives = css_flows.contains(case.observable);
            println!(
                "EPUB33_P2_CSS_CAPABILITY|case={}|result=success|baseline={}|source-observable={}|rawml-marker={}|resource-rewrite={}",
                case.id,
                sections.contains(BASELINE_MARKER),
                source_survives,
                rawml.contains(case.id),
                css_flows.contains("kindle:embed:")
            );
            assert!(sections.contains(BASELINE_MARKER));
        }
        Err(error) => {
            println!(
                "EPUB33_P2_CSS_CAPABILITY|case={}|result=error|error={}",
                case.id, error
            );
        }
    }
}
