//! Primary audit coverage: KCSS-01..KCSS-06, KCSS-27..KCSS-40;
//! formal converter behavior for CSS writer transport and active stylesheet graphs.

use crate::support::{Azw3, zip_epub};
use aozoraepub3_to_azw3::{ConvertOptions, convert_bytes};

const WRITER_MARKER: &str = "KCSS_ROUND2";

#[test]
fn style_attribute_projection_preserves_supported_declarations() {
    let body = r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><style type="text/css">.sheet-epub { -epub-writing-mode: vertical-rl; -epub-text-combine: horizontal; -epub-text-emphasis-style: sesame; }</style></head><body><p id="inline-max" style="max-width:22em; max-height:30em; width:11em; min-width:13em; background-image:url(&quot;data:image/png;base64,QUJD&quot;)" >inline max</p><p id="inline-epub" style="-epub-writing-mode:vertical-rl; -epub-text-combine:horizontal; -epub-text-emphasis-style:sesame;">inline epub</p></body></html>"#;
    let epub = css_writer_fixture(body, &[]);
    let book = convert(&epub).expect("style attribute fixture must convert");
    let sections = reconstructed_sections(&book);
    let inline_max = sections
        .iter()
        .find(|section| section.contains("id=\"inline-max\""))
        .expect("inline max paragraph must survive RawML reconstruction");
    let inline_epub = sections
        .iter()
        .find(|section| section.contains("id=\"inline-epub\""))
        .expect("inline epub paragraph must survive RawML reconstruction");
    let css = secondary_css(&book);

    assert!(!inline_max.contains("max-width"));
    assert!(!inline_max.contains("max-height"));
    assert!(inline_max.contains("width:11em"));
    assert!(inline_max.contains("min-width:13em"));
    assert!(inline_max.contains("data:image/png;base64,QUJD"));
    assert!(!inline_epub.contains("-epub-writing-mode"));
    assert!(!inline_epub.contains("-epub-text-combine"));
    assert!(!inline_epub.contains("-epub-text-emphasis-style"));
    assert!(inline_epub.contains("-webkit-writing-mode:vertical-rl"));
    assert!(inline_epub.contains("-webkit-text-combine:horizontal"));
    assert!(inline_epub.contains("-webkit-text-emphasis-style:sesame"));
    assert!(css.contains("-webkit-writing-mode: vertical-rl"));
    assert!(css.contains("-webkit-text-combine: horizontal"));
    assert!(css.contains("-webkit-text-emphasis-style: sesame"));
    assert!(!css.contains("-epub-writing-mode: vertical-rl"));
    println!(
        "{WRITER_MARKER}_A|inline-max-width=REMOVED|inline-max-height=REMOVED|inline-width=PRESERVE|inline-min-width=PRESERVE|inline-epub=WEBKIT|inline-url=PRESERVED|stylesheet-epub=WEBKIT"
    );
}

#[test]
fn comment_prefixed_declarations_are_projected_safely() {
    let css = r#".comment-before {
    /* marker-a */
    max-width: 22em;

    /* marker-b */
    max-height: 30em;
}
.comment-in-name {
    max-width/**/: 23em;
    max-height/**/: 31em;
}
.comment-epub {
    /* marker-c */
    -epub-writing-mode: vertical-rl;
}
.clean-epub { -epub-writing-mode: vertical-rl; }
"#;
    let body = r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><link rel="stylesheet" href="round2.css"/></head><body><p>comment fixture</p></body></html>"#;
    let epub = css_writer_fixture(body, &[("round2", "round2.css", css)]);
    let book = convert(&epub).expect("comment fixture must convert");
    let projected = secondary_css(&book);

    assert!(projected.contains("marker-a"));
    assert!(!projected.contains("max-width: 22em"));
    assert!(projected.contains("marker-b"));
    assert!(!projected.contains("max-height: 30em"));
    assert!(!projected.contains("max-width/**/: 23em"));
    assert!(!projected.contains("max-height/**/: 31em"));
    assert!(projected.contains("marker-c"));
    assert!(!projected.contains("-epub-writing-mode: vertical-rl"));
    assert!(projected.contains("-webkit-writing-mode: vertical-rl"));
    println!(
        "{WRITER_MARKER}_B|comment-before-max=REMOVED|comment-in-name-max=REMOVED|comment-before-epub=WEBKIT|clean-epub=WEBKIT|comments=RETAINED"
    );
}

#[test]
fn function_parentheses_and_semicolons_preserve_following_declarations() {
    let css = r#".function-semicolon {
    background-image: url(data:text/plain;charset=utf-8,abc);
    color: #123456;
    max-width: 22em;
    width: 11em;
}
"#;
    let body = r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><link rel="stylesheet" href="round2.css"/></head><body><p>function fixture</p></body></html>"#;
    let epub = css_writer_fixture(body, &[("round2", "round2.css", css)]);
    let book = convert(&epub).expect("function semicolon fixture must convert");
    let projected = secondary_css(&book);
    assert!(projected.contains("background-image: url(data:text/plain"));
    assert!(projected.contains("color: #123456"));
    assert!(projected.contains("width: 11em"));
    assert!(!projected.contains("max-width: 22em"));
    assert!(projected.contains("charset=utf-8,abc"));
    assert!(!projected.contains("color: #123456;\n    ;\n    width: 11em"));
    assert!(!projected.contains("\n    ;\n"));
    println!(
        "{WRITER_MARKER}_C|fixture=url(data:text/plain;charset=utf-8,abc)|accepted=true|function-value=PRESERVED|following-declarations=RETAINED|empty-declaration=ABSENT|max-width=REMOVED"
    );
}

#[test]
fn active_stylesheet_graph_controls_css_validation() {
    let body = r#"<html xmlns="http://www.w3.org/1999/xhtml"><head></head><body><p>unused stylesheet fixture</p></body></html>"#;
    let active = css_writer_fixture(body, &[]);
    let unsupported_css = "p:first-child { color: red; }";
    let unused_reject = css_writer_fixture(body, &[("unused", "unused.css", unsupported_css)]);
    assert!(convert(&active).is_ok());
    assert!(convert(&unused_reject).is_ok());
    let active_reject_body = r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><link rel="stylesheet" href="unused.css"/></head><body><p>active stylesheet fixture</p></body></html>"#;
    let active_reject = css_writer_fixture(
        active_reject_body,
        &[("unused", "unused.css", unsupported_css)],
    );
    let error = convert_bytes(&active_reject, &ConvertOptions::default())
        .expect_err("active unsupported stylesheet must remain rejected");
    assert!(error.to_string().contains("first-child"));
    println!(
        "{WRITER_MARKER}_D1|unused-unsupported-selector=IGNORED|active-unsupported-selector=REJECT"
    );

    let unused_layout = css_writer_fixture(
        body,
        &[(
            "unused",
            "unused.css",
            ".vrtl { writing-mode: vertical-rl; }",
        )],
    );
    let horizontal = convert(&active).expect("active baseline must convert");
    let vertical = convert(&unused_layout).expect("unused writing-mode stylesheet must convert");
    let horizontal_rawml = String::from_utf8_lossy(&horizontal.rawml()).into_owned();
    let vertical_rawml = String::from_utf8_lossy(&vertical.rawml()).into_owned();
    assert!(!horizontal_rawml.contains("writing-mode: vertical-rl"));
    assert!(!vertical_rawml.contains("writing-mode: vertical-rl"));
    assert!(!vertical_rawml.contains(".vrtl"));
    assert_eq!(
        horizontal_rawml.contains("vertical-rl"),
        vertical_rawml.contains("vertical-rl")
    );
    let active_layout_body = r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><link rel="stylesheet" href="unused.css"/></head><body><p>active layout fixture</p></body></html>"#;
    let active_layout = css_writer_fixture(
        active_layout_body,
        &[(
            "unused",
            "unused.css",
            ".vrtl { writing-mode: vertical-rl; }",
        )],
    );
    let active_vertical =
        convert(&active_layout).expect("active writing-mode stylesheet must convert");
    let active_vertical_rawml = String::from_utf8_lossy(&active_vertical.rawml()).into_owned();
    assert!(active_vertical_rawml.contains("writing-mode: vertical-rl"));
    assert_eq!(
        horizontal.exth().text(525).as_deref(),
        Some("horizontal-lr")
    );
    assert_eq!(vertical.exth().text(525).as_deref(), Some("horizontal-lr"));
    assert_eq!(
        active_vertical.exth().text(525).as_deref(),
        Some("vertical-rl")
    );
    println!(
        "{WRITER_MARKER}_D2|unused-writing-mode=IGNORED|active-writing-mode=VERTICAL|baseline=horizontal"
    );

    let unused_import = css_writer_fixture(
        body,
        &[
            (
                "unused",
                "unused.css",
                "@import \"another-unused.css\"; .unused-import { color: red; }",
            ),
            (
                "another",
                "another-unused.css",
                ".another-unused-marker { color: blue; }",
            ),
        ],
    );
    let imported_book = convert(&unused_import).expect("unused @import graph must convert");
    let imported_css = secondary_css(&imported_book);
    assert!(!imported_css.contains("unused-import"));
    assert!(!imported_css.contains("another-unused-marker"));
    let active_import_body = r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><link rel="stylesheet" href="root.css"/></head><body><p>active import fixture</p></body></html>"#;
    let active_import = css_writer_fixture(
        active_import_body,
        &[
            (
                "root",
                "root.css",
                "@import \"nested/child.css\"; .active-root-marker { color: red; }",
            ),
            (
                "child",
                "nested/child.css",
                ".active-import-marker { color: blue; }",
            ),
        ],
    );
    let active_imported_book = convert(&active_import).expect("active @import graph must convert");
    let active_imported_css = secondary_css(&active_imported_book);
    assert!(active_imported_css.contains("active-root-marker"));
    assert!(active_imported_css.contains("active-import-marker"));
    println!(
        "{WRITER_MARKER}_D3|unused-import-graph=NOT-EMITTED|active-import-graph=EMITTED|conversion=SUCCESS"
    );
}

fn convert(epub: &[u8]) -> Result<Azw3, String> {
    convert_bytes(epub, &ConvertOptions::default())
        .map(Azw3::parse)
        .map_err(|error| error.to_string())
}

fn css_writer_fixture(body: &str, resources: &[(&str, &str, &str)]) -> Vec<u8> {
    let mut manifest =
        String::from(r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#);
    let mut files = vec![("body.xhtml", body.as_bytes().to_vec())];
    for (id, href, source) in resources {
        manifest.push_str(&format!(
            r#"<item id="{id}" href="{href}" media-type="text/css"/>"#
        ));
        files.push((*href, source.as_bytes().to_vec()));
    }
    let package = format!(
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/ops" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Kindle CSS Round2 Characterization</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language></metadata><manifest>{manifest}</manifest><spine><itemref idref="body"/></spine></package>"#
    );
    zip_epub(&package, &files, None)
}

fn reconstructed_sections(book: &Azw3) -> Vec<String> {
    book.reconstructed_xhtml_sections()
        .into_iter()
        .map(|section| String::from_utf8_lossy(&section).into_owned())
        .collect()
}

fn secondary_css(book: &Azw3) -> String {
    let rawml = book.rawml();
    book.fdst_ranges()
        .iter()
        .skip(1)
        .map(|(start, end)| String::from_utf8_lossy(&rawml[*start..*end]).into_owned())
        .collect::<Vec<_>>()
        .join("\n")
}
