//! Diagnostic characterization only; release ownership is recorded in the
//! responsibility-based E2E modules where applicable.

use crate::support::{Azw3, zip_epub};
use aozoraepub3_to_azw3::{ConvertOptions, convert_bytes};

const BASELINE_MARKER: &str = "EPUB33_P1_BASELINE_MARKER";
const FONT_UUID: &str = "urn:uuid:12345678-1234-5678-1234-567812345678";
const FONT_BYTES: &[u8] = include_bytes!("../fixtures/embedded-font/fonts/EBGaramond12-Bold.ttf");

fn base_package(manifest: &str, spine: &str, metadata: &str) -> String {
    format!(
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/">{metadata}</metadata><manifest>{manifest}</manifest><spine>{spine}</spine></package>"#
    )
}

fn basic_metadata() -> &'static str {
    "<dc:title>EPUB 3.3 P1 Characterization</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language>"
}

fn convert_result(epub: &[u8]) -> Result<Azw3, String> {
    convert_bytes(epub, &ConvertOptions::default())
        .map(Azw3::parse)
        .map_err(|error| error.to_string())
}

fn rawml(azw3: &Azw3) -> String {
    String::from_utf8_lossy(&azw3.rawml()).into_owned()
}

fn reconstructed_sections(azw3: &Azw3) -> String {
    azw3.reconstructed_xhtml_sections()
        .into_iter()
        .map(|section| String::from_utf8_lossy(&section).into_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

fn encryption_xml(algorithm: &str, target: &str) -> Vec<u8> {
    format!(
        r#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:enc="http://www.w3.org/2001/04/xmlenc#"><enc:EncryptedData><enc:EncryptionMethod Algorithm="{algorithm}"/><enc:CipherData><enc:CipherReference URI="{target}"/></enc:CipherData></enc:EncryptedData></encryption>"#
    )
    .into_bytes()
}

// Minimal SHA-1 implementation used only to construct the IDPF-shaped test
// resource. The production code is intentionally not changed by this test.
fn sha1_digest(input: &[u8]) -> [u8; 20] {
    let mut message = input.to_vec();
    let bit_length = (message.len() as u64) * 8;
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_length.to_be_bytes());

    let mut h = [
        0x67452301u32,
        0xEFCDAB89,
        0x98BADCFE,
        0x10325476,
        0xC3D2E1F0,
    ];
    for chunk in message.chunks_exact(64) {
        let mut words = [0u32; 80];
        for (index, word) in words[..16].iter_mut().enumerate() {
            let start = index * 4;
            *word = u32::from_be_bytes(
                chunk[start..start + 4]
                    .try_into()
                    .expect("SHA-1 word has four bytes"),
            );
        }
        for index in 16..80 {
            words[index] =
                (words[index - 3] ^ words[index - 8] ^ words[index - 14] ^ words[index - 16])
                    .rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) = (h[0], h[1], h[2], h[3], h[4]);
        for (index, word) in words.iter().enumerate() {
            let (f, k) = match index {
                0..=19 => ((b & c) | ((!b) & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                _ => (b ^ c ^ d, 0xCA62C1D6),
            };
            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(*word);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut digest = [0u8; 20];
    for (index, word) in h.iter().enumerate() {
        digest[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    digest
}

fn idpf_obfuscate(font: &[u8], identifier: &str) -> Vec<u8> {
    let key = sha1_digest(identifier.as_bytes());
    let mut obfuscated = font.to_vec();
    for (index, byte) in obfuscated.iter_mut().take(1040).enumerate() {
        *byte ^= key[index % key.len()];
    }
    obfuscated
}

#[test]
fn font_obfuscation_matrix_observes_supported_and_rejected_inputs() {
    let css = b"@font-face { font-family: P1; src: url('fonts/p1.ttf'); }".to_vec();
    let body = format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><link rel="stylesheet" href="style.css"/></head><body><p>{BASELINE_MARKER}</p></body></html>"#
    )
    .into_bytes();
    let manifest = r#"<item id="style" href="style.css" media-type="text/css"/><item id="font" href="fonts/p1.ttf" media-type="font/ttf"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#;
    let spine = r#"<itemref idref="body"/>"#;
    let plain_package = base_package(manifest, spine, basic_metadata());
    let plain_epub = zip_epub(
        &plain_package,
        &[
            ("style.css", css.clone()),
            ("fonts/p1.ttf", FONT_BYTES.to_vec()),
            ("body.xhtml", body.clone()),
        ],
        None,
    );
    let plain = convert_result(&plain_epub).expect("plain font baseline converts");
    assert!(contains_bytes(&plain.bytes, FONT_BYTES));
    assert!(rawml(&plain).contains(BASELINE_MARKER));
    println!(
        "EPUB33_P1|ids=G1-06,G1-07,G6-21|fixture=plain-font|result=success|font-bytes=preserved"
    );

    let obfuscated = idpf_obfuscate(FONT_BYTES, FONT_UUID);
    assert_ne!(&obfuscated[..4], &[0x00, 0x01, 0x00, 0x00]);
    let idpf_package = base_package(
        manifest,
        spine,
        &format!(
            r#"{}<dc:identifier id="book-id">{FONT_UUID}</dc:identifier>"#,
            basic_metadata()
        ),
    )
    .replace(
        "<package xmlns=",
        "<package unique-identifier=\"book-id\" xmlns=",
    );
    let idpf_epub = zip_epub(
        &idpf_package,
        &[
            ("style.css", css.clone()),
            ("fonts/p1.ttf", obfuscated.clone()),
            ("body.xhtml", body.clone()),
            (
                "META-INF/encryption.xml",
                encryption_xml("http://www.idpf.org/2008/embedding", "fonts/p1.ttf"),
            ),
        ],
        None,
    );
    let idpf = convert_result(&idpf_epub).expect("IDPF-obfuscated fixture converts");
    assert!(contains_bytes(&idpf.bytes, FONT_BYTES));
    assert!(!contains_bytes(&idpf.bytes, &obfuscated));
    assert!(rawml(&idpf).contains("kindle:embed:"));
    println!(
        "EPUB33_P1|ids=G1-06,G1-07,G6-21|fixture=idpf-obfuscated-valid-target|result=success|deobfuscated=true|source-bytes-copied=false"
    );

    let unknown_epub = zip_epub(
        &idpf_package,
        &[
            ("style.css", css.clone()),
            ("fonts/p1.ttf", obfuscated.clone()),
            ("body.xhtml", body.clone()),
            (
                "META-INF/encryption.xml",
                encryption_xml("urn:example:unknown-obfuscation", "fonts/p1.ttf"),
            ),
        ],
        None,
    );
    let unknown = convert_result(&unknown_epub).expect_err("unknown algorithm fixture rejects");
    assert!(unknown.contains("unsupported encryption algorithm"));
    println!(
        "EPUB33_P1|ids=G1-06,G1-07,G6-21|fixture=unknown-algorithm|result=error|error=unsupported-algorithm"
    );

    let encrypted_non_font_package = base_package(
        r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/><item id="binary" href="data.bin" media-type="application/octet-stream"/>"#,
        spine,
        basic_metadata(),
    );
    let body_without_stylesheet = format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>{BASELINE_MARKER}</p></body></html>"#
    )
    .into_bytes();
    let encrypted_non_font_epub = zip_epub(
        &encrypted_non_font_package,
        &[
            ("body.xhtml", body_without_stylesheet),
            ("data.bin", b"ENCRYPTED_NON_FONT_MARKER".to_vec()),
            (
                "META-INF/encryption.xml",
                encryption_xml("http://www.idpf.org/2008/embedding", "data.bin"),
            ),
        ],
        None,
    );
    let encrypted_non_font =
        convert_result(&encrypted_non_font_epub).expect_err("encrypted non-font fixture rejects");
    assert!(encrypted_non_font.contains("not a font resource"));
    println!(
        "EPUB33_P1|ids=G1-06,G1-07,G6-21|fixture=encrypted-non-font|result=error|error=non-font-target"
    );

    let missing_epub = zip_epub(
        &idpf_package,
        &[
            ("style.css", css),
            ("body.xhtml", body),
            (
                "META-INF/encryption.xml",
                encryption_xml("http://www.idpf.org/2008/embedding", "fonts/missing.ttf"),
            ),
        ],
        None,
    );
    let missing = convert_result(&missing_epub).expect_err("missing font target must fail");
    assert!(missing.contains("not a manifest resource"));
    println!(
        "EPUB33_P1|ids=G1-06,G1-07,G6-21|fixture=missing-font-entry|result=error|error=missing-manifest-target"
    );
}

#[test]
fn mathml_transport_matrix_observes_rejection_boundaries() {
    // These fixtures exercise the current safe-rejection boundary: a MathML
    // property or namespace must produce an identifiable error rather than
    // silently transporting unsupported semantics or a fallback resource.
    for (property, fixture) in [
        ("", "mathml-property-absent"),
        (" properties=\"mathml\"", "mathml-property-present"),
    ] {
        let package = base_package(
            &format!(
                r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"{property}/><item id="fallback" href="fallback.png" media-type="image/png"/>"#
            ),
            r#"<itemref idref="body"/>"#,
            basic_metadata(),
        );
        let body = format!(
            r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>{BASELINE_MARKER}</p><p>MATHML_BEGIN<math xmlns="http://www.w3.org/1998/Math/MathML" id="formula"><semantics><mfrac><mi>a</mi><mi>b</mi></mfrac><msubsup><mi>x</mi><mn>1</mn><mn>2</mn></msubsup><annotation>a/b</annotation></semantics></math>MATHML_END</p><p><img src="fallback.png" alt="Equation fallback description"/></p><figure><figcaption>FIGCAPTION_MARKER</figcaption></figure></body></html>"#
        );
        let epub = zip_epub(
            &package,
            &[
                ("body.xhtml", body.into_bytes()),
                ("fallback.png", b"MATHML_FALLBACK_IMAGE".to_vec()),
            ],
            None,
        );
        let error = convert_result(&epub).expect_err("MathML fixture must be rejected");
        assert!(error.contains("MathML"), "{fixture}: {error}");
        assert!(
            error.to_ascii_lowercase().contains("unsupported"),
            "{fixture}: {error}"
        );
        println!(
            "EPUB33_P1|ids=G3-07,G2-25|fixture={fixture}|result=error|mathml=unsupported|fallback-resource=not-transported"
        );
    }
}

#[test]
fn language_bidi_and_direction_transport_is_observable() {
    let package = base_package(
        r#"<item id="style" href="style.css" media-type="text/css"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#,
        r#"<itemref idref="body"/>"#,
        basic_metadata(),
    );
    let body = format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml" xml:lang="en" lang="ja" dir="rtl"><head><link rel="stylesheet" href="style.css"/></head><body><p>{BASELINE_MARKER}</p><p lang="ar" dir="rtl">ARABIC_MARKER العربية Latin</p><p xml:lang="he" dir="ltr">HEBREW_MARKER עברית Latin</p><p dir="auto">AUTO_BIDI_MARKER</p></body></html>"#
    );
    let css = b"html { direction: ltr; } body { direction: rtl; unicode-bidi: isolate; } p[dir=auto] { unicode-bidi: plaintext; }".to_vec();
    let epub = zip_epub(
        &package,
        &[("body.xhtml", body.into_bytes()), ("style.css", css)],
        None,
    );
    let azw3 = convert_result(&epub).expect("language/bidi fixture converts");
    let sections = reconstructed_sections(&azw3);
    let rawml = rawml(&azw3);
    for marker in [
        BASELINE_MARKER,
        "xml:lang=\"en\"",
        "lang=\"ja\"",
        "dir=\"rtl\"",
        "lang=\"ar\"",
        "xml:lang=\"he\"",
        "ARABIC_MARKER العربية Latin",
        "HEBREW_MARKER עברית Latin",
        "AUTO_BIDI_MARKER",
    ] {
        assert!(
            sections.contains(marker),
            "language/bidi fixture lost {marker}"
        );
    }
    assert!(rawml.contains("unicode-bidi: isolate"));
    assert!(rawml.contains("unicode-bidi: plaintext"));
    assert!(rawml.contains("direction: rtl"));
    println!(
        "EPUB33_P1|ids=G3-10,G3-11,G3-12,G7-18|fixture=language-bidi-mixed|result=success|attributes=transported|css-direction=transported-and-layout-fallback|unicode-bidi=transported|renderer-semantics=not-observed"
    );
}

#[test]
fn rendition_spread_orientation_and_viewport_are_observable() {
    let cases = [
        (
            "publication-orientation-portrait",
            r#"<meta property="rendition:layout">pre-paginated</meta><meta property="rendition:orientation">portrait</meta>"#,
            "",
            Some("portrait"),
            None,
        ),
        (
            "publication-orientation-landscape",
            r#"<meta property="rendition:layout">pre-paginated</meta><meta property="rendition:orientation">landscape</meta>"#,
            "",
            Some("landscape"),
            None,
        ),
        (
            "publication-orientation-auto",
            r#"<meta property="rendition:layout">pre-paginated</meta><meta property="rendition:orientation">auto</meta>"#,
            "",
            Some("none"),
            None,
        ),
        (
            "publication-spread-none",
            r#"<meta property="rendition:spread">none</meta>"#,
            "",
            None,
            None,
        ),
        (
            "publication-flow-paginated",
            r#"<meta property="rendition:flow">paginated</meta>"#,
            "",
            None,
            None,
        ),
        (
            "item-flow-paginated",
            "",
            "rendition:flow-paginated",
            None,
            None,
        ),
        (
            "item-orientation-equivalent-landscape",
            r#"<meta property="rendition:layout">pre-paginated</meta><meta property="rendition:orientation">landscape</meta>"#,
            "rendition:orientation-landscape",
            Some("landscape"),
            None,
        ),
        (
            "publication-orientation-reflowable",
            r#"<meta property="rendition:orientation">portrait</meta>"#,
            "",
            Some("portrait"),
            None,
        ),
        (
            "publication-spread-auto",
            r#"<meta property="rendition:spread">auto</meta>"#,
            "",
            None,
            None,
        ),
        (
            "publication-spread-landscape",
            r#"<meta property="rendition:spread">landscape</meta>"#,
            "",
            None,
            None,
        ),
        (
            "publication-spread-portrait",
            r#"<meta property="rendition:spread">portrait</meta>"#,
            "",
            None,
            None,
        ),
        (
            "publication-flow-auto",
            r#"<meta property="rendition:flow">auto</meta>"#,
            "",
            None,
            None,
        ),
        (
            "publication-spread-invalid",
            r#"<meta property="rendition:spread">invalid</meta>"#,
            "",
            None,
            Some("unsupported rendition:spread value invalid"),
        ),
        (
            "publication-flow-invalid",
            r#"<meta property="rendition:flow">invalid</meta>"#,
            "",
            None,
            Some("unsupported rendition:flow value invalid"),
        ),
        (
            "publication-spread-both",
            r#"<meta property="rendition:spread">both</meta>"#,
            "",
            None,
            None,
        ),
        (
            "publication-flow-scrolled-continuous",
            r#"<meta property="rendition:flow">scrolled-continuous</meta>"#,
            "",
            None,
            None,
        ),
        (
            "publication-flow-scrolled-doc",
            r#"<meta property="rendition:flow">scrolled-doc</meta>"#,
            "",
            None,
            None,
        ),
        (
            "publication-align-x-center",
            r#"<meta property="rendition:align-x">center</meta>"#,
            "",
            None,
            Some("align-x-center"),
        ),
        (
            "item-orientation-different",
            "",
            "rendition:orientation-landscape",
            None,
            None,
        ),
        ("item-spread-auto", "", "rendition:spread-auto", None, None),
        ("item-spread-both", "", "rendition:spread-both", None, None),
        (
            "item-align-x-center",
            "",
            "rendition:align-x-center",
            None,
            None,
        ),
    ];
    for (fixture, metadata, item_properties, expected_orientation, expected_error) in cases {
        let package = base_package(
            r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/><item id="body2" href="body2.xhtml" media-type="application/xhtml+xml"/>"#,
            &format!(
                r#"<itemref idref="body" properties="{item_properties}"/><itemref idref="body2"/>"#
            ),
            &format!("{}{metadata}", basic_metadata()),
        );
        let body = format!(
            r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><meta name="viewport" content="width=600,height=800"/></head><body><p>{BASELINE_MARKER}</p><p>RENDITION_{fixture}</p><svg xmlns="http://www.w3.org/2000/svg"><text>RENDITION_PAGE_MARKER</text></svg></body></html>"#
        );
        let body2 = r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><meta name="viewport" content="width=600,height=800"/></head><body><p>RENDITION_NEIGHBOR_MARKER</p><svg xmlns="http://www.w3.org/2000/svg"><text>RENDITION_NEIGHBOR_PAGE_MARKER</text></svg></body></html>"#;
        let epub = zip_epub(
            &package,
            &[
                ("body.xhtml", body.into_bytes()),
                ("body2.xhtml", body2.as_bytes().to_vec()),
            ],
            None,
        );
        let result = convert_result(&epub);
        if let Some(expected_error) = expected_error {
            let error = result.expect_err("unsupported rendition fixture must reject");
            assert!(
                error.contains(expected_error),
                "{fixture} error did not contain {expected_error}: {error}"
            );
            continue;
        }
        let azw3 = result.expect("rendition fixture converts");
        let sections = reconstructed_sections(&azw3);
        let rawml = rawml(&azw3);
        assert!(sections.contains(BASELINE_MARKER));
        assert!(sections.contains("RENDITION_NEIGHBOR_MARKER"));
        assert!(sections.contains("width=600,height=800"));
        assert!(!sections.contains("kindle-rendition-"));
        assert!(!sections.contains("kindle-page-spread-"));
        assert!(!rawml.contains("kindle-rendition-"));
        if let Some(expected_orientation) = expected_orientation {
            assert_eq!(azw3.exth().text(124).as_deref(), Some(expected_orientation));
        }
        println!(
            "EPUB33_P1|ids=G6-04..G6-11,G6-14|fixture={fixture}|result=success|orientation-exth124={:?}|paginated=existing-kf8-flow|spread-none=no-op|unsupported-marker-classes=false",
            azw3.exth().text(124)
        );
    }
}

#[test]
fn generic_svg_resource_matrix_is_observable() {
    let package = base_package(
        r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/><item id="svg" href="images/diagram.svg" media-type="image/svg+xml" properties="svg"/><item id="pixel" href="images/pixel.png" media-type="image/png"/>"#,
        r#"<itemref idref="body"/>"#,
        basic_metadata(),
    );
    let svg = br##"<svg xmlns="http://www.w3.org/2000/svg" width="20" height="20"><text id="text-node" x="1" y="10">SVG_RESOURCE_MARKER</text><use href="#text-node">SVG_INTERNAL_FRAGMENT</use><image href="pixel.png"/></svg>"##;
    let body = format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>{BASELINE_MARKER}</p><img src="images/diagram.svg" alt="SVG image description"/><object data="images/diagram.svg" type="image/svg+xml">OBJECT_FALLBACK_MARKER</object><div data="images/diagram.svg">GENERIC_DATA_ATTRIBUTE</div></body></html>"#
    );
    let epub = zip_epub(
        &package,
        &[
            ("body.xhtml", body.into_bytes()),
            ("images/diagram.svg", svg.to_vec()),
            ("images/pixel.png", b"SVG_RASTER_RESOURCE".to_vec()),
        ],
        None,
    );
    let azw3 = convert_result(&epub).expect("generic SVG resource fixture converts");
    let sections = reconstructed_sections(&azw3);
    assert!(contains_bytes(&azw3.bytes, svg));
    assert!(contains_bytes(&azw3.bytes, b"SVG_RASTER_RESOURCE"));
    assert!(contains_bytes(&azw3.bytes, b"SVG_INTERNAL_FRAGMENT"));
    assert!(sections.contains(BASELINE_MARKER));
    assert!(
        sections.contains("kindle:embed:"),
        "img src was not rewritten"
    );
    assert!(sections.contains("SVG image description"));
    assert!(sections.contains("<object data=\"kindle:embed:"));
    assert!(!sections.contains("<object data=\"images/diagram.svg\""));
    assert!(sections.contains("data=\"images/diagram.svg\" aid=\""));
    assert!(sections.contains("GENERIC_DATA_ATTRIBUTE"));
    println!(
        "EPUB33_P1|ids=G6-19,G2-23|fixture=generic-svg-img-object|result=success|resource-record=true|img-src=rewritten|object-data=rewritten|object-subcase=closed"
    );
}

#[test]
fn image_alt_and_figcaption_transport_is_observable() {
    let package = base_package(
        r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/><item id="image" href="image.png" media-type="image/png"/>"#,
        r#"<itemref idref="body"/>"#,
        basic_metadata(),
    );
    let body = format!(
        r#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>{BASELINE_MARKER}</p><img src="image.png" alt="MEANINGFUL_ALT_MARKER"/><img src="image.png" alt=""/><img src="image.png"/><figure><img src="image.png" alt="FIGURE_ALT_MARKER"/><figcaption>FIGCAPTION_MARKER</figcaption></figure></body></html>"#
    );
    let epub = zip_epub(
        &package,
        &[
            ("body.xhtml", body.into_bytes()),
            ("image.png", b"ALT_IMAGE_RESOURCE".to_vec()),
        ],
        None,
    );
    let azw3 = convert_result(&epub).expect("image alt fixture converts");
    let sections = reconstructed_sections(&azw3);
    assert!(sections.contains(BASELINE_MARKER));
    assert!(sections.contains("alt=\"MEANINGFUL_ALT_MARKER\""));
    assert!(sections.contains("alt=\"\""));
    assert!(sections.contains("FIGURE_ALT_MARKER"));
    assert!(sections.contains("FIGCAPTION_MARKER"));
    assert!(contains_bytes(&azw3.bytes, b"ALT_IMAGE_RESOURCE"));
    println!(
        "EPUB33_P1|ids=G8-13|fixture=image-alt-empty-missing-figcaption|result=success|alt-transport=true|empty-vs-missing-preserved=true|figcaption=true"
    );
}
