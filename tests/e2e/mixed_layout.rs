//! Primary audit coverage: A-15, A-16, C-05..C-12, E-14, F-09.

use crate::support::{convert_epub, mixed_layout_fixed_page_recipe};

// E2E-ID: E2E-FXL-01
// Audit: A-15, A-16, C-05..C-12, E-14, F-09
#[test]
fn item_level_pre_paginated_semantic_lowers_to_a_dedicated_svg_flow() {
    // Audit coverage: A-15, A-16, C-05..C-12, E-14, F-09. This fixture is intentionally
    // independent of the production flow encoder and has no publication-level
    // fixed-layout declaration.
    let azw3 = convert_epub(mixed_layout_fixed_page_recipe());
    let rawml = String::from_utf8(azw3.rawml()).expect("mixed-layout RawML is UTF-8");
    let ranges = azw3.fdst_ranges();

    assert!(ranges.len() >= 3, "main, CSS, and page flows are required");
    assert_eq!(ranges[0].0, 0, "C-10: main flow starts at zero");
    assert_eq!(ranges.last().unwrap().1, rawml.len());
    assert!(
        ranges.windows(2).all(|pair| pair[0].1 == pair[1].0),
        "C-10: FDST ranges must be contiguous"
    );

    let main = &rawml[..ranges[0].1];
    assert!(
        !main.contains("<svg"),
        "C-06: page SVG leaked into main RawML"
    );
    assert!(main.contains("SECTION1_REFLOWABLE"));
    assert!(main.contains("SECTION2_REFLOWABLE"));
    assert!(
        main.contains("kindle:flow:0001?mime=text/css"),
        "C-11: fixture source stylesheet flow is missing"
    );
    assert!(
        main.contains("kindle:flow:0002?mime=text/css"),
        "C-09: generated layout CSS flow is missing"
    );
    // The fixture has one linked stylesheet: main 0000, source CSS 0001,
    // generated layout CSS 0002, and the fixed-page SVG 0003.
    assert!(
        main.contains("kindle:flow:0003?mime=image/svg+xml"),
        "C-08: fixed page flow must use fixture flow 0003"
    );

    assert!(
        ranges.len() > 3,
        "C-10: main, CSS, generated CSS, and page flows are required"
    );
    let page_flow = flow_bytes(&rawml, &ranges, 3);
    assert!(
        page_flow.contains("kindle:flow:0001?mime=text/css"),
        "C-11: page SVG flow must reference the fixture source stylesheet"
    );
    assert!(
        page_flow.contains("<svg"),
        "C-07: SVG secondary flow missing"
    );
    assert!(
        page_flow.contains("kindle:embed:"),
        "C-11: page resource URI missing"
    );
    assert!(
        !page_flow.contains("<html"),
        "page flow is not an XHTML section"
    );
    assert!(
        page_flow.contains("?mime=image/png"),
        "C-11/E-14: image MIME resolution missing"
    );
    assert_eq!(
        azw3.exth().get(122),
        None,
        "A-16/B-08: item override did not promote the publication"
    );
    assert_eq!(
        azw3.exth().get(127),
        None,
        "B-13 must not synthesize zero-gutter"
    );
    assert_eq!(
        azw3.exth().get(128),
        None,
        "B-14 must not synthesize zero-margin"
    );

    let first = main.find("SECTION1_REFLOWABLE").unwrap();
    let fixed = main.find("?mime=image/svg+xml").unwrap();
    let second = main.find("SECTION2_REFLOWABLE").unwrap();
    assert!(
        first < fixed && fixed < second,
        "F-09: source reading order changed"
    );

    let embed = page_flow
        .split("kindle:embed:")
        .nth(1)
        .and_then(|value| value.split('?').next())
        .expect("page flow embed URI");
    let image_offset = kindle_base32_decode(embed) as usize;
    let mobi = azw3.mobi();
    assert!(
        image_offset > 0,
        "E-14: embed URI uses a positive resource index"
    );
    let resolved_record = mobi.first_image + image_offset - 1;
    assert!(
        azw3.record(resolved_record).starts_with(b"\x89PNG"),
        "E-14: page flow resource does not resolve to the source image"
    );
}

fn flow_bytes<'a>(rawml: &'a str, ranges: &[(usize, usize)], index: usize) -> &'a str {
    let (start, end) = ranges[index];
    &rawml[start..end]
}

fn kindle_base32_decode(value: &str) -> u32 {
    value.bytes().fold(0, |decoded, digit| {
        let value = match digit {
            b'0'..=b'9' => u32::from(digit - b'0'),
            b'A'..=b'V' => u32::from(digit - b'A') + 10,
            _ => panic!("invalid Kindle base32 digit {digit:?}"),
        };
        decoded * 32 + value
    })
}
