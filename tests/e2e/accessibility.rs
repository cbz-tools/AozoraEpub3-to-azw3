//! Primary audit coverage: G6-22..G6-25, G8-06..G8-14.

use crate::support::{Azw3, zip_epub};
use aozoraepub3_to_azw3::{ConvertOptions, convert_bytes};

fn package(manifest: &str, spine: &str) -> String {
    package_with_metadata(manifest, spine, "")
}

fn package_with_metadata(manifest: &str, spine: &str, metadata: &str) -> String {
    format!(
        r#"<package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:schema="http://schema.org/"><dc:title>Round9 Media Safety</dc:title><dc:creator>Round9 Author</dc:creator><dc:language>en</dc:language>{metadata}</metadata><manifest>{manifest}</manifest><spine>{spine}</spine></package>"#
    )
}

fn body(content: &str) -> Vec<u8> {
    format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>ROUND9_BODY_MARKER</p>{content}</body></html>"#
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

// E2E-ID: E2E-ACCESS-02
// Audit: G6-23, G6-24, G6-25, G8-09
#[test]
fn audio_video_playback_is_explicitly_rejected() {
    // Audit coverage: G6-23..G6-25, G8-09.
    for (kind, tag, media_type, feature_id, remote) in [
        ("audio", "audio", "audio/mpeg", "G6-23", false),
        ("audio", "audio", "audio/mpeg", "G6-25", true),
        ("video", "video", "video/mp4", "G6-24", false),
        ("video", "video", "video/mp4", "G6-25", true),
    ] {
        for with_fallback in [false, true] {
            let source = if remote {
                format!(
                    r#"<{tag} src="https://media.example/{kind}.mp4">{}</{tag}>"#,
                    if with_fallback {
                        "<p>ROUND9_MEDIA_FALLBACK</p>"
                    } else {
                        ""
                    }
                )
            } else {
                format!(
                    r#"<{tag} src="{kind}.bin">{}</{tag}>"#,
                    if with_fallback {
                        "<p>ROUND9_MEDIA_FALLBACK</p>"
                    } else {
                        ""
                    }
                )
            };
            let manifest = if remote {
                r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#
                    .to_owned()
            } else {
                format!(
                    r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/><item id="media" href="{kind}.bin" media-type="{media_type}"/>"#
                )
            };
            let mut files = vec![("body.xhtml", body(&source))];
            if !remote {
                files.push(("audio.bin", b"synthetic-audio".to_vec()));
                files.push(("video.bin", b"synthetic-video".to_vec()));
            }
            let epub = zip_epub(
                &package(&manifest, r#"<itemref idref="body"/>"#),
                &files,
                None,
            );
            let error = convert_result(&epub).expect_err("spine media playback must reject");
            assert!(
                error.contains(feature_id),
                "{kind} {feature_id} boundary missing for fallback={with_fallback}, remote={remote}: {error}"
            );
            assert!(
                error.contains("media boundary"),
                "feature-specific media boundary missing: {error}"
            );
            println!(
                "ROUND9_SAFE_BOUNDARY|ids=G6-23/G6-24/G6-25|kind={kind}|fallback={with_fallback}|remote={remote}|result=explicit-error"
            );
        }
    }
}

// E2E-ID: E2E-ACCESS-05
// Audit: G6-23, G6-24, G6-25, G8-07
#[test]
fn unlinked_media_resources_do_not_change_reading_output() {
    // Audit coverage: G6-22..G6-25, G8-07.
    let body = body("<p>ROUND9_UNLINKED_MEDIA_SAFE</p>");
    let baseline_epub = zip_epub(
        &package(
            r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#,
            r#"<itemref idref="body"/>"#,
        ),
        &[("body.xhtml", body.clone())],
        None,
    );
    let baseline = convert_result(&baseline_epub).expect("baseline must convert");

    let media_manifest = r#"
        <item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>
        <item id="audio" href="audio.mp3" media-type="audio/mpeg"/>
        <item id="video" href="video.mp4" media-type="video/mp4"/>
        <item id="smil" href="media.smil" media-type="application/smil+xml"/>
        <item id="remote-audio" href="https://media.example/audio.mp3" media-type="audio/mpeg"/>
        <item id="remote-video" href="//media.example/video.mp4" media-type="video/mp4"/>
    "#;
    let media_epub = zip_epub(
        &package(media_manifest, r#"<itemref idref="body"/>"#),
        &[
            ("body.xhtml", body),
            ("audio.mp3", b"synthetic-audio".to_vec()),
            ("video.mp4", b"synthetic-video".to_vec()),
            (
                "media.smil",
                br#"<smil xmlns="http://www.w3.org/ns/SMIL"><body><par><text src="body.xhtml#n1"/><audio src="audio.mp3"/></par></body></smil>"#.to_vec(),
            ),
        ],
        None,
    );
    let media = convert_result(&media_epub).expect("unlinked manifest media must remain safe");
    assert_eq!(
        sections(&media),
        sections(&baseline),
        "unlinked media changed the emitted reading sections"
    );
    assert!(sections(&media).contains("ROUND9_UNLINKED_MEDIA_SAFE"));
    println!(
        "ROUND9_SAFE_OMISSION|ids=G6-22/G6-23/G6-24/G6-25/G8-07|unlinked-local+remote-media-smil=accepted|reading-sections=unchanged"
    );
}

// E2E-ID: E2E-ACCESS-03
// Audit: G6-22
#[test]
fn generic_resources_are_transportable_without_affecting_reading() {
    // Audit coverage: G6-22. E-13 remains conditional; this test covers only
    // the documented unreferenced/explicit-rejection boundary.
    let manifest = r#"
        <item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>
        <item id="xml" href="data/ancillary.xml" media-type="application/xml"/>
        <item id="json" href="data/ancillary.json" media-type="application/json"/>
        <item id="opaque" href="data/opaque.bin" media-type="application/octet-stream"/>
    "#;
    let epub = zip_epub(
        &package(manifest, r#"<itemref idref="body"/>"#),
        &[
            ("body.xhtml", body("<p>ROUND9_GENERIC_RESOURCE_SAFE</p>")),
            ("data/ancillary.xml", b"ROUND9_GENERIC_XML".to_vec()),
            ("data/ancillary.json", b"ROUND9_GENERIC_JSON".to_vec()),
            ("data/opaque.bin", b"ROUND9_GENERIC_OPAQUE".to_vec()),
        ],
        None,
    );
    let azw3 = convert_result(&epub).expect("unreferenced generic resources must remain safe");
    let output = sections(&azw3);
    assert!(output.contains("ROUND9_GENERIC_RESOURCE_SAFE"));
    assert!(
        azw3.bytes
            .windows(b"ROUND9_GENERIC_XML".len())
            .any(|window| window == b"ROUND9_GENERIC_XML")
    );
    assert!(
        azw3.bytes
            .windows(b"ROUND9_GENERIC_JSON".len())
            .any(|window| window == b"ROUND9_GENERIC_JSON")
    );
    assert!(
        azw3.bytes
            .windows(b"ROUND9_GENERIC_OPAQUE".len())
            .any(|window| window == b"ROUND9_GENERIC_OPAQUE")
    );
    println!(
        "ROUND9_SAFE_TRANSPORT|id=G6-22|generic=xml+json+octet-stream|body=preserved|resource-bytes=serialized"
    );

    let object_epub = zip_epub(
        &package(manifest, r#"<itemref idref="body"/>"#),
        &[
            (
                "body.xhtml",
                body(
                    r#"<object data="data/ancillary.xml" type="application/xml">ROUND9_GENERIC_OBJECT</object>"#,
                ),
            ),
            ("data/ancillary.xml", b"ROUND9_GENERIC_XML".to_vec()),
            ("data/ancillary.json", b"ROUND9_GENERIC_JSON".to_vec()),
            ("data/opaque.bin", b"ROUND9_GENERIC_OPAQUE".to_vec()),
        ],
        None,
    );
    let error = convert_result(&object_epub)
        .expect_err("generic object targets must not become unreachable successful output");
    assert!(
        error.contains("G6-22"),
        "generic object boundary missing: {error}"
    );
    assert!(
        error.contains("generic non-image"),
        "generic object reason missing: {error}"
    );
    println!(
        "ROUND9_SAFE_BOUNDARY|id=G6-22|generic-object=explicit-error|successful-output=not-presented"
    );
}

// E2E-ID: E2E-ACCESS-01
// Audit: G8-10, G8-12, G8-14
#[test]
fn accessibility_metadata_and_aria_do_not_change_reading_semantics() {
    // Audit coverage: G8-10..G8-14 and G8-11 transport.
    let manifest = r#"<item id="one" href="one.xhtml" media-type="application/xhtml+xml"/><item id="two" href="two.xhtml" media-type="application/xhtml+xml"/>"#;
    let metadata = r#"
        <meta property="schema:accessMode">textual</meta>
        <meta property="schema:accessModeSufficient">textual</meta>
        <meta property="schema:accessibilityFeature">alternativeText</meta>
        <meta property="schema:accessibilityHazard">none</meta>
        <meta property="schema:accessibilitySummary">ROUND9_ACCESSIBILITY_SUMMARY</meta>
        <meta property="dcterms:conformsTo">EPUB Accessibility 1.1</meta>
    "#;
    let epub = zip_epub(
        &package_with_metadata(
            manifest,
            r#"<itemref idref="one"/><itemref idref="two"/>"#,
            metadata,
        ),
        &[
            (
                "one.xhtml",
                body(
                    r#"<main role="main" aria-label="ROUND9_MAIN_LABEL"><p aria-hidden="false">ROUND9_READ_ORDER_ONE</p></main>"#,
                ),
            ),
            (
                "two.xhtml",
                body(
                    r#"<aside role="complementary" aria-describedby="one"><p>ROUND9_READ_ORDER_TWO</p></aside>"#,
                ),
            ),
        ],
        None,
    );
    let azw3 =
        convert_result(&epub).expect("accessibility metadata and ARIA transport must succeed");
    let output = sections(&azw3);
    for marker in [
        "ROUND9_READ_ORDER_ONE",
        "ROUND9_READ_ORDER_TWO",
        "role=\"main\"",
        "aria-label=\"ROUND9_MAIN_LABEL\"",
        "aria-hidden=\"false\"",
        "role=\"complementary\"",
        "aria-describedby=\"one\"",
    ] {
        assert!(
            output.contains(marker),
            "accessibility marker missing: {marker}"
        );
    }
    assert!(
        output.find("ROUND9_READ_ORDER_ONE").unwrap()
            < output.find("ROUND9_READ_ORDER_TWO").unwrap(),
        "spine reading order changed"
    );
    assert!(
        !output.contains("ROUND9_ACCESSIBILITY_SUMMARY"),
        "declarative accessibility metadata must not be injected into reading content"
    );
    println!(
        "ROUND9_SAFE_BOUNDARY|ids=G8-10/G8-11/G8-12/G8-14|metadata=ignored|aria=transported|spine-order=preserved"
    );
}

// E2E-ID: E2E-ACCESS-04
// Audit: G8-06, G8-07, G8-08, G8-09
#[test]
fn media_overlay_linkage_and_direct_smil_are_explicitly_rejected() {
    // Audit coverage: G8-06..G8-08.
    let linked_overlay = zip_epub(
        &package(
            r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml" media-overlay="overlay"/><item id="overlay" href="media.smil" media-type="application/smil+xml"/>"#,
            r#"<itemref idref="body"/>"#,
        ),
        &[
            ("body.xhtml", body("<p>ROUND9_OVERLAY_LINKED</p>")),
            ("media.smil", b"synthetic-smil".to_vec()),
        ],
        None,
    );
    let error = convert_result(&linked_overlay).expect_err("media-overlay linkage must reject");
    assert!(
        error.contains("G8-06"),
        "overlay feature tag missing: {error}"
    );
    assert!(
        error.contains("media overlay"),
        "overlay boundary missing: {error}"
    );

    let direct_smil = zip_epub(
        &package(
            r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/><item id="smil" href="media.smil" media-type="application/smil+xml"/>"#,
            r#"<itemref idref="body"/><itemref idref="smil"/>"#,
        ),
        &[
            ("body.xhtml", body("<p>ROUND9_DIRECT_SMIL</p>")),
            ("media.smil", b"synthetic-smil".to_vec()),
        ],
        None,
    );
    let error = convert_result(&direct_smil).expect_err("direct spine SMIL must reject");
    assert!(error.contains("G8-07"), "SMIL feature tag missing: {error}");
    assert!(error.contains("SMIL"), "SMIL boundary missing: {error}");
    println!(
        "ROUND9_SAFE_BOUNDARY|ids=G8-06/G8-07/G8-08/G8-09|manifest-media-overlay+direct-spine-smil=explicit-error"
    );

    let metadata_only = zip_epub(
        &package_with_metadata(
            r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/><item id="smil" href="media.smil" media-type="application/smil+xml"/>"#,
            r#"<itemref idref="body"/>"#,
            r#"<meta property="media:duration">00:00:10</meta><meta property="media:active-class">-epub-media-overlay-active</meta>"#,
        ),
        &[
            ("body.xhtml", body("<p>ROUND9_MEDIA_METADATA_ONLY</p>")),
            ("media.smil", b"synthetic-smil".to_vec()),
        ],
        None,
    );
    let azw3 = convert_result(&metadata_only)
        .expect("unassociated media-overlay metadata and SMIL must be safely ignored");
    assert!(sections(&azw3).contains("ROUND9_MEDIA_METADATA_ONLY"));
    assert!(!sections(&azw3).contains("media:duration"));
    println!(
        "ROUND9_SAFE_OMISSION|ids=G8-08|unassociated-media-overlay-metadata=ignored|body=preserved"
    );
}
