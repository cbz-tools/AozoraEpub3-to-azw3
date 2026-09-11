//! Kindle CSS writer characterization and control evidence. The dedicated
//! production E2E and safe-rejection assertions are cited by KCSS/G7 rows;
//! KindleGen comparison output itself is diagnostic, not a device contract.

use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::support::{Azw3, zip_epub};
use aozoraepub3_to_azw3::{ConvertOptions, convert_bytes};
use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};

const CSS_MARKER: &str = "KCSS_CHARACTERIZATION_FIXTURE";

#[test]
fn kindle_css_compatibility_production_e2e() {
    let epub = fixture_epub(false);
    let self_bytes = convert_bytes(&epub, &ConvertOptions::default())
        .expect("synthetic Kindle CSS fixture must convert");
    let self_book = Azw3::parse(self_bytes);
    let self_rawml = self_book.rawml();
    let self_fdst = self_book.fdst_ranges();
    let self_css = fdst_secondary_stream_from_ranges(&self_rawml, &self_fdst);
    let self_css = String::from_utf8(self_css).expect("self CSS secondary flows are UTF-8");

    assert!(!self_css.is_empty(), "self AZW3 has no CSS secondary flow");
    assert!(
        !css_contains_property(&self_css, "max-width"),
        "self AZW3 CSS retains an active max-width declaration: {self_css}"
    );
    assert!(
        !css_contains_property(&self_css, "max-height"),
        "self AZW3 CSS retains an active max-height declaration: {self_css}"
    );

    for (property, value) in [
        ("width", "11em"),
        ("height", "12em"),
        ("min-width", "13em"),
        ("min-height", "14em"),
        ("outline-offset", "3px"),
        ("position", "relative"),
        ("foo-max-width", "1em"),
        ("--max-width", "2em"),
    ] {
        assert!(
            css_contains_declaration(&self_css, property, value),
            "self AZW3 CSS lost {property}: {value}: {self_css}"
        );
    }
}

#[test]
#[ignore = "requires KINDLEGEN_EXE"]
fn kindle_css_compatibility_characterization_same_input() {
    let epub = fixture_epub(false);
    let root = temp_root();
    fs::create_dir_all(&root).expect("create characterization temp directory");
    let epub_path = root.join("kcss-characterization.epub");
    let mobi_path = root.join("kcss-characterization.mobi");
    fs::write(&epub_path, &epub).expect("write characterization EPUB");

    let kindlegen = std::env::var_os("KINDLEGEN_EXE")
        .map(PathBuf::from)
        .expect("set KINDLEGEN_EXE to run KindleGen characterization");
    let output = Command::new(&kindlegen)
        .arg(&epub_path)
        .arg("-o")
        .arg("kcss-characterization.mobi")
        .arg("-verbose")
        .current_dir(&root)
        .output()
        .unwrap_or_else(|error| panic!("run KindleGen {}: {error}", kindlegen.display()));
    let log = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    println!("KCSS_KINDLEGEN_LOG\n{log}");
    assert!(
        output.status.success() || mobi_path.is_file(),
        "KindleGen failed with {} and did not create the MOBI",
        output.status
    );

    let kindlegen_bytes = fs::read(&mobi_path).expect("read KindleGen MOBI");
    print_palmdb_summary("KCSS_KINDLEGEN_RECORDS", &kindlegen_bytes);
    let kf8_boundary = find_kf8_boundary(&kindlegen_bytes);
    let kindlegen_palm_doc = palm_doc_header(&kindlegen_bytes, kf8_boundary);
    let kindlegen_rawml = generic_palm_doc_stream(
        &kindlegen_bytes,
        kf8_boundary,
        kindlegen_palm_doc.text_records,
        kindlegen_palm_doc.compression,
    );
    let kindlegen_fdst = find_fdst(
        &kindlegen_bytes,
        kf8_boundary,
        kindlegen_palm_doc.text_records,
    );
    let kindlegen_css = fdst_secondary_stream(&kindlegen_rawml, &kindlegen_fdst);
    println!(
        "KCSS_KINDLEGEN_KF8|boundary-record={kf8_boundary}|text-length={}|text-records={}|decoded-stream-bytes={}|fdst={kindlegen_fdst:?}|rawml-marker={}",
        kindlegen_palm_doc.text_length,
        kindlegen_palm_doc.text_records,
        kindlegen_rawml.len(),
        String::from_utf8_lossy(&kindlegen_rawml).contains(CSS_MARKER)
    );
    for (index, flow) in secondary_flows(&kindlegen_rawml, &kindlegen_fdst)
        .into_iter()
        .enumerate()
    {
        println!(
            "KCSS_KINDLEGEN_FLOW|index={index}|bytes={}|contains-css-marker={}|contains-visited={}",
            flow.len(),
            String::from_utf8_lossy(flow).contains("@charset"),
            String::from_utf8_lossy(flow).contains(":visited")
        );
    }

    let self_bytes = convert_bytes(&epub, &ConvertOptions::default()).expect("convert self EPUB");
    let self_book = Azw3::parse(self_bytes);
    let self_rawml = self_book.rawml();
    let self_fdst = self_book.fdst_ranges();
    let self_css = fdst_secondary_stream_from_ranges(&self_rawml, &self_fdst);
    println!(
        "KCSS_SELF|records={}|text-records={}|rawml-bytes={}|fdst={:?}|rawml-marker={}",
        self_book.record_count(),
        self_book.palm_doc().text_records,
        self_rawml.len(),
        self_fdst,
        String::from_utf8_lossy(&self_rawml).contains(CSS_MARKER)
    );
    print_css_case_report(&log, &kindlegen_css, &self_css);

    let _ = fs::remove_dir_all(root);
}

#[test]
#[ignore = "requires KINDLEGEN_EXE"]
fn kindle_css_compatibility_control_rejection_characterization() {
    let epub = fixture_epub(true);
    let kindlegen_css = kindlegen_secondary_css(&epub);
    let self_error = convert_bytes(&epub, &ConvertOptions::default())
        .expect_err("rejected pseudo-class controls must not convert");
    let self_error = format!("{self_error:?}");
    println!(
        "KCSS_CONTROL_CASE|kindlegen-first-child={}|kindlegen-nth-child={}|self=REJECT|self-error={}",
        kindlegen_css.contains(":first-child") && kindlegen_css.contains("#101000"),
        kindlegen_css.contains(":nth-child(2n+1)") && kindlegen_css.contains("#650000"),
        self_error
    );
    assert!(kindlegen_css.contains(":first-child"));
    assert!(kindlegen_css.contains(":nth-child(2n+1)"));
    assert!(self_error.contains("first-child") && self_error.contains("nth-child"));
}

fn kindlegen_secondary_css(epub: &[u8]) -> String {
    let root = temp_root();
    fs::create_dir_all(&root).expect("create control characterization temp directory");
    let epub_path = root.join("kcss-controls.epub");
    let mobi_path = root.join("kcss-controls.mobi");
    fs::write(&epub_path, epub).expect("write control characterization EPUB");
    let kindlegen = std::env::var_os("KINDLEGEN_EXE")
        .map(PathBuf::from)
        .expect("set KINDLEGEN_EXE to run KindleGen characterization");
    let output = Command::new(&kindlegen)
        .arg(&epub_path)
        .arg("-o")
        .arg("kcss-controls.mobi")
        .arg("-verbose")
        .current_dir(&root)
        .output()
        .unwrap_or_else(|error| panic!("run KindleGen {}: {error}", kindlegen.display()));
    assert!(output.status.success() || mobi_path.is_file());
    let bytes = fs::read(&mobi_path).expect("read control characterization MOBI");
    let boundary = find_kf8_boundary(&bytes);
    let palm_doc = palm_doc_header(&bytes, boundary);
    let rawml = generic_palm_doc_stream(
        &bytes,
        boundary,
        palm_doc.text_records,
        palm_doc.compression,
    );
    let fdst = find_fdst(&bytes, boundary, palm_doc.text_records);
    let css = fdst_secondary_stream(&rawml, &fdst);
    assert!(!css.is_empty(), "KindleGen control fixture has no CSS flow");
    let _ = fs::remove_dir_all(root);
    String::from_utf8(css).expect("KindleGen CSS flow is UTF-8")
}

fn fixture_epub(include_rejected_controls: bool) -> Vec<u8> {
    let document_count = 2;
    let mut document_manifest = String::new();
    let mut document_spine = String::new();
    for index in 0..document_count {
        document_manifest.push_str(&format!(
            "<item id=\"body{index}\" href=\"item/xhtml/body-{index:02}.xhtml\" media-type=\"application/xhtml+xml\"/>"
        ));
        document_spine.push_str(&format!("<itemref idref=\"body{index}\" linear=\"yes\"/>"));
    }
    let package = format!(
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Kindle CSS Characterization</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language></metadata><manifest><item id="style" href="item/style/style.css" media-type="text/css"/><item id="child" href="item/style/child.css" media-type="text/css"/><item id="child2" href="item/style/child2.css" media-type="text/css"/><item id="child3" href="item/style/child3.css" media-type="text/css"/><item id="child4" href="item/style/child4.css" media-type="text/css"/><item id="child5" href="item/style/child5.css" media-type="text/css"/><item id="child6" href="item/style/child6.css" media-type="text/css"/><item id="nav" href="item/xhtml/nav.xhtml" media-type="application/xhtml+xml" properties="nav"/><item id="ncx" href="item/toc.ncx" media-type="application/x-dtbncx+xml"/><item id="cover" href="item/xhtml/cover.xhtml" media-type="application/xhtml+xml"/>{document_manifest}<item id="pixel" href="item/images/pixel.png" media-type="image/png" properties="cover-image"/></manifest><spine toc="ncx"><itemref idref="cover" linear="yes"/>{document_spine}</spine></package>"#
    );
    let mut css = r#"@charset "UTF-8";
@namespace svg url("http://www.w3.org/2000/svg");
@import "child.css";
@import "child2.css";
@import "child3.css";
@import "child4.css";
@import "child5.css";
@import "child6.css";
@page { margin: 1em; }
@supports (display: block) { .kcss-supports { display: block; } }
@media amzn-kf8 { .kcss-media { color: #ab0000; } }
@font-face { font-family: KcssFixture; src: url(data:font/ttf;base64,AAAA); }

p.kcss-first-of-type:first-of-type { color: #110000; }
p.kcss-last-child:last-child { color: #220000; }
p.kcss-last-of-type:last-of-type { color: #330000; }
li.kcss-nth-last-child:nth-last-child(2) { color: #440000; }
p.kcss-nth-last-of-type:nth-last-of-type(2) { color: #550000; }
p.kcss-nth-of-type:nth-of-type(2) { color: #660000; }
p.kcss-only-child:only-child { color: #770000; }
p.kcss-only-of-type:only-of-type { color: #880000; }
a:visited { color: #990000; }
a:link { color: #aa0000; }

.kcss-max-width { max-width: 22em; }
.kcss-max-height { max-height: 30em; }
.kcss-false-positive {
    foo-max-width: 1em;
    --max-width: 2em;
    font-family: "max-width";
    background-image: url(data:image/png;base64,AAAA);
}
.kcss-outline { outline: 1px solid #123456; }
.kcss-outline-color { outline-color: #234567; }
.kcss-outline-style { outline-style: dashed; }
.kcss-outline-width { outline-width: 2px; }
.kcss-width { width: 11em; }
.kcss-height { height: 12em; }
.kcss-min-width { min-width: 13em; }
.kcss-min-height { min-height: 14em; }
.kcss-outline-offset { outline-offset: 3px; }
.kcss-position { position: relative; top: 1px; }
"#
    .to_owned();
    if include_rejected_controls {
        css.push_str(
            "p.kcss-first-child:first-child { color: #101000; }\nli.kcss-nth-child:nth-child(2n+1) { color: #650000; }\n",
        );
    }
    for index in 0..256 {
        css.push_str(&format!(".kcss-filler-{index} {{ color: #000000; }}\n"));
    }
    let child = format!("/* {CSS_MARKER}_CHILD */\n.kcss-import {{ color: #bc0000; }}");
    let nav = r#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><head><title>Navigation</title></head><body><nav epub:type="toc"><ol><li><a href="body-00.xhtml">Body</a></li></ol></nav></body></html>"#;
    let ncx = r#"<ncx xmlns="http://www.daisy.org/z3986/2005/ncx/"><head></head><docTitle><text>Kindle CSS Characterization</text></docTitle><navMap><navPoint id="body" playOrder="1"><navLabel><text>Body</text></navLabel><content src="xhtml/body-00.xhtml"/></navPoint></navMap></ncx>"#;
    let cover = r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><title>Cover</title></head><body><img src="../images/pixel.png" alt="cover"/></body></html>"#;
    let body_filler = (0..256)
        .map(|index| format!("<p>KCSS_BODY_FILLER_{index:03}</p>"))
        .collect::<String>();
    let body = format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><style type="text/css">{css}</style></head><body><div><p class="kcss-first-of-type">first</p><span>span</span><p class="kcss-nth-of-type">second</p><p class="kcss-nth-last-of-type">third</p><em>em</em></div><div><p class="kcss-last-child kcss-last-of-type">last</p></div><div><p class="kcss-only-child kcss-only-of-type">only</p></div><ul><li>one</li><li class="kcss-nth-last-child">two</li><li>three</li></ul><p class="kcss-max-width kcss-max-height kcss-outline kcss-outline-color kcss-outline-style kcss-outline-width kcss-width kcss-height kcss-min-width kcss-min-height kcss-outline-offset kcss-position kcss-supports kcss-media">{CSS_MARKER}</p><img src="../images/pixel.png" alt="pixel"/><a href="https://example.invalid/">link</a>{body_filler}</body></html>"#
    );
    let mut files = vec![
        ("item/style/style.css", css.as_bytes().to_vec()),
        ("item/style/child.css", child.clone().into_bytes()),
        ("item/style/child2.css", child.clone().into_bytes()),
        ("item/style/child3.css", child.clone().into_bytes()),
        ("item/style/child4.css", child.clone().into_bytes()),
        ("item/style/child5.css", child.clone().into_bytes()),
        ("item/style/child6.css", child.into_bytes()),
        ("item/xhtml/nav.xhtml", nav.as_bytes().to_vec()),
        ("item/toc.ncx", ncx.as_bytes().to_vec()),
        ("item/xhtml/cover.xhtml", cover.as_bytes().to_vec()),
        ("item/images/pixel.png", pixel_png()),
    ];
    for index in 0..document_count {
        let path = Box::leak(format!("item/xhtml/body-{index:02}.xhtml").into_boxed_str());
        files.push((path, body.as_bytes().to_vec()));
    }
    zip_epub(&package, &files, None)
}

fn secondary_flows<'a>(
    rawml: &'a [u8],
    fdst: &Option<(usize, Vec<(usize, usize)>)>,
) -> Vec<&'a [u8]> {
    fdst.as_ref()
        .map(|(_, ranges)| {
            ranges
                .iter()
                .skip(1)
                .map(|(start, end)| rawml.get(*start..*end).expect("FDST flow range"))
                .collect()
        })
        .unwrap_or_default()
}

fn fdst_secondary_stream(rawml: &[u8], fdst: &Option<(usize, Vec<(usize, usize)>)>) -> Vec<u8> {
    let mut stream = Vec::new();
    for flow in secondary_flows(rawml, fdst) {
        stream.extend_from_slice(flow);
    }
    stream
}

fn fdst_secondary_stream_from_ranges(rawml: &[u8], ranges: &[(usize, usize)]) -> Vec<u8> {
    let mut stream = Vec::new();
    for (start, end) in ranges.iter().skip(1) {
        stream.extend_from_slice(rawml.get(*start..*end).expect("self FDST flow range"));
    }
    stream
}

fn css_contains_property(css: &str, property: &str) -> bool {
    css.split(['{', '}', ';']).any(|segment| {
        segment
            .trim_start()
            .split_once(':')
            .is_some_and(|(name, _)| name.trim().eq_ignore_ascii_case(property))
    })
}

fn css_contains_declaration(css: &str, property: &str, value: &str) -> bool {
    css.split(['{', '}', ';']).any(|segment| {
        segment
            .trim_start()
            .split_once(':')
            .is_some_and(|(name, declaration_value)| {
                name.trim().eq_ignore_ascii_case(property) && declaration_value.trim() == value
            })
    })
}

#[derive(Clone, Copy)]
struct CssCase {
    id: &'static str,
    marker: &'static str,
    property: Option<&'static str>,
    value: Option<&'static str>,
    diagnostic: &'static str,
}

fn print_css_case_report(log: &str, kindlegen_css: &[u8], self_css: &[u8]) {
    let kindlegen_css = String::from_utf8_lossy(kindlegen_css);
    let self_css = String::from_utf8_lossy(self_css);
    let cases = [
        CssCase {
            id: "KCSS-01",
            marker: ".kcss-max-width",
            property: Some("max-width"),
            value: Some("22em"),
            diagnostic: "W28001",
        },
        CssCase {
            id: "KCSS-02",
            marker: ".kcss-max-height",
            property: Some("max-height"),
            value: Some("30em"),
            diagnostic: "W28001",
        },
        CssCase {
            id: "KCSS-03",
            marker: ".kcss-outline",
            property: Some("outline"),
            value: Some("1px solid #123456"),
            diagnostic: "none",
        },
        CssCase {
            id: "KCSS-04",
            marker: ".kcss-outline-color",
            property: Some("outline-color"),
            value: Some("#234567"),
            diagnostic: "none",
        },
        CssCase {
            id: "KCSS-05",
            marker: ".kcss-outline-style",
            property: Some("outline-style"),
            value: Some("dashed"),
            diagnostic: "none",
        },
        CssCase {
            id: "KCSS-06",
            marker: ".kcss-outline-width",
            property: Some("outline-width"),
            value: Some("2px"),
            diagnostic: "none",
        },
        CssCase {
            id: "KCSS-14",
            marker: ":first-of-type",
            property: Some("color"),
            value: Some("#110000"),
            diagnostic: "none",
        },
        CssCase {
            id: "KCSS-15",
            marker: ":last-child",
            property: Some("color"),
            value: Some("#220000"),
            diagnostic: "none",
        },
        CssCase {
            id: "KCSS-16",
            marker: ":last-of-type",
            property: Some("color"),
            value: Some("#330000"),
            diagnostic: "none",
        },
        CssCase {
            id: "KCSS-17",
            marker: ":nth-last-child(2)",
            property: Some("color"),
            value: Some("#440000"),
            diagnostic: "none",
        },
        CssCase {
            id: "KCSS-18",
            marker: ":nth-last-of-type(2)",
            property: Some("color"),
            value: Some("#550000"),
            diagnostic: "none",
        },
        CssCase {
            id: "KCSS-19",
            marker: ":nth-of-type(2)",
            property: Some("color"),
            value: Some("#660000"),
            diagnostic: "none",
        },
        CssCase {
            id: "KCSS-20",
            marker: ":only-child",
            property: Some("color"),
            value: Some("#770000"),
            diagnostic: "none",
        },
        CssCase {
            id: "KCSS-21",
            marker: ":only-of-type",
            property: Some("color"),
            value: Some("#880000"),
            diagnostic: "none",
        },
        CssCase {
            id: "KCSS-22",
            marker: ":visited",
            property: Some("color"),
            value: Some("#990000"),
            diagnostic: "none",
        },
        CssCase {
            id: "KCSS-27",
            marker: ".kcss-width",
            property: Some("width"),
            value: Some("11em"),
            diagnostic: "none",
        },
        CssCase {
            id: "KCSS-28",
            marker: ".kcss-height",
            property: Some("height"),
            value: Some("12em"),
            diagnostic: "none",
        },
        CssCase {
            id: "KCSS-29",
            marker: ".kcss-min-width",
            property: Some("min-width"),
            value: Some("13em"),
            diagnostic: "none",
        },
        CssCase {
            id: "KCSS-30",
            marker: ".kcss-min-height",
            property: Some("min-height"),
            value: Some("14em"),
            diagnostic: "none",
        },
        CssCase {
            id: "KCSS-31",
            marker: ".kcss-outline-offset",
            property: Some("outline-offset"),
            value: Some("3px"),
            diagnostic: "none",
        },
        CssCase {
            id: "KCSS-32",
            marker: ".kcss-position",
            property: Some("position"),
            value: Some("relative"),
            diagnostic: "W28003",
        },
        CssCase {
            id: "KCSS-33",
            marker: ":link",
            property: Some("color"),
            value: Some("#aa0000"),
            diagnostic: "none",
        },
        CssCase {
            id: "KCSS-34",
            marker: "@charset",
            property: None,
            value: None,
            diagnostic: "I10004",
        },
        CssCase {
            id: "KCSS-35",
            marker: "@font-face",
            property: None,
            value: None,
            diagnostic: "I10004",
        },
        CssCase {
            id: "KCSS-36",
            marker: "@import",
            property: None,
            value: None,
            diagnostic: "I10004",
        },
        CssCase {
            id: "KCSS-37",
            marker: "@media",
            property: None,
            value: None,
            diagnostic: "I10004",
        },
        CssCase {
            id: "KCSS-38",
            marker: "@page",
            property: None,
            value: None,
            diagnostic: "I10004",
        },
        CssCase {
            id: "KCSS-39",
            marker: "@namespace",
            property: None,
            value: None,
            diagnostic: "I10004",
        },
        CssCase {
            id: "KCSS-40",
            marker: "@supports",
            property: None,
            value: None,
            diagnostic: "I10004",
        },
    ];
    for case in cases {
        let expected = case
            .property
            .zip(case.value)
            .is_some_and(|(property, value)| {
                kindlegen_css.contains(case.marker)
                    && kindlegen_css.contains(&format!("{property}: {value}"))
            });
        let self_present = case
            .property
            .zip(case.value)
            .is_some_and(|(property, value)| {
                self_css.contains(case.marker) && self_css.contains(&format!("{property}: {value}"))
            });
        let kgen_behavior = if case.property.is_none() {
            if kindlegen_css.contains(case.marker) {
                "PRESERVE"
            } else {
                "REMOVE"
            }
        } else if expected {
            "PRESERVE"
        } else if kindlegen_css.contains(case.marker) {
            "REMOVE"
        } else {
            "REJECT"
        };
        let self_behavior = if case.property.is_none() {
            if self_css.contains(case.marker) {
                "PRESERVE"
            } else {
                "REMOVE"
            }
        } else if self_present {
            "PRESERVE"
        } else if self_css.contains(case.marker) {
            "REMOVE"
        } else {
            "REJECT"
        };
        let diagnostic = if case.diagnostic == "none" || !log.contains(case.diagnostic) {
            "none"
        } else {
            case.diagnostic
        };
        println!(
            "KCSS_CASE|id={}|diagnostic={}|kindlegen={}|self={}|same-input={}|kindlegen-rule-presence={}|self-rule-presence={}",
            case.id,
            diagnostic,
            kgen_behavior,
            self_behavior,
            if kgen_behavior == self_behavior {
                "MATCH"
            } else {
                "DIFFERENCE"
            },
            if kindlegen_css.contains(case.marker) {
                "present"
            } else {
                "absent"
            },
            if self_css.contains(case.marker) {
                "present"
            } else {
                "absent"
            },
        );
    }
}

fn pixel_png() -> Vec<u8> {
    let image = RgbaImage::from_pixel(1, 1, Rgba([0, 128, 255, 255]));
    let mut output = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut output, ImageFormat::Png)
        .expect("encode characterization PNG");
    output.into_inner()
}

fn temp_root() -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before UNIX_EPOCH")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "kcss-characterization-{}-{now}",
        std::process::id()
    ))
}

fn print_palmdb_summary(label: &str, bytes: &[u8]) {
    let offsets = palmdb_offsets(bytes);
    println!(
        "{label}|file-bytes={}|record-count={}",
        bytes.len(),
        offsets.len()
    );
    for (index, start) in offsets.iter().copied().enumerate() {
        let end = offsets.get(index + 1).copied().unwrap_or(bytes.len());
        let record = &bytes[start..end];
        let header = if record.len() >= 20 {
            format!(
                "compression={}|text-length={}|text-records={}|record-size={}|magic={}",
                u16be(record, 0),
                u32be(record, 4),
                u16be(record, 8),
                u16be(record, 10),
                String::from_utf8_lossy(&record[16..20])
            )
        } else {
            String::from("short-record")
        };
        let marker = String::from_utf8_lossy(record).contains(CSS_MARKER);
        println!(
            "{label}|record={index}|offset={start}|bytes={}|{header}|marker={marker}",
            record.len()
        );
    }
}

fn palmdb_offsets(bytes: &[u8]) -> Vec<usize> {
    assert!(bytes.len() >= 78, "PalmDB header is truncated");
    let count = u16be(bytes, 76) as usize;
    (0..count)
        .map(|index| u32be(bytes, 78 + index * 8) as usize)
        .collect()
}

fn u16be(bytes: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes(bytes[offset..offset + 2].try_into().expect("u16 field"))
}

fn u32be(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(bytes[offset..offset + 4].try_into().expect("u32 field"))
}

fn generic_palm_doc_stream(
    bytes: &[u8],
    boundary: usize,
    text_records: usize,
    compression: u16,
) -> Vec<u8> {
    let offsets = palmdb_offsets(bytes);
    let mut rawml = Vec::new();
    for index in 1..=text_records {
        let record = kindlegen_text_payload(palmdb_record(bytes, &offsets, boundary + index));
        match compression {
            1 => rawml.extend_from_slice(record),
            2 => {
                let mut expanded_record = Vec::new();
                palm_doc_decompress(record, &mut expanded_record);
                rawml.extend_from_slice(&expanded_record);
            }
            compression => panic!("unsupported KindleGen PalmDOC compression {compression}"),
        }
    }
    rawml
}

fn kindlegen_text_payload(record: &[u8]) -> &[u8] {
    let (tbs_start, _) = text_trailing_bounds(record);
    let overlap_len = usize::from(record[tbs_start - 1] & 3);
    let payload_end = tbs_start
        .checked_sub(1 + overlap_len)
        .expect("KindleGen text overlap crosses TBS marker");
    &record[..payload_end]
}

fn text_trailing_bounds(record: &[u8]) -> (usize, usize) {
    assert!(!record.is_empty(), "empty KindleGen text record");
    let mut value = 0usize;
    let mut shift = 0;
    let mut cursor = record.len();
    loop {
        cursor -= 1;
        let byte = record[cursor];
        value |= usize::from(byte & 0x7f) << shift;
        if byte & 0x80 != 0 {
            break;
        }
        shift += 7;
        assert!(
            shift < usize::BITS as usize,
            "KindleGen text TBS VWI is too long"
        );
        assert!(cursor > 0, "KindleGen text TBS VWI is unterminated");
    }
    assert!(
        value <= record.len(),
        "KindleGen text TBS is outside record"
    );
    let tbs_start = record.len() - value;
    assert!(tbs_start > 0, "KindleGen text TBS entry has no marker");
    (tbs_start, cursor)
}

fn palm_doc_decompress(input: &[u8], output: &mut Vec<u8>) {
    let mut cursor = 0;
    while cursor < input.len() {
        let byte = input[cursor];
        cursor += 1;
        match byte {
            0x00..=0x08 => {
                let count = byte as usize;
                output.extend_from_slice(
                    input
                        .get(cursor..cursor + count)
                        .expect("PalmDOC literal run"),
                );
                cursor += count;
            }
            0x09..=0x7f => output.push(byte),
            0x80..=0xbf => {
                let next = *input.get(cursor).expect("PalmDOC back-reference");
                cursor += 1;
                let distance = ((byte as usize & 0x3f) << 5) | (next as usize >> 3);
                let length = (next & 0x07) as usize + 3;
                let start = output
                    .len()
                    .checked_sub(distance)
                    .expect("PalmDOC back-reference distance");
                for index in 0..length {
                    output.push(output[start + index]);
                }
            }
            0xc0..=0xff => {
                output.push(b' ');
                output.push(byte ^ 0x80);
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct PalmDocHeader {
    compression: u16,
    text_length: usize,
    text_records: usize,
}

fn palm_doc_header(bytes: &[u8], record_index: usize) -> PalmDocHeader {
    let offsets = palmdb_offsets(bytes);
    let record = palmdb_record(bytes, &offsets, record_index);
    PalmDocHeader {
        compression: u16be(record, 0),
        text_length: u32be(record, 4) as usize,
        text_records: u16be(record, 8) as usize,
    }
}

fn find_kf8_boundary(bytes: &[u8]) -> usize {
    let offsets = palmdb_offsets(bytes);
    (0..offsets.len())
        .find(|index| {
            let record = palmdb_record(bytes, &offsets, *index);
            record.get(16..20) == Some(b"MOBI") && u32be(record, 16 + 0x14) == 8
        })
        .expect("KindleGen dual-format MOBI has no V8 MOBI header")
}

fn find_fdst(
    bytes: &[u8],
    kf8_boundary: usize,
    text_records: usize,
) -> Option<(usize, Vec<(usize, usize)>)> {
    let offsets = palmdb_offsets(bytes);
    let first_non_text = kf8_boundary + 1 + text_records;
    (first_non_text..offsets.len()).find_map(|record_index| {
        let record = palmdb_record(bytes, &offsets, record_index);
        if record.get(..4) != Some(b"FDST") {
            return None;
        }
        let count = u32be(record, 8) as usize;
        let ranges = (0..count)
            .map(|index| {
                (
                    u32be(record, 12 + index * 8) as usize,
                    u32be(record, 16 + index * 8) as usize,
                )
            })
            .collect::<Vec<_>>();
        Some((record_index, ranges))
    })
}

fn palmdb_record<'a>(bytes: &'a [u8], offsets: &[usize], index: usize) -> &'a [u8] {
    let end = offsets.get(index + 1).copied().unwrap_or(bytes.len());
    &bytes[offsets[index]..end]
}
