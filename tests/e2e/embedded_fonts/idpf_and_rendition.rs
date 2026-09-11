//! Primary audit coverage: G1-06, G1-07, G6-04..G6-11, G6-14, G6-19.

use crate::support::{Azw3, zip_epub};
use aozoraepub3_to_azw3::{ConvertOptions, convert_bytes};

const FONT_BYTES: &[u8] =
    include_bytes!("../../fixtures/embedded-font/fonts/EBGaramond12-Bold.ttf");
const IDENTIFIER: &str = "urn:uuid:12345678-1234-5678-1234-567812345678";

fn sha1_digest(input: &[u8]) -> [u8; 20] {
    let mut message = input.to_vec();
    let bit_length = (message.len() as u64) * 8;
    message.push(0x80);
    while message.len() % 64 != 56 {
        message.push(0);
    }
    message.extend_from_slice(&bit_length.to_be_bytes());
    let mut state = [
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
            *word = u32::from_be_bytes(chunk[start..start + 4].try_into().unwrap());
        }
        for index in 16..80 {
            words[index] =
                (words[index - 3] ^ words[index - 8] ^ words[index - 14] ^ words[index - 16])
                    .rotate_left(1);
        }
        let (mut a, mut b, mut c, mut d, mut e) =
            (state[0], state[1], state[2], state[3], state[4]);
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
        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
        state[4] = state[4].wrapping_add(e);
    }
    let mut digest = [0u8; 20];
    for (index, word) in state.iter().enumerate() {
        digest[index * 4..index * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
    digest
}

fn idpf_obfuscate(font: &[u8], identifier: &str) -> Vec<u8> {
    let normalized = identifier
        .chars()
        .filter(|character| !matches!(character, ' ' | '\t' | '\r' | '\n'))
        .collect::<String>();
    let key = sha1_digest(normalized.as_bytes());
    let mut output = font.to_vec();
    for (index, byte) in output.iter_mut().take(1040).enumerate() {
        *byte ^= key[index % key.len()];
    }
    output
}

fn package(metadata: &str, manifest: &str, spine: &str) -> String {
    format!(
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/opf" version="3.0" unique-identifier="pub-id"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/">{metadata}</metadata><manifest>{manifest}</manifest><spine>{spine}</spine></package>"#
    )
}

fn metadata(identifier: &str) -> String {
    format!(
        "<dc:title>EPUB 3.3 P0 round two</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language><dc:identifier id=\"pub-id\">{identifier}</dc:identifier>"
    )
}

fn body() -> Vec<u8> {
    br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>ROUND2_BASELINE</p><svg xmlns="http://www.w3.org/2000/svg"><image href="images/diagram.svg"/></svg><object data="images/diagram.svg#fragment" type="image/svg+xml"><p>OBJECT_FALLBACK</p></object><div data="images/diagram.svg">GENERIC_DATA_ATTRIBUTE</div></body></html>"#.to_vec()
}

fn font_body() -> Vec<u8> {
    br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>ROUND2_BASELINE</p></body></html>"#
        .to_vec()
}

fn encryption_xml(algorithm: &str, target: &str) -> Vec<u8> {
    format!(
        r#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:xenc="http://www.w3.org/2001/04/xmlenc#"><xenc:EncryptedData><xenc:EncryptionMethod Algorithm="{algorithm}"/><xenc:CipherData><xenc:CipherReference URI="{target}"/></xenc:CipherData></xenc:EncryptedData></encryption>"#
    )
    .into_bytes()
}

fn convert(epub: &[u8]) -> Result<Azw3, String> {
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

// E2E-ID: E2E-FONT-02
// Audit: G1-07, G6-21
#[test]
fn idpf_font_obfuscation_uses_unique_identifier_and_plain_font_path() {
    // Audit coverage: G1-07, G6-21. Direct source-font byte and generated
    // resource assertions cover the IDPF-obfuscated FONT path.
    let obfuscated = idpf_obfuscate(
        FONT_BYTES,
        "urn:uuid: 12345678-1234-5678-1234-567812345678\n",
    );
    let package = package(
        &metadata("urn:uuid: 12345678-1234-5678-1234-567812345678\n"),
        r#"<item id="style" href="style.css" media-type="text/css"/><item id="font" href="fonts/font.ttf" media-type="font/ttf"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#,
        r#"<itemref idref="body"/>"#,
    );
    let epub = zip_epub(
        &package,
        &[
            (
                "style.css",
                b"@font-face{font-family:R2;src:url('fonts/font.ttf')}".to_vec(),
            ),
            ("fonts/font.ttf", obfuscated),
            ("body.xhtml", font_body()),
            (
                "META-INF/encryption.xml",
                encryption_xml("http://www.idpf.org/2008/embedding", "fonts/font.ttf"),
            ),
        ],
        None,
    );
    let azw3 = convert(&epub).expect("IDPF font obfuscation converts");
    assert!(
        azw3.bytes
            .windows(FONT_BYTES.len())
            .any(|window| window == FONT_BYTES)
    );
    assert!(
        !azw3
            .bytes
            .windows(1040)
            .any(|window| window == &idpf_obfuscate(FONT_BYTES, IDENTIFIER)[..1040])
    );
}

// E2E-ID: E2E-FONT-03
// Audit: G1-06
#[test]
fn encryption_invalid_shapes_and_non_font_targets_are_explicit_errors() {
    // Audit coverage: G1-06. Direct error assertions cover malformed
    // encryption metadata and non-font targets; this is safe-reject evidence.
    let base_manifest = r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/><item id="font" href="fonts/font.ttf" media-type="font/ttf"/><item id="bin" href="data.bin" media-type="application/octet-stream"/>"#;
    let base_files = |encryption: Vec<u8>, target: &str| {
        zip_epub(
            &package(
                &metadata(IDENTIFIER),
                base_manifest,
                r#"<itemref idref="body"/>"#,
            ),
            &[
                ("body.xhtml", body()),
                ("fonts/font.ttf", FONT_BYTES.to_vec()),
                ("data.bin", b"NON_FONT".to_vec()),
                ("META-INF/encryption.xml", encryption),
            ],
            Some(("unused", target.as_bytes().to_vec())),
        )
    };
    let unknown = convert(&base_files(
        encryption_xml("urn:example:real-encryption", "fonts/font.ttf"),
        "x",
    ))
    .expect_err("unknown encryption must reject");
    assert!(unknown.contains("unsupported encryption algorithm"));
    let malformed = convert(&base_files(b"<encryption><EncryptedData>".to_vec(), "x"))
        .expect_err("malformed encryption must reject");
    assert!(
        malformed.contains("unexpected namespace")
            || malformed.contains("root element")
            || malformed.contains("XML error")
            || malformed.contains("unterminated"),
        "{malformed}"
    );
    let wrong_root = convert(&base_files(
        br#"<not-encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:enc="http://www.w3.org/2001/04/xmlenc#"><enc:EncryptedData><enc:EncryptionMethod Algorithm="http://www.idpf.org/2008/embedding"/><enc:CipherData><enc:CipherReference URI="fonts/font.ttf"/></enc:CipherData></enc:EncryptedData></not-encryption>"#.to_vec(),
        "x",
    ))
    .expect_err("wrong encryption.xml root must reject");
    assert!(wrong_root.contains("root element"));
    let wrong_namespace = convert(&base_files(
        br#"<encryption xmlns="urn:example:wrong-container" xmlns:enc="http://www.w3.org/2001/04/xmlenc#"><enc:EncryptedData><enc:EncryptionMethod Algorithm="http://www.idpf.org/2008/embedding"/><enc:CipherData><enc:CipherReference URI="fonts/font.ttf"/></enc:CipherData></enc:EncryptedData></encryption>"#.to_vec(),
        "x",
    ))
    .expect_err("wrong encryption.xml root namespace must reject");
    assert!(wrong_namespace.contains("root element"));
    let wrong_child_namespace = convert(&base_files(
        br#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:enc="urn:example:wrong-encryption"><enc:EncryptedData><enc:EncryptionMethod Algorithm="http://www.idpf.org/2008/embedding"/><enc:CipherData><enc:CipherReference URI="fonts/font.ttf"/></enc:CipherData></enc:EncryptedData></encryption>"#.to_vec(),
        "x",
    ))
    .expect_err("wrong XML Encryption child namespace must reject");
    assert!(wrong_child_namespace.contains("unexpected namespace or name"));
    let malformed_namespace = convert(&base_files(
        br#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:xml="urn:example:invalid-xml-namespace"/>"#.to_vec(),
        "x",
    ))
    .expect_err("malformed namespace declaration must reject");
    assert!(malformed_namespace.contains("namespace"));
    let unknown_child = convert(&base_files(
        br#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:enc="http://www.w3.org/2001/04/xmlenc#"><enc:EncryptedData><enc:Unknown/><enc:EncryptionMethod Algorithm="http://www.idpf.org/2008/embedding"/><enc:CipherData><enc:CipherReference URI="fonts/font.ttf"/></enc:CipherData></enc:EncryptedData></encryption>"#.to_vec(),
        "x",
    ))
    .expect_err("unknown encryption.xml child must reject");
    assert!(unknown_child.contains("unexpected namespace or name"));
    let unknown_nested_child = convert(&base_files(
        br#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:enc="http://www.w3.org/2001/04/xmlenc#"><enc:EncryptedData><enc:EncryptionMethod Algorithm="http://www.idpf.org/2008/embedding"><enc:Unknown/></enc:EncryptionMethod><enc:CipherData><enc:CipherReference URI="fonts/font.ttf"/></enc:CipherData></enc:EncryptedData></encryption>"#.to_vec(),
        "x",
    ))
    .expect_err("nested encryption.xml child must reject");
    assert!(unknown_nested_child.contains("unexpected namespace or name"));
    let empty_target = convert(&base_files(
        br#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:enc="http://www.w3.org/2001/04/xmlenc#"><enc:EncryptedData/></encryption>"#.to_vec(),
        "x",
    ))
    .expect_err("empty EncryptedData target must reject");
    assert!(empty_target.contains("exactly one EncryptionMethod"));
    let attribute_without_method = convert(&base_files(
        br#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:enc="http://www.w3.org/2001/04/xmlenc#"><enc:EncryptedData Algorithm="http://www.idpf.org/2008/embedding"><enc:CipherData><enc:CipherReference URI="fonts/font.ttf"/></enc:CipherData></enc:EncryptedData></encryption>"#.to_vec(),
        "x",
    ))
    .expect_err("EncryptedData Algorithm attribute without EncryptionMethod must reject");
    assert!(attribute_without_method.contains("child EncryptionMethod"));
    let empty_encryption = convert(&base_files(
        br#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container"/>"#.to_vec(),
        "x",
    ))
    .expect_err("empty encryption.xml must reject");
    assert!(empty_encryption.contains("no EncryptedData targets"));
    let missing_algorithm = convert(&base_files(
        br#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:enc="http://www.w3.org/2001/04/xmlenc#"><enc:EncryptedData><enc:EncryptionMethod/><enc:CipherData><enc:CipherReference URI="fonts/font.ttf"/></enc:CipherData></enc:EncryptedData></encryption>"#.to_vec(),
        "x",
    ))
    .expect_err("EncryptionMethod without Algorithm must reject");
    assert!(missing_algorithm.contains("no Algorithm"));
    let missing_uri = convert(&base_files(
        br#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:enc="http://www.w3.org/2001/04/xmlenc#"><enc:EncryptedData><enc:EncryptionMethod Algorithm="http://www.idpf.org/2008/embedding"/><enc:CipherData><enc:CipherReference/></enc:CipherData></enc:EncryptedData></encryption>"#.to_vec(),
        "x",
    ))
    .expect_err("CipherReference without URI must reject");
    assert!(missing_uri.contains("no URI"));
    let namespaced_algorithm = convert(&base_files(
        br#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:enc="http://www.w3.org/2001/04/xmlenc#" xmlns:attr="urn:example:wrong-attribute"><enc:EncryptedData><enc:EncryptionMethod attr:Algorithm="http://www.idpf.org/2008/embedding"/><enc:CipherData><enc:CipherReference URI="fonts/font.ttf"/></enc:CipherData></enc:EncryptedData></encryption>"#.to_vec(),
        "x",
    ))
    .expect_err("namespaced Algorithm must reject");
    assert!(namespaced_algorithm.contains("no Algorithm"));
    let wrong_case_algorithm = convert(&base_files(
        br#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:enc="http://www.w3.org/2001/04/xmlenc#"><enc:EncryptedData><enc:EncryptionMethod algorithm="http://www.idpf.org/2008/embedding"/><enc:CipherData><enc:CipherReference URI="fonts/font.ttf"/></enc:CipherData></enc:EncryptedData></encryption>"#.to_vec(),
        "x",
    ))
    .expect_err("wrong-case Algorithm must reject");
    assert!(wrong_case_algorithm.contains("no Algorithm"));
    let namespaced_uri = convert(&base_files(
        br#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:enc="http://www.w3.org/2001/04/xmlenc#" xmlns:attr="urn:example:wrong-attribute"><enc:EncryptedData><enc:EncryptionMethod Algorithm="http://www.idpf.org/2008/embedding"/><enc:CipherData><enc:CipherReference attr:URI="fonts/font.ttf"/></enc:CipherData></enc:EncryptedData></encryption>"#.to_vec(),
        "x",
    ))
    .expect_err("namespaced URI must reject");
    assert!(namespaced_uri.contains("no URI"));
    let wrong_case_uri = convert(&base_files(
        br#"<encryption xmlns="urn:oasis:names:tc:opendocument:xmlns:container" xmlns:enc="http://www.w3.org/2001/04/xmlenc#"><enc:EncryptedData><enc:EncryptionMethod Algorithm="http://www.idpf.org/2008/embedding"/><enc:CipherData><enc:CipherReference uri="fonts/font.ttf"/></enc:CipherData></enc:EncryptedData></encryption>"#.to_vec(),
        "x",
    ))
    .expect_err("wrong-case URI must reject");
    assert!(wrong_case_uri.contains("no URI"));
    let non_font = convert(&base_files(
        encryption_xml("http://www.idpf.org/2008/embedding", "data.bin"),
        "x",
    ))
    .expect_err("encrypted non-font must reject");
    assert!(non_font.contains("not a font resource"));
    let missing = convert(&base_files(
        encryption_xml("http://www.idpf.org/2008/embedding", "fonts/missing.ttf"),
        "x",
    ))
    .expect_err("missing target must reject");
    assert!(missing.contains("not a manifest resource"));
    let missing_zip_resource_epub = zip_epub(
        &package(
            &metadata(IDENTIFIER),
            base_manifest,
            r#"<itemref idref="body"/>"#,
        ),
        &[
            ("body.xhtml", body()),
            ("data.bin", b"NON_FONT".to_vec()),
            (
                "META-INF/encryption.xml",
                encryption_xml("http://www.idpf.org/2008/embedding", "fonts/font.ttf"),
            ),
        ],
        None,
    );
    let missing_zip_resource = convert(&missing_zip_resource_epub)
        .expect_err("manifest font target missing from ZIP must reject");
    assert!(
        missing_zip_resource.contains("missing encrypted resource fonts/font.ttf"),
        "{missing_zip_resource}"
    );
    let invalid_id = package(
        "<dc:title>Invalid ID</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language>",
        r#"<item id="font" href="fonts/font.ttf" media-type="font/ttf"/><item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#,
        r#"<itemref idref="body"/>"#,
    );
    let invalid_id_epub = zip_epub(
        &invalid_id,
        &[
            ("body.xhtml", body()),
            ("fonts/font.ttf", FONT_BYTES.to_vec()),
            (
                "META-INF/encryption.xml",
                encryption_xml("http://www.idpf.org/2008/embedding", "fonts/font.ttf"),
            ),
        ],
        None,
    );
    let invalid_id_error =
        convert(&invalid_id_epub).expect_err("missing unique identifier must reject");
    assert!(invalid_id_error.contains("unique-identifier"));
}

// E2E-ID: E2E-RESOURCE-01
// Audit: G6-04, G6-19
#[test]
fn rendition_projection_and_object_data_rewrite_are_observable_in_kf8() {
    // Audit coverage: G6-04, G6-19, KAMZ-02. Direct KF8 observables cover
    // validated rendition projection, SVG object-data rewriting, and transport
    // of Region Magnification markup without claiming EXTH 132 generation.
    let package = package(
        &format!(
            "{}<meta property=\"rendition:layout\">pre-paginated</meta><meta property=\"rendition:orientation\">landscape</meta><meta property=\"rendition:flow\">paginated</meta>",
            metadata(IDENTIFIER)
        ),
        r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/><item id="page" href="page.xhtml" media-type="application/xhtml+xml"/><item id="svg" href="images/diagram.svg" media-type="image/svg+xml"/>"#,
        r#"<itemref idref="body"/><itemref idref="page"/>"#,
    );
    let page = br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><svg xmlns="http://www.w3.org/2000/svg"><image href="images/diagram.svg"/></svg><div app-amzn-magnify="source=region1"><a data-app-amzn-magnify="target=region1">MAGNIFY_MARKER</a></div><p>PAGE_TWO</p></body></html>"#;
    let epub = zip_epub(
        &package,
        &[
            ("body.xhtml", body()),
            ("page.xhtml", page.to_vec()),
            (
                "images/diagram.svg",
                br#"<svg xmlns="http://www.w3.org/2000/svg"><text>SVG_ROUND2</text></svg>"#
                    .to_vec(),
            ),
        ],
        None,
    );
    let azw3 = convert(&epub).expect("supported rendition semantics convert");
    let sections = sections(&azw3);
    assert!(!sections.contains("kindle-rendition-"));
    assert!(!sections.contains("kindle-page-spread-"));
    assert!(sections.contains("<object data=\"kindle:embed:"));
    assert!(!sections.contains("<object data=\"images/diagram.svg"));
    let rawml_bytes = azw3.rawml();
    let rawml = String::from_utf8_lossy(&rawml_bytes);
    assert!(rawml.contains("data=\"images/diagram.svg\" aid=\""));
    assert!(rawml.contains("GENERIC_DATA_ATTRIBUTE"));
    assert!(sections.contains("GENERIC_DATA_ATTRIBUTE"));
    assert!(sections.contains("app-amzn-magnify=\"source=region1\""));
    assert!(sections.contains("data-app-amzn-magnify=\"target=region1\""));
    assert!(sections.contains("MAGNIFY_MARKER"));
    assert_eq!(
        azw3.exth().get(132),
        None,
        "KAMZ-02 does not infer EXTH 132"
    );
    assert_eq!(azw3.exth().text(124).as_deref(), Some("landscape"));
}

#[test]
fn scrolling_and_non_equivalent_item_orientation_are_retained() {
    // Audit coverage: G6-05, G6-10. Publication flow is retained in RESC and
    // item orientation remains an itemref property without changing EXTH 124.
    let manifest = r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#;
    let scrolling = package(
        &format!(
            "{}<meta property=\"rendition:flow\">scrolled-doc</meta>",
            metadata(IDENTIFIER)
        ),
        manifest,
        r#"<itemref idref="body"/>"#,
    );
    let scrolling_epub = zip_epub(&scrolling, &[("body.xhtml", font_body())], None);
    let scrolling_azw3 = convert(&scrolling_epub).expect("scrolling flow converts");
    assert!(scrolling_azw3.record_with_magic(b"RESC").is_some());

    let orientation = package(
        &metadata(IDENTIFIER),
        manifest,
        r#"<itemref idref="body" properties="rendition:orientation-landscape"/>"#,
    );
    let orientation_epub = zip_epub(&orientation, &[("body.xhtml", font_body())], None);
    let orientation_azw3 = convert(&orientation_epub).expect("item orientation override converts");
    assert!(orientation_azw3.record_with_magic(b"RESC").is_some());
}

#[test]
fn unsupported_rendition_semantics_reject_before_kf8_output() {
    // Audit coverage: invalid G6 spread/flow values and publication alignment
    // remain explicit rejects; deprecated portrait spread is projected.
    let manifest = r#"<item id="body" href="body.xhtml" media-type="application/xhtml+xml"/>"#;
    let cases = [
        (
            "publication-spread-invalid",
            "<meta property=\"rendition:spread\">invalid</meta>",
            "",
            "rendition:spread",
        ),
        (
            "publication-flow-invalid",
            "<meta property=\"rendition:flow\">invalid</meta>",
            "",
            "rendition:flow",
        ),
        (
            "publication-align-x-center",
            "<meta property=\"rendition:align-x\">center</meta>",
            "",
            "align-x-center",
        ),
    ];
    for (fixture, metadata_suffix, item_properties, expected_error) in cases {
        let opf = package(
            &format!("{}{}", metadata(IDENTIFIER), metadata_suffix),
            manifest,
            &format!(r#"<itemref idref="body" properties="{item_properties}"/>"#),
        );
        let epub = zip_epub(&opf, &[("body.xhtml", body())], None);
        let error = convert(&epub).expect_err("unsupported rendition must reject before output");
        assert!(
            error.contains(expected_error),
            "{fixture} error did not contain {expected_error}: {error}"
        );
    }
}
