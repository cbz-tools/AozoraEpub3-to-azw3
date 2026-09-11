//! Primary audit coverage: G1-03, G1-04, G1-08..G1-11, G2-12, G2-17..G2-20.

use std::io::{Cursor, Write};

use crate::support::{Azw3, zip_epub};
use aozoraepub3_to_azw3::{ConvertOptions, convert_bytes};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

const BASELINE_MARKER: &str = "EPUB33_ROUND7A_BASELINE";
const NAV_TOC_MARKER: &str = "NAV_TOC_MARKER";
const KNOWN_NAV_PROPERTY_MARKER: &str = "KNOWN_NAV_PROPERTY_MARKER";

fn metadata() -> &'static str {
    r#"<dc:title>Main Title</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language>"#
}

fn package(manifest: &str, spine: &str, package_children: &str) -> String {
    package_with_metadata_children("", manifest, spine, package_children)
}

fn package_with_metadata_children(
    metadata_children: &str,
    manifest: &str,
    spine: &str,
    package_children: &str,
) -> String {
    format!(
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/">{}{}</metadata><manifest>{}</manifest><spine>{}</spine>{}</package>"#,
        metadata(),
        metadata_children,
        manifest,
        spine,
        package_children
    )
}

fn body(marker: &str, extra: &str) -> Vec<u8> {
    format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>{marker}</p>{extra}</body></html>"#
    )
    .into_bytes()
}

fn convert_result(epub: &[u8]) -> Result<Azw3, String> {
    convert_bytes(epub, &ConvertOptions::default())
        .map(Azw3::parse)
        .map_err(|error| error.to_string())
}

fn sections(azw3: &Azw3) -> String {
    azw3.reconstructed_xhtml_sections()
        .into_iter()
        .map(|section| String::from_utf8_lossy(&section).into_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

fn rawml(azw3: &Azw3) -> String {
    String::from_utf8_lossy(&azw3.rawml()).into_owned()
}

fn serialized_contains(azw3: &Azw3, marker: &str) -> bool {
    azw3.bytes
        .windows(marker.len())
        .any(|window| window == marker.as_bytes())
}

fn assert_baseline(azw3: &Azw3, fixture: &str) {
    let sections = sections(azw3);
    assert!(
        sections.contains(BASELINE_MARKER),
        "baseline marker missing for {fixture}: {sections}"
    );
}

fn print_result(ids: &str, fixture: &str, result: &Result<Azw3, String>) {
    match result {
        Ok(azw3) => println!(
            "ROUND7A_OBSERVATION|ids={ids}|fixture={fixture}|result=success|sections={}|rawml-bytes={}|guide-entries={}",
            azw3.reconstructed_xhtml_sections().len(),
            azw3.rawml().len(),
            azw3.guide_targets().len(),
        ),
        Err(error) => println!(
            "ROUND7A_OBSERVATION|ids={ids}|fixture={fixture}|result=explicit-error|error={error}"
        ),
    }
}

fn write_entry<W: Write + std::io::Seek>(zip: &mut ZipWriter<W>, name: &str, data: &[u8]) {
    let stored = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Stored);
    zip.start_file(name, stored)
        .expect("start synthetic EPUB entry");
    zip.write_all(data).expect("write synthetic EPUB entry");
}

fn shaped_epub(
    mimetype: Option<&[u8]>,
    mimetype_first: bool,
    mimetype_compression: zip::CompressionMethod,
    container: &str,
    entries: &[(&str, Vec<u8>)],
) -> Vec<u8> {
    let mut output = Cursor::new(Vec::new());
    let mut zip = ZipWriter::new(&mut output);
    let mimetype_options = SimpleFileOptions::default().compression_method(mimetype_compression);

    if mimetype_first {
        if let Some(mimetype) = mimetype {
            zip.start_file("mimetype", mimetype_options)
                .expect("start synthetic mimetype entry");
            zip.write_all(mimetype)
                .expect("write synthetic mimetype entry");
        }
    }
    write_entry(&mut zip, "META-INF/container.xml", container.as_bytes());
    for (name, data) in entries {
        write_entry(&mut zip, name, data);
    }
    if !mimetype_first {
        if let Some(mimetype) = mimetype {
            zip.start_file("mimetype", mimetype_options)
                .expect("start late synthetic mimetype entry");
            zip.write_all(mimetype)
                .expect("write late synthetic mimetype entry");
        }
    }
    zip.finish().expect("finish synthetic EPUB ZIP");
    output.into_inner()
}

fn canonical_container(rootfiles: &str) -> String {
    format!(
        r#"<container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0"><rootfiles>{rootfiles}</rootfiles></container>"#
    )
}

// E2E-ID: E2E-OCF-01
// Audit: G1-01, G1-02, G1-03
#[test]
fn multiple_rootfiles_use_the_default_record_selection_boundary() {
    // Audit coverage: G1-03.
    let primary_package = package(
        r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#,
        r#"<itemref idref="body"/>"#,
        "",
    );
    let alternate_package = package(
        r#"<item id="body" href="alternate.xhtml" media-type="application/xhtml+xml"/>"#,
        r#"<itemref idref="body"/>"#,
        "",
    );
    let container = canonical_container(
        r#"<rootfile full-path="package.opf" media-type="application/oebps-package+xml"/><rootfile full-path="alternate.opf" media-type="application/oebps-package+xml"/>"#,
    );
    let epub = shaped_epub(
        Some(b"application/epub+zip"),
        true,
        zip::CompressionMethod::Stored,
        &container,
        &[
            ("package.opf", primary_package.into_bytes()),
            ("alternate.opf", alternate_package.into_bytes()),
            (
                "body.xhtml",
                body(BASELINE_MARKER, "<p>PRIMARY_ROOTFILE_MARKER</p>"),
            ),
            ("alternate.xhtml", body("ALTERNATE_ROOTFILE_MARKER", "")),
        ],
    );

    let result = convert_result(&epub);
    print_result("G1-03", "multiple-rootfiles", &result);
    match result {
        Ok(azw3) => {
            let sections = sections(&azw3);
            assert!(sections.contains(BASELINE_MARKER));
            assert!(sections.contains("PRIMARY_ROOTFILE_MARKER"));
            assert!(!sections.contains("ALTERNATE_ROOTFILE_MARKER"));
            assert_eq!(azw3.exth().text(503).as_deref(), Some("Main Title"));
            println!(
                "ROUND7A_SAFE_PROJECTION|id=G1-03|evidence=first-rootfile-selected-as-default-without-merging-alternate"
            );
        }
        Err(error) => panic!(
            "G1-03 multiple-rootfiles unexpectedly failed; expected first rootfile selection: {error}"
        ),
    }
}

// E2E-ID: E2E-OCF-02
// Audit: G1-02, G1-04
#[test]
fn mimetype_shapes_remain_outside_publication_semantics() {
    // Audit coverage: G1-04.
    let package = package(
        r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#,
        r#"<itemref idref="body"/>"#,
        "",
    );
    let cases = [
        (
            "canonical",
            Some(b"application/epub+zip".as_slice()),
            true,
            zip::CompressionMethod::Stored,
        ),
        (
            "wrong-value",
            Some(b"application/zip".as_slice()),
            true,
            zip::CompressionMethod::Stored,
        ),
        ("missing", None, true, zip::CompressionMethod::Stored),
        (
            "late-entry",
            Some(b"application/epub+zip".as_slice()),
            false,
            zip::CompressionMethod::Stored,
        ),
        (
            "compressed",
            Some(b"application/epub+zip".as_slice()),
            true,
            zip::CompressionMethod::Deflated,
        ),
    ];

    for (fixture, mimetype, mimetype_first, compression) in cases {
        let epub = shaped_epub(
            mimetype,
            mimetype_first,
            compression,
            &canonical_container(
                r#"<rootfile full-path="package.opf" media-type="application/oebps-package+xml"/>"#,
            ),
            &[
                ("package.opf", package.as_bytes().to_vec()),
                ("body.xhtml", body(BASELINE_MARKER, "")),
            ],
        );
        let result = convert_result(&epub);
        print_result("G1-04", fixture, &result);
        let azw3 = result.unwrap_or_else(|error| panic!("{fixture} unexpectedly failed: {error}"));
        assert_baseline(&azw3, fixture);
        assert_eq!(azw3.exth().text(503).as_deref(), Some("Main Title"));
        println!(
            "ROUND7A_SAFE_OMISSION|id=G1-04|fixture={fixture}|evidence=mimetype-validation-shape-does-not-change-publication-semantic-output"
        );
    }
}

// E2E-ID: E2E-OCF-03
// Audit: G1-01, G1-08, G1-09, G1-10, G1-11
#[test]
fn ocf_companion_metadata_is_ignored_without_body_loss() {
    // Audit coverage: G1-08..G1-11.
    let companion_cases = [
        (
            "G1-08",
            "META-INF/manifest.xml",
            b"<manifest>MANIFEST_COMPANION_MARKER</manifest>".to_vec(),
            "MANIFEST_COMPANION_MARKER",
        ),
        (
            "G1-09",
            "META-INF/metadata.xml",
            br#"<metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>CONTAINER_TITLE_MARKER</dc:title></metadata>"#.to_vec(),
            "CONTAINER_TITLE_MARKER",
        ),
        (
            "G1-10",
            "META-INF/rights.xml",
            b"<rights>RIGHTS_COMPANION_MARKER</rights>".to_vec(),
            "RIGHTS_COMPANION_MARKER",
        ),
        (
            "G1-11",
            "META-INF/signatures.xml",
            b"<signatures>SIGNATURES_COMPANION_MARKER</signatures>".to_vec(),
            "SIGNATURES_COMPANION_MARKER",
        ),
    ];

    let package = package(
        r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#,
        r#"<itemref idref="body"/>"#,
        "",
    );
    for (id, path, companion, marker) in companion_cases {
        let epub = zip_epub(
            &package,
            &[("body.xhtml", body(BASELINE_MARKER, "")), (path, companion)],
            None,
        );
        let result = convert_result(&epub);
        print_result(id, path, &result);
        let azw3 = result.unwrap_or_else(|error| panic!("{id} unexpectedly failed: {error}"));
        assert_baseline(&azw3, path);
        assert_eq!(azw3.exth().text(503).as_deref(), Some("Main Title"));
        assert!(!sections(&azw3).contains(marker));
        assert!(!rawml(&azw3).contains(marker));
        assert!(!serialized_contains(&azw3, marker));
        println!(
            "ROUND7A_SAFE_OMISSION|id={id}|evidence=OCF-companion-not-projected-and-body-preserved"
        );
    }
}

// E2E-ID: E2E-OCF-08
// Audit: G2-12
#[test]
fn package_link_metadata_is_not_a_resource() {
    // Audit coverage: G2-12.
    let cases = [
        (
            "external",
            r#"<link rel="record" href="https://example.invalid/record.json" media-type="application/json"/>"#,
            None,
        ),
        (
            "local",
            r#"<link rel="record" href="record.json" media-type="application/json"/>"#,
            Some(("record.json", b"PACKAGE_LINK_TARGET_MARKER".to_vec())),
        ),
    ];
    for (fixture, link, extra) in cases {
        let package = package_with_metadata_children(
            link,
            r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#,
            r#"<itemref idref="body"/>"#,
            "",
        );
        let files = match extra {
            Some((name, data)) => vec![("body.xhtml", body(BASELINE_MARKER, "")), (name, data)],
            None => vec![("body.xhtml", body(BASELINE_MARKER, ""))],
        };
        let epub = zip_epub(&package, &files, None);
        let result = convert_result(&epub);
        print_result("G2-12", fixture, &result);
        let azw3 = result.unwrap_or_else(|error| panic!("{fixture} unexpectedly failed: {error}"));
        assert_baseline(&azw3, fixture);
        assert!(!sections(&azw3).contains("PACKAGE_LINK_TARGET_MARKER"));
        assert!(!rawml(&azw3).contains("PACKAGE_LINK_TARGET_MARKER"));
        assert!(!serialized_contains(&azw3, "PACKAGE_LINK_TARGET_MARKER"));
        println!(
            "ROUND7A_SAFE_OMISSION|id=G2-12|fixture={fixture}|evidence=package-link-not-added-to-reading-or-resource-output"
        );
    }
}

// E2E-ID: E2E-OCF-06
// Audit: G2-17
#[test]
fn package_and_css_data_url_boundaries_are_distinct() {
    // Audit coverage: G2-17.
    let package_data_url = package_with_metadata_children(
        r#"<link rel="record" href="data:application/json;base64,ROUND7_METADATA_DATA" media-type="application/json"/>"#,
        r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/><item id="package-data" href="data:application/octet-stream;base64,ROUND7_PACKAGE_DATA" media-type="application/octet-stream"/>"#,
        r#"<itemref idref="body"/>"#,
        "",
    );
    let package_data_epub = zip_epub(
        &package_data_url,
        &[("body.xhtml", body(BASELINE_MARKER, ""))],
        None,
    );
    let package_result = convert_result(&package_data_epub);
    print_result("G2-17", "package-level-data-href", &package_result);
    let package_azw3 = package_result
        .unwrap_or_else(|error| panic!("package-level data URL unexpectedly failed: {error}"));
    assert_baseline(&package_azw3, "package-level-data-href");
    assert!(!rawml(&package_azw3).contains("ROUND7_PACKAGE_DATA"));
    assert!(!rawml(&package_azw3).contains("ROUND7_METADATA_DATA"));
    assert!(!serialized_contains(&package_azw3, "ROUND7_PACKAGE_DATA"));
    assert!(!serialized_contains(&package_azw3, "ROUND7_METADATA_DATA"));
    println!(
        "ROUND7A_SAFE_OMISSION|id=G2-17|fixture=package-level-data-href|evidence=spec-prohibited-package-data-urls-are-not-treated-as-ZIP-resources-and-publication-output-is-unchanged"
    );

    let css_package = package(
        r#"<item id="style" href="style.css" media-type="text/css"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#,
        r#"<itemref idref="body"/>"#,
        "",
    );
    let css_data_url = "data:image/png;base64,ROUND7_CSS_DATA";
    let css_epub = zip_epub(
        &css_package,
        &[
            (
                "style.css",
                format!(".asset {{ background-image: url({css_data_url}); }}").into_bytes(),
            ),
            (
                "body.xhtml",
                body(
                    BASELINE_MARKER,
                    r#"<link rel="stylesheet" href="style.css"/><p class="asset">CSS_DATA_URL_MARKER</p>"#,
                ),
            ),
        ],
        None,
    );
    let css_result = convert_result(&css_epub);
    print_result("G2-17", "css-data-url", &css_result);
    let css_azw3 =
        css_result.unwrap_or_else(|error| panic!("CSS data URL unexpectedly failed: {error}"));
    assert_baseline(&css_azw3, "css-data-url");
    let css_output = rawml(&css_azw3);
    assert!(css_output.contains("CSS_DATA_URL_MARKER"));
    assert!(css_output.contains(css_data_url));
    assert!(!css_output.contains("kindle:embed:"));
    println!(
        "ROUND7A_SAFE_TRANSPORT|id=G2-17|fixture=css-data-url|evidence=CSS-data-url-preserved-without-package-resource-embedding"
    );
}

// E2E-ID: E2E-OCF-07
// Audit: G2-18
#[test]
fn package_collection_shapes_record_the_omission_boundary() {
    // Audit coverage: G2-18.
    let cases = [
        ("empty", r#"<collection role="series"/>"#),
        (
            "link",
            r#"<collection role="series"><link href="series.json"/></collection>"#,
        ),
        (
            "metadata",
            r#"<collection role="series"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>COLLECTION_TITLE_MARKER</dc:title></metadata></collection>"#,
        ),
    ];
    for (fixture, collection) in cases {
        let package = package(
            r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#,
            r#"<itemref idref="body"/>"#,
            collection,
        );
        let epub = zip_epub(
            &package,
            &[
                ("body.xhtml", body(BASELINE_MARKER, "")),
                ("series.json", b"SERIES_LINK_MARKER".to_vec()),
            ],
            None,
        );
        let result = convert_result(&epub);
        print_result("G2-18", fixture, &result);
        let azw3 = result.unwrap_or_else(|error| panic!("{fixture} unexpectedly failed: {error}"));
        assert_baseline(&azw3, fixture);
        let title = azw3.exth().text(503);
        println!(
            "ROUND7A_COLLECTION_OBSERVATION|fixture={fixture}|exth-title={title:?}|collection-marker-in-sections={}|series-marker-in-output={}",
            sections(&azw3).contains("COLLECTION_TITLE_MARKER"),
            rawml(&azw3).contains("SERIES_LINK_MARKER"),
        );
        assert_eq!(title.as_deref(), Some("Main Title"));
        assert!(!sections(&azw3).contains("COLLECTION_TITLE_MARKER"));
        assert!(!rawml(&azw3).contains("SERIES_LINK_MARKER"));
        assert!(!serialized_contains(&azw3, "SERIES_LINK_MARKER"));
        println!(
            "ROUND7A_SAFE_OMISSION|id=G2-18|fixture={fixture}|evidence=collection-content-does-not-displace-package-title-or-reading-output"
        );
    }
}

// E2E-ID: E2E-OCF-09
// Audit: G2-19
#[test]
fn legacy_guide_does_not_override_epub3_navigation() {
    // Audit coverage: G2-19.
    let guide_package = package(
        r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>"#,
        r#"<itemref idref="body"/>"#,
        r#"<guide><reference type="text" title="LEGACY_GUIDE_TITLE_MARKER" href="body.xhtml#guide-target"/></guide>"#,
    );
    let epub = zip_epub(
        &guide_package,
        &[
            (
                "body.xhtml",
                body(
                    BASELINE_MARKER,
                    r#"<p id="guide-target">GUIDE_TARGET_MARKER</p>"#,
                ),
            ),
            (
                "nav.xhtml",
                br#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="body.xhtml">NAV_TOC_MARKER</a></li></ol></nav></body></html>"#.to_vec(),
            ),
        ],
        None,
    );
    let result = convert_result(&epub);
    print_result("G2-19", "legacy-guide", &result);
    let azw3 = result.unwrap_or_else(|error| panic!("legacy Guide unexpectedly failed: {error}"));
    let baseline_epub = zip_epub(
        &package(
            r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>"#,
            r#"<itemref idref="body"/>"#,
            "",
        ),
        &[
            (
                "body.xhtml",
                body(
                    BASELINE_MARKER,
                    r#"<p id="guide-target">GUIDE_TARGET_MARKER</p>"#,
                ),
            ),
            (
                "nav.xhtml",
                br#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="body.xhtml">NAV_TOC_MARKER</a></li></ol></nav></body></html>"#.to_vec(),
            ),
        ],
        None,
    );
    let baseline = convert_result(&baseline_epub)
        .unwrap_or_else(|error| panic!("guide-free EPUB unexpectedly failed: {error}"));
    assert_baseline(&azw3, "legacy-guide");
    let guide_targets = azw3.guide_targets();
    assert!(serialized_contains(&azw3, NAV_TOC_MARKER));
    assert!(serialized_contains(&baseline, NAV_TOC_MARKER));
    println!(
        "ROUND7A_GUIDE_OBSERVATION|id=G2-19|legacy-title-present-in-output={}|guide-targets={guide_targets:?}",
        rawml(&azw3).contains("LEGACY_GUIDE_TITLE_MARKER"),
    );
    assert!(sections(&azw3).contains("GUIDE_TARGET_MARKER"));
    assert!(!rawml(&azw3).contains("LEGACY_GUIDE_TITLE_MARKER"));
    assert_eq!(rawml(&azw3), rawml(&baseline));
    assert_eq!(guide_targets, baseline.guide_targets());
    println!(
        "ROUND7A_SAFE_OMISSION|id=G2-19|evidence=EPUB3-nav-output-is-identical-with-or-without-legacy-guide"
    );
}

// E2E-ID: E2E-OCF-05
// Audit: G2-20
#[test]
fn generic_manifest_properties_preserve_feature_channels() {
    // Audit coverage: G2-20.
    let cases = [
        (
            "xhtml-unknown-properties",
            r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml" properties="example:unknown custom-token"/>"#,
            r#"<itemref idref="body"/>"#,
            vec![("body.xhtml", body(BASELINE_MARKER, ""))],
            None,
        ),
        (
            "css-unknown-property",
            r#"<item id="style" href="style.css" media-type="text/css" properties="example:unknown"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#,
            r#"<itemref idref="body"/>"#,
            vec![
                (
                    "style.css",
                    b"body { color: red; } /* GENERIC_PROPERTY_CSS_MARKER */".to_vec(),
                ),
                (
                    "body.xhtml",
                    body(
                        BASELINE_MARKER,
                        r#"<link rel="stylesheet" href="style.css"/>"#,
                    ),
                ),
            ],
            Some("GENERIC_PROPERTY_CSS_MARKER"),
        ),
    ];
    for (fixture, manifest, spine, files, css_marker) in cases {
        let package = package(manifest, spine, "");
        let epub = zip_epub(&package, &files, None);
        let result = convert_result(&epub);
        print_result("G2-20", fixture, &result);
        let azw3 = result.unwrap_or_else(|error| panic!("{fixture} unexpectedly failed: {error}"));
        assert_baseline(&azw3, fixture);
        if let Some(css_marker) = css_marker {
            assert!(rawml(&azw3).contains(css_marker));
        }
        println!(
            "ROUND7A_SAFE_OMISSION|id=G2-20|fixture={fixture}|evidence=unknown-manifest-properties-do-not-displace-body-or-CSS-transport"
        );
    }

    let known_plus_unknown_package = package(
        r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav unknown-token"/>"#,
        r#"<itemref idref="body"/>"#,
        "",
    );
    let known_plus_unknown = zip_epub(
        &known_plus_unknown_package,
        &[
            ("body.xhtml", body(BASELINE_MARKER, "")),
            (
                "nav.xhtml",
                br#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="body.xhtml">KNOWN_NAV_PROPERTY_MARKER</a></li></ol></nav></body></html>"#.to_vec(),
            ),
        ],
        None,
    );
    let azw3 = convert_result(&known_plus_unknown)
        .unwrap_or_else(|error| panic!("known property with unknown token failed: {error}"));
    let known_nav_baseline = package(
        r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/><item id="nav" href="nav.xhtml" media-type="application/xhtml+xml" properties="nav"/>"#,
        r#"<itemref idref="body"/>"#,
        "",
    );
    let known_nav_baseline = convert_result(&zip_epub(
        &known_nav_baseline,
        &[
            ("body.xhtml", body(BASELINE_MARKER, "")),
            (
                "nav.xhtml",
                br#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body><nav epub:type="toc"><ol><li><a href="body.xhtml">KNOWN_NAV_PROPERTY_MARKER</a></li></ol></nav></body></html>"#.to_vec(),
            ),
        ],
        None,
    ))
    .unwrap_or_else(|error| panic!("known nav baseline failed: {error}"));
    assert_baseline(&azw3, "known-plus-unknown-nav");
    assert!(serialized_contains(&azw3, KNOWN_NAV_PROPERTY_MARKER));
    assert!(serialized_contains(
        &known_nav_baseline,
        KNOWN_NAV_PROPERTY_MARKER
    ));
    assert_eq!(rawml(&azw3), rawml(&known_nav_baseline));
    assert_eq!(azw3.guide_targets(), known_nav_baseline.guide_targets());
    println!(
        "ROUND7A_SAFE_OMISSION|id=G2-20|fixture=known-plus-unknown-nav|evidence=known-nav-property-remains-effective-alongside-unknown-token"
    );
}
