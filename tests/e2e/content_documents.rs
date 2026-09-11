//! Primary audit coverage: G3-02, G3-03, G3-08, G3-09, G3-13, G3-16,
//! G3-17, G3-19, G3-20, G5-11, G5-13.

use crate::support::{Azw3, zip_epub};
use aozoraepub3_to_azw3::{ConvertOptions, convert_bytes};

fn package(manifest: &str, spine: &str) -> String {
    format!(
        r#"<package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Round8 G3 G5</dc:title><dc:creator>Round8 Author</dc:creator><dc:language>en</dc:language></metadata><manifest>{manifest}</manifest><spine>{spine}</spine></package>"#
    )
}

fn rawml(azw3: &Azw3) -> String {
    String::from_utf8_lossy(&azw3.rawml()).into_owned()
}

fn sections(azw3: &Azw3) -> String {
    azw3.reconstructed_xhtml_sections()
        .into_iter()
        .map(|section| String::from_utf8_lossy(&section).into_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

fn convert_result(epub: &[u8]) -> Result<Azw3, String> {
    convert_bytes(epub, &ConvertOptions::default())
        .map(Azw3::parse)
        .map_err(|error| error.to_string())
}

// E2E-ID: E2E-CONTENT-01
// Audit: G3-02, G3-03, G3-08, G3-09, G3-13, G3-20, G8-11
#[test]
fn general_xhtml_semantics_project_without_attribute_loss() {
    // Audit coverage: G3-02, G3-03, G3-08, G3-09, G3-13, G3-20 and G8-11.
    let manifest = r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#;
    let body = br##"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops" prefix="schema: https://schema.org/" vocab="https://schema.org/" typeof="schema:Article" about="#article"><body><section epub:type="chapter unknown-structural-token" property="schema:articleBody" resource="#section"><header role="banner" aria-label="HEADER_ARIA"><hgroup><h1>ROUND8_STRUCTURAL_MARKER</h1><h2>ROUND8_SUBHEADING_MARKER</h2></hgroup></header><article><aside rel="schema:mentions" rev="schema:isPartOf" content="ROUND8_RDFA_MARKER"><p aria-describedby="detail" aria-labelledby="heading" aria-hidden="false" aria-expanded="true" aria-current="page">ROUND8_ARIA_MARKER</p></aside><figure><figcaption>ROUND8_FIGURE_MARKER</figcaption></figure><table aria-label="ROUND8_TABLE_LABEL"><caption>ROUND8_TABLE_MARKER</caption><colgroup><col span="1"/></colgroup><thead><tr><th scope="col" rowspan="1" headers="head">Head</th></tr></thead><tbody><tr><td colspan="1" headers="head">Cell</td></tr></tbody><tfoot><tr><td>Foot</td></tr></tfoot></table></article><footer>ROUND8_FOOTER_MARKER</footer></section></body></html>"##;
    let epub = zip_epub(
        &package(manifest, r#"<itemref idref="body"/>"#),
        &[("body.xhtml", body.to_vec())],
        None,
    );
    let result = convert_result(&epub);
    println!(
        "ROUND8_OBSERVATION|ids=G3-02/G3-03/G3-08/G3-09/G3-13/G3-20|result={}",
        result
            .as_ref()
            .map(|_| "success")
            .unwrap_or("explicit-error")
    );
    let azw3 = result.expect("general XHTML semantic matrix must convert");
    let output = sections(&azw3);
    for marker in [
        "ROUND8_STRUCTURAL_MARKER",
        "ROUND8_SUBHEADING_MARKER",
        "ROUND8_RDFA_MARKER",
        "ROUND8_ARIA_MARKER",
        "ROUND8_FIGURE_MARKER",
        "ROUND8_TABLE_MARKER",
        "ROUND8_FOOTER_MARKER",
    ] {
        assert!(output.contains(marker), "semantic marker missing: {marker}");
    }
    for attribute in [
        "epub:type=\"chapter unknown-structural-token\"",
        "prefix=\"schema: https://schema.org/\"",
        "vocab=\"https://schema.org/\"",
        "typeof=\"schema:Article\"",
        "about=\"#article\"",
        "property=\"schema:articleBody\"",
        "resource=\"#section\"",
        "rel=\"schema:mentions\"",
        "rev=\"schema:isPartOf\"",
        "role=\"banner\"",
        "aria-label=\"HEADER_ARIA\"",
        "aria-describedby=\"detail\"",
        "aria-labelledby=\"heading\"",
        "aria-hidden=\"false\"",
        "aria-expanded=\"true\"",
        "aria-current=\"page\"",
        "scope=\"col\"",
        "rowspan=\"1\"",
        "colspan=\"1\"",
        "headers=\"head\"",
    ] {
        assert!(
            output.contains(attribute),
            "attribute was not transported: {attribute}"
        );
    }
    assert!(rawml(&azw3).contains("ROUND8_TABLE_MARKER"));
    println!(
        "ROUND8_SAFE_PROJECTION|ids=G3-02/G3-03/G3-08/G3-09/G3-13/G3-20|observables=RawML+reconstructed-XHTML+structural-attrs+table"
    );
}

// E2E-ID: E2E-CONTENT-03
// Audit: G3-19
#[test]
fn picture_fallback_projects_and_package_srcset_rejects() {
    // Audit coverage: G3-19.
    let manifest = r#"
        <item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>
        <item id="fallback" href="images/fallback.png" media-type="image/png"/>
        <item id="large" href="images/large.png" media-type="image/png"/>
    "#;
    let fallback_body = br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><picture><img src="images/fallback.png" alt="ROUND8_FALLBACK_ALT"/></picture><p>ROUND8_PICTURE_FALLBACK_SAFE</p></body></html>"#;
    let fallback_epub = zip_epub(
        &package(manifest, r#"<itemref idref="body"/>"#),
        &[
            ("body.xhtml", fallback_body.to_vec()),
            ("images/fallback.png", b"synthetic-fallback-image".to_vec()),
            ("images/large.png", b"synthetic-large-image".to_vec()),
        ],
        None,
    );
    let result = convert_result(&fallback_epub);
    println!(
        "ROUND8_OBSERVATION|id=G3-19|fixture=fallback-only|result={}",
        result
            .as_ref()
            .map(|_| "success")
            .unwrap_or("explicit-error")
    );
    let azw3 = result.expect("fallback-only picture must remain safe");
    let output = sections(&azw3);
    assert!(output.contains("ROUND8_FALLBACK_ALT"));
    assert!(output.contains("ROUND8_PICTURE_FALLBACK_SAFE"));
    assert!(
        output.contains("kindle:embed:"),
        "fallback image was not projected"
    );
    assert!(!output.contains("images/fallback.png"));
    assert!(
        !azw3.image_records().is_empty(),
        "fallback resource was not serialized"
    );

    let external_data_body = br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><img srcset="data:image/png;base64,AAAA 1x"/><img srcset="DATA:image/png;base64,BBBB 2x"/><p>ROUND8_EXTERNAL_DATA_SRCSET_SAFE</p></body></html>"#;
    let external_data_epub = zip_epub(
        &package(
            r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#,
            r#"<itemref idref="body"/>"#,
        ),
        &[("body.xhtml", external_data_body.to_vec())],
        None,
    );
    let external_data =
        convert_result(&external_data_epub).expect("external data srcset must remain supported");
    assert!(sections(&external_data).contains("ROUND8_EXTERNAL_DATA_SRCSET_SAFE"));
    let external_data_rawml = rawml(&external_data);
    assert!(external_data_rawml.contains("data:image/png;base64,AAAA 1x"));
    assert!(external_data_rawml.contains("DATA:image/png;base64,BBBB 2x"));
    println!(
        "ROUND8_SAFE_BOUNDARY|id=G3-19|fixture=external-data-srcset|external-data-srcset=accepted"
    );

    for (fixture, srcset) in [
        ("picture-source", "images/large.png 2x"),
        ("img-srcset", "images/large.png 2x"),
    ] {
        let body = format!(
            r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><picture>{}<img src="images/fallback.png"/></picture></body></html>"#,
            if fixture == "picture-source" {
                format!(r#"<source srcset="{srcset}"/>"#)
            } else {
                String::new()
            }
        )
        .replace(
            "<img src=\"images/fallback.png\"/>",
            &format!(
                r#"<img src="images/fallback.png" srcset="{srcset}"/>"#
            ),
        );
        let epub = zip_epub(
            &package(manifest, r#"<itemref idref="body"/>"#),
            &[
                ("body.xhtml", body.into_bytes()),
                ("images/fallback.png", b"synthetic-fallback-image".to_vec()),
                ("images/large.png", b"synthetic-large-image".to_vec()),
            ],
            None,
        );
        let error = convert_result(&epub).expect_err("package-local srcset must be rejected");
        assert!(error.contains("G3-19"), "feature tag missing: {error}");
        assert!(error.contains("srcset"), "srcset boundary missing: {error}");
        println!(
            "ROUND8_SAFE_BOUNDARY|id=G3-19|fixture={fixture}|package-local-srcset=explicitly-rejected|fallback-only=accepted"
        );
    }
}

// E2E-ID: E2E-CONTENT-02
// Audit: G3-14, G3-15, G3-16, G3-17
#[test]
fn cross_document_and_external_links_are_safe() {
    // Audit coverage: G3-16, G3-17.
    let manifest = r#"
        <item id="one" href="OPS/chapters/one.xhtml" media-type="application/xhtml+xml"/>
        <item id="two" href="OPS/章/二.xhtml" media-type="application/xhtml+xml"/>
    "#;
    let one = "<html xmlns=\"http://www.w3.org/1999/xhtml\"><body><p>ROUND8_LINK_SOURCE</p><a href=\"../%E7%AB%A0/%E4%BA%8C.xhtml#unicode-target\">ROUND8_PERCENT_LINK</a><a href=\"../章/二.xhtml#unicode-target\">ROUND8_UNICODE_LINK</a><a href=\"http://example.com/http\">HTTP</a><a href=\"https://example.com/https\">HTTPS</a><a href=\"//cdn.example.com/asset\">SCHEME_RELATIVE</a><a href=\"mailto:reader@example.com\">MAILTO</a><a href=\"tel:+81300000000\">TEL</a></body></html>".as_bytes().to_vec();
    let two = br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p id="unicode-target">ROUND8_LINK_TARGET</p></body></html>"#;
    let epub = zip_epub(
        &package(manifest, r#"<itemref idref="one"/><itemref idref="two"/>"#),
        &[
            ("OPS/chapters/one.xhtml", one),
            ("OPS/章/二.xhtml", two.to_vec()),
        ],
        None,
    );
    let result = convert_result(&epub);
    println!(
        "ROUND8_OBSERVATION|ids=G3-16/G3-17|fixture=relative-percent-unicode-and-external|result={}",
        result
            .as_ref()
            .map(|_| "success")
            .unwrap_or("explicit-error")
    );
    let azw3 = result.expect("cross-document and external link matrix must convert");
    let output = sections(&azw3);
    assert!(output.contains("ROUND8_LINK_TARGET"));
    assert!(output.matches("kindle:pos:fid:").count() >= 2);
    for href in [
        "http://example.com/http",
        "https://example.com/https",
        "//cdn.example.com/asset",
        "mailto:reader@example.com",
        "tel:+81300000000",
    ] {
        assert!(output.contains(href), "external link changed: {href}");
    }
    println!(
        "ROUND8_SAFE_PROJECTION|ids=G3-16/G3-17|cross-document=percent+unicode+fragment|external=http+https+scheme-relative+mailto+tel|links=KF8-positioned"
    );

    let broken = br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><a href="../missing.xhtml">ROUND8_BROKEN_LINK</a></body></html>"#;
    let broken_epub = zip_epub(
        &package(
            r#"<item id="one" href="OPS/chapters/one.xhtml" media-type="application/xhtml+xml"/>"#,
            r#"<itemref idref="one"/>"#,
        ),
        &[("OPS/chapters/one.xhtml", broken.to_vec())],
        None,
    );
    let error =
        convert_result(&broken_epub).expect_err("broken cross-document link must be explicit");
    assert!(error.contains("internal link target does not resolve"));
    println!(
        "ROUND8_SAFE_BOUNDARY|id=G3-16|broken-target=explicit-error|successful-output=not-presented"
    );
}

// E2E-ID: E2E-CONTENT-04
// Audit: G5-11, G5-13
#[test]
fn custom_navigation_omits_unlinked_span_and_preserves_hierarchy() {
    // Audit coverage: G5-11, G5-13.
    let manifest = r#"
        <item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>
        <item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>
    "#;
    let body = br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p id="one">ROUND8_NAV_ONE</p><p id="two">ROUND8_NAV_TWO</p></body></html>"#;
    let nav = br#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><span>ROUND8_PART_HEADING</span><ol><li><a href="body.xhtml#one">ROUND8_CHILD_ONE</a></li><li><span>ROUND8_SUBHEADING</span><ol><li><a href="body.xhtml#two">ROUND8_GRANDCHILD</a></li></ol></li></ol></li></ol></nav><nav epub:type="custom-reading-order"><ol><li><a href="body.xhtml#one">ROUND8_CUSTOM_OMITTED</a></li></ol></nav><nav epub:type="landmarks"><ol><li><a epub:type="bodymatter" href="body.xhtml">Body</a></li></ol></nav></body></html>"#;
    let epub = zip_epub(
        &package(manifest, r#"<itemref idref="body"/><itemref idref="nav"/>"#),
        &[("body.xhtml", body.to_vec()), ("nav.xhtml", nav.to_vec())],
        None,
    );
    let result = convert_result(&epub);
    println!(
        "ROUND8_OBSERVATION|ids=G5-11/G5-13|fixture=custom-nav-and-unlinked-span-headings|result={}",
        result
            .as_ref()
            .map(|_| "success")
            .unwrap_or("explicit-error")
    );
    let azw3 = result.expect("navigation hierarchy matrix must convert");
    let output = sections(&azw3);
    for marker in [
        "ROUND8_PART_HEADING",
        "ROUND8_SUBHEADING",
        "ROUND8_CHILD_ONE",
        "ROUND8_GRANDCHILD",
    ] {
        assert!(
            output.contains(marker),
            "navigation marker missing: {marker}"
        );
    }
    for heading in ["ROUND8_PART_HEADING", "ROUND8_SUBHEADING"] {
        let heading_at = output
            .find(heading)
            .unwrap_or_else(|| panic!("heading missing from sections: {heading}"));
        assert!(
            output[heading_at..].contains("<ol"),
            "child list missing after heading {heading}: {output}"
        );
    }
    let part_heading = output.find("ROUND8_PART_HEADING").unwrap();
    let child_one = output.find("ROUND8_CHILD_ONE").unwrap();
    let subheading = output.find("ROUND8_SUBHEADING").unwrap();
    let grandchild = output.find("ROUND8_GRANDCHILD").unwrap();
    assert!(part_heading < child_one && child_one < subheading && subheading < grandchild);
    let ncx_pointer = azw3.mobi().ncx;
    let ncx_report = azw3.index_report(ncx_pointer);
    for record_index in 0..=ncx_report.detail_count {
        assert!(
            !String::from_utf8_lossy(azw3.record(ncx_pointer + record_index))
                .contains("ROUND8_CUSTOM_OMITTED")
        );
    }
    let ncx = azw3.index_report(azw3.mobi().ncx);
    assert_eq!(
        ncx.entry_count, 2,
        "custom nav must not enter the visible NCX"
    );
    assert_eq!(
        azw3.ncx_depths(),
        vec![0, 0],
        "linked descendants remain in the NCX while unlinked headings have no target"
    );
    println!(
        "ROUND8_SAFE_BOUNDARY|id=G5-11|custom-nav=omitted-without-recognized-channel-loss|id=G5-13|unlinked-span-headings=source-hierarchy-retained-linked-descendants-preserved|ncx-depths=0,0"
    );
}
