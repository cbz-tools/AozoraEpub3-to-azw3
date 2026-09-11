//! Primary audit coverage: G7-06, G7-08, G7-12, G7-16, G7-27, G7-28.

use crate::support::{Azw3, zip_epub};
use aozoraepub3_to_azw3::{ConvertOptions, convert_bytes};

const BASELINE_MARKER: &str = "EPUB33_P2_CSS_TRANSPORT_BASELINE";

struct Case {
    id: &'static str,
    css: &'static str,
    observables: &'static [&'static str],
    data_url: bool,
}

// E2E-ID: E2E-CSS-03
// Audit: G2-17, G4-02, G7-01
#[test]
fn css_transport_preserves_selectors_at_rules_and_variables() {
    // Audit coverage: G7-06, G7-07, G7-08, G7-12, G7-16, G7-27, G7-28.
    // This is converter transport evidence only; it does not claim firmware
    // selector/variable evaluation or device rendering.
    let cases = [
        Case {
            id: "G7-06-descendant",
            css: "div p { color: red; }",
            observables: &["div p", "color: red"],
            data_url: false,
        },
        Case {
            id: "G7-05-child",
            css: "div > p { color: red; }",
            observables: &["div > p", "color: red"],
            data_url: false,
        },
        Case {
            id: "G7-08-attribute",
            css: "[data-kind=\"x\"] { color: red; }",
            observables: &["[data-kind=\"x\"]", "color: red"],
            data_url: false,
        },
        Case {
            id: "G7-12-supports",
            css: ".base { display: block; } @supports (display:grid) { .base { display: none; } }",
            observables: &["@supports (display:grid)", ".base", "display: none"],
            data_url: false,
        },
        Case {
            id: "G7-16-data-png",
            css: ".asset { background-image: url(data:image/png;base64,AAAA); }",
            observables: &["data:image/png;base64,AAAA"],
            data_url: true,
        },
        Case {
            id: "G7-16-data-jpeg",
            css: ".asset { background-image: url(data:image/jpeg;base64,AAAA); }",
            observables: &["data:image/jpeg;base64,AAAA"],
            data_url: true,
        },
        Case {
            id: "G7-16-data-svg",
            css: ".asset { background-image: url(data:image/svg+xml,%3Csvg%3E%3C/svg%3E); }",
            observables: &["data:image/svg+xml,%3Csvg%3E%3C/svg%3E"],
            data_url: true,
        },
        Case {
            id: "G7-16-data-other",
            css: ".asset { background-image: url(data:application/octet-stream;base64,AAAA); }",
            observables: &["data:application/octet-stream;base64,AAAA"],
            data_url: true,
        },
        Case {
            id: "G7-27-declaration-only",
            css: ":root { --main-color: black; }",
            observables: &["--main-color: black"],
            data_url: false,
        },
        Case {
            id: "G7-28-var-consumption",
            css: ":root { --display-mode: none; } .hidden { display: var(--display-mode); }",
            observables: &["--display-mode: none", "var(--display-mode)"],
            data_url: false,
        },
    ];

    for case in cases {
        let azw3 = convert_case(&case);
        let sections = reconstructed_sections(&azw3);
        let css_flow = css_flows(&azw3);

        assert!(
            sections.contains(BASELINE_MARKER),
            "baseline missing for {}",
            case.id
        );
        assert!(
            css_flow.contains(case.id),
            "case marker missing for {}",
            case.id
        );
        for observable in case.observables {
            assert!(
                css_flow.contains(observable),
                "CSS observable {observable:?} missing for {}: {css_flow}",
                case.id
            );
        }

        if case.data_url {
            assert!(
                !css_flow.contains("kindle:embed:"),
                "data URL was incorrectly converted into a package resource for {}: {css_flow}",
                case.id
            );
        }
    }
}

fn convert_case(case: &Case) -> Azw3 {
    let epub = zip_epub(
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>EPUB 3.3 CSS transport</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language></metadata><manifest><item id="style" href="style.css" media-type="text/css"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="body"/></spine></package>"#,
        &[
            (
                "style.css",
                format!("/* {} */\n{}", case.id, case.css).into_bytes(),
            ),
            (
                "body.xhtml",
                format!(
                    r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><link rel="stylesheet" href="style.css"/></head><body><div><p class="base hidden asset" data-kind="x">{BASELINE_MARKER}</p></div></body></html>"#
                )
                .into_bytes(),
            ),
        ],
        None,
    );

    Azw3::parse(
        convert_bytes(&epub, &ConvertOptions::default())
            .unwrap_or_else(|error| panic!("{} must transport safely: {error}", case.id)),
    )
}

fn reconstructed_sections(azw3: &Azw3) -> String {
    azw3.reconstructed_xhtml_sections()
        .into_iter()
        .map(|section| String::from_utf8_lossy(&section).into_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

fn css_flows(azw3: &Azw3) -> String {
    let rawml = azw3.rawml();
    azw3.fdst_ranges()
        .into_iter()
        .skip(1)
        .filter_map(|(start, end)| rawml.get(start..end))
        .map(String::from_utf8_lossy)
        .collect::<Vec<_>>()
        .join("\n")
}
