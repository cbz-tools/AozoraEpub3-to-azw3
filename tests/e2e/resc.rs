use std::io::Cursor;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};

use super::support::{Azw3, convert_epub, zip_epub};

#[derive(Debug, PartialEq, Eq)]
struct RescItemRef {
    idref: String,
    properties: String,
    skelid: usize,
}

struct ParsedResc {
    record_index: usize,
    record: Vec<u8>,
    xml: Vec<u8>,
    metadata: Vec<(String, String)>,
    itemrefs: Vec<RescItemRef>,
}

// E2E-ID: E2E-RESC-03
// Audit: G6-08, G6-09, G6-26, D-17
#[test]
fn resc_projects_spine_semantics_and_skel_topology() {
    // Audit coverage: D-17..D-22. RESC binary shape is checked against the
    // KINDLEGEN-FINAL/REFERENCE evidence boundary; XML source identity and
    // SKEL linkage are checked against this synthetic EPUB end to end.
    let epub = page_spread_recipe();
    let azw3 = convert_epub(epub);
    let resc = parse_resc(&azw3);
    let mobi = azw3.mobi();

    assert_eq!(resc.record.len() % 4096, 16, "D-17 RESC payload geometry");
    assert!(
        resc.record.len() >= 16 + 4096,
        "D-17 RESC has a padded payload"
    );
    assert!(
        resc.record_index < azw3.record_count(),
        "D-17 RESC is in range"
    );
    assert!(
        resc.record_index > mobi.last_image,
        "D-17 RESC follows images"
    );
    assert!(resc.record_index < mobi.flis, "D-17 RESC precedes FLIS");
    assert_ne!(
        resc.record_index, mobi.first_image,
        "D-17 RESC is not first image"
    );
    assert!(resc.xml.len() < 4096, "D-17 RESC XML fits its payload");

    let expected_ids = ["left", "right", "center"]
        .into_iter()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    assert_eq!(
        resc.itemrefs
            .iter()
            .map(|item| item.idref.clone())
            .collect::<Vec<_>>(),
        expected_ids,
        "D-18/D-19 RESC itemrefs preserve surviving source spine order and identity"
    );
    assert_eq!(
        resc.itemrefs
            .iter()
            .map(|item| item.properties.as_str())
            .collect::<Vec<_>>(),
        [
            "rendition:page-spread-left",
            "page-spread-right",
            "rendition:page-spread-center",
        ],
        "D-21 page-spread properties survive in their source spelling"
    );
    assert_eq!(
        resc.itemrefs
            .iter()
            .map(|item| item.skelid)
            .collect::<Vec<_>>(),
        [1, 2, 3],
        "D-20 RESC skelids use emitted SKEL indices after synthetic TOC insertion"
    );

    let sections = azw3.reconstructed_xhtml_sections();
    assert_eq!(
        sections.len(),
        4,
        "synthetic TOC occupies the first SKEL entry"
    );
    assert!(
        String::from_utf8_lossy(&sections[0]).contains("Left"),
        "synthetic TOC is emitted as the first SKEL section"
    );
    for item in &resc.itemrefs {
        let section = sections
            .get(item.skelid)
            .unwrap_or_else(|| panic!("D-20 skelid {} is out of range", item.skelid));
        let marker = format!("{}_SENTINEL", item.idref.to_ascii_uppercase());
        assert!(
            String::from_utf8_lossy(section).contains(&marker),
            "D-20 skelid {} does not resolve to {}",
            item.skelid,
            item.idref
        );
    }

    assert!(
        resc.itemrefs.iter().all(|item| item.idref != "cover"),
        "D-22 suppressed native cover XHTML must not receive a RESC itemref"
    );
    assert!(
        sections
            .iter()
            .all(|section| !String::from_utf8_lossy(section).contains("COVER_SENTINEL")),
        "D-22 suppressed native cover XHTML must not receive a SKEL entry"
    );
}

// E2E-ID: E2E-RESC-02
// Audit: G6-05, G6-06, G6-07, G6-10, G6-11, G6-27, G6-28, KLAY-04
#[test]
fn resc_projects_publication_rendition_and_itemref_overrides() {
    let azw3 = convert_epub(rendition_recipe("auto", "paginated"));
    let resc = parse_resc(&azw3);

    assert_eq!(
        azw3.exth().text(124).as_deref(),
        Some("portrait"),
        "G6-05 publication orientation remains EXTH 124"
    );
    assert_eq!(
        resc.metadata,
        vec![
            ("rendition:orientation".to_owned(), "portrait".to_owned(),),
            ("rendition:spread".to_owned(), "auto".to_owned()),
            ("rendition:flow".to_owned(), "paginated".to_owned()),
            (
                "rendition:viewport".to_owned(),
                "width=600,height=800".to_owned(),
            ),
        ],
        "G6-06/G6-10/G6-28 publication rendition values are RESC metadata"
    );
    assert_eq!(
        resc.itemrefs
            .iter()
            .map(|item| (item.idref.as_str(), item.properties.as_str(), item.skelid))
            .collect::<Vec<_>>(),
        vec![
            (
                "item1",
                "rendition:orientation-auto rendition:spread-portrait rendition:flow-auto rendition:flow-paginated rendition:flow-scrolled-continuous rendition:flow-scrolled-doc rendition:align-x-center facing-page-left layout-blank",
                0,
            ),
            (
                "item2",
                "rendition:orientation-landscape rendition:spread-landscape facing-page-right",
                1,
            ),
        ],
        "G6-05/G6-07/G6-27/KLAY-04 itemref properties and SKEL mapping"
    );
    assert!(
        resc.itemrefs
            .iter()
            .all(|item| !item.properties.contains("page-spread-center")),
        "G6-11 align-x-center must remain distinct from page-spread-center"
    );

    for spread in ["auto", "both", "landscape", "none", "portrait"] {
        let resc = parse_resc(&convert_epub(rendition_recipe(spread, "paginated")));
        assert_eq!(
            resc.metadata,
            vec![
                ("rendition:orientation".to_owned(), "portrait".to_owned(),),
                ("rendition:spread".to_owned(), spread.to_owned()),
                ("rendition:flow".to_owned(), "paginated".to_owned()),
                (
                    "rendition:viewport".to_owned(),
                    "width=600,height=800".to_owned(),
                ),
            ],
            "G6-06 publication spread {spread}"
        );
    }
    for flow in ["auto", "paginated", "scrolled-continuous", "scrolled-doc"] {
        let resc = parse_resc(&convert_epub(rendition_recipe("none", flow)));
        assert_eq!(
            resc.metadata,
            vec![
                ("rendition:orientation".to_owned(), "portrait".to_owned(),),
                ("rendition:spread".to_owned(), "none".to_owned()),
                ("rendition:flow".to_owned(), flow.to_owned()),
                (
                    "rendition:viewport".to_owned(),
                    "width=600,height=800".to_owned(),
                ),
            ],
            "G6-10 publication flow {flow}"
        );
    }
}

// E2E-ID: E2E-RESC-01
// Audit: G6-04
#[test]
fn publication_orientation_survives_mixed_and_reflowable_layouts() {
    // Audit coverage: G6-04. Publication orientation is independent of
    // publication layout and is retained in both RESC and EXTH 124.
    let (layout, orientation, expected, item_properties) = (
        r#"<meta property="rendition:layout">reflowable</meta>"#,
        "portrait",
        "portrait",
        "",
    );
    let azw3 = convert_epub(orientation_recipe(layout, orientation, item_properties));
    assert_eq!(azw3.exth().text(124).as_deref(), Some(expected));
    let resc = parse_resc(&azw3);
    assert!(
        resc.metadata
            .contains(&("rendition:orientation".to_owned(), expected.to_owned(),)),
        "RESC retains publication orientation for {expected}"
    );

    let mixed = convert_epub(mixed_orientation_recipe());
    assert_eq!(mixed.exth().text(124).as_deref(), Some("landscape"));
    let resc = parse_resc(&mixed);
    assert!(
        resc.metadata
            .contains(&("rendition:orientation".to_owned(), "landscape".to_owned(),)),
        "RESC retains publication orientation for mixed layout"
    );
}

fn parse_resc(azw3: &Azw3) -> ParsedResc {
    let record_index = azw3
        .record_with_magic(b"RESC")
        .expect("D-17 RESC record exists");
    let record = azw3.record(record_index).to_vec();
    assert_eq!(&record[..4], b"RESC");
    assert_eq!(u32_be(&record, 4), 0x10);
    assert_eq!(u32_be(&record, 8), 1);
    let header_length = u32_be(&record, 12) as usize;
    let payload = &record[16..];
    assert!(
        header_length <= payload.len(),
        "RESC header exceeds payload"
    );
    let header = std::str::from_utf8(&payload[..header_length]).expect("RESC header is ASCII");
    assert!(header.starts_with("size=") && header.contains("&version=1&type=1"));
    assert_eq!(header_length, header.len(), "RESC header length prefix");
    let encoded_size = header
        .strip_prefix("size=")
        .and_then(|value| value.split_once('&'))
        .map(|(value, _)| value)
        .expect("RESC header has a size field");
    let xml_length = decode_resc_base32(encoded_size) as usize;
    let xml_offset = header_length;
    let xml_end = xml_offset + xml_length;
    assert!(
        xml_end <= payload.len(),
        "RESC XML length is within payload"
    );
    let payload_length = xml_offset + xml_length;
    let payload_blocks = payload_length.div_ceil(4096);
    assert_eq!(
        record.len(),
        16 + payload_blocks * 4096,
        "RESC payload uses dynamic 4KB-block capacity"
    );
    assert!(payload[xml_end..].iter().all(|byte| *byte == 0));
    let xml = payload[xml_offset..xml_end].to_vec();

    let mut reader = Reader::from_reader(Cursor::new(xml.as_slice()));
    let mut buffer = Vec::new();
    let mut saw_package = false;
    let mut saw_spine = false;
    let mut itemrefs = Vec::new();
    loop {
        match reader
            .read_event_into(&mut buffer)
            .expect("RESC XML parses")
        {
            Event::Start(event) | Event::Empty(event) => match local_name(event.name().as_ref()) {
                "package" => saw_package = true,
                "spine" => saw_spine = true,
                "itemref" => itemrefs.push(parse_itemref(&event)),
                _ => {}
            },
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    assert!(
        saw_package && saw_spine,
        "RESC has package/spine XML structure"
    );
    let metadata = parse_metadata(&xml);
    ParsedResc {
        record_index,
        record,
        xml,
        metadata,
        itemrefs,
    }
}

fn parse_metadata(xml: &[u8]) -> Vec<(String, String)> {
    let mut reader = Reader::from_reader(Cursor::new(xml));
    let mut buffer = Vec::new();
    let mut current = None;
    let mut metadata = Vec::new();
    loop {
        match reader
            .read_event_into(&mut buffer)
            .expect("RESC metadata parses")
        {
            Event::Start(event) if local_name(event.name().as_ref()) == "meta" => {
                let property = event
                    .attributes()
                    .map(|attribute| attribute.expect("RESC metadata attribute"))
                    .find(|attribute| attribute.key.as_ref() == b"property")
                    .map(|attribute| {
                        attribute
                            .unescape_value()
                            .expect("RESC metadata property")
                            .into_owned()
                    })
                    .expect("RESC metadata property attribute");
                current = Some((property, String::new()));
            }
            Event::Text(event) => {
                if let Some((_, value)) = current.as_mut() {
                    value.push_str(&event.unescape().expect("RESC metadata text"));
                }
            }
            Event::End(event) if local_name(event.name().as_ref()) == "meta" => {
                metadata.push(current.take().expect("RESC metadata start"));
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    metadata
}

fn parse_itemref(event: &BytesStart<'_>) -> RescItemRef {
    let mut idref = None;
    let mut properties = String::new();
    let mut skelid = None;
    for attribute in event
        .attributes()
        .map(|attribute| attribute.expect("RESC attribute"))
    {
        let value = attribute
            .unescape_value()
            .expect("RESC attribute value")
            .into_owned();
        match attribute.key.as_ref() {
            b"idref" => idref = Some(value),
            b"properties" => properties = value,
            b"skelid" => skelid = Some(value.parse().expect("numeric RESC skelid")),
            _ => {}
        }
    }
    RescItemRef {
        idref: idref.expect("RESC itemref idref"),
        properties,
        skelid: skelid.expect("RESC itemref skelid"),
    }
}

fn local_name(name: &[u8]) -> &str {
    std::str::from_utf8(name)
        .expect("RESC element name is UTF-8")
        .rsplit(':')
        .next()
        .expect("RESC local element name")
}

fn u32_be(bytes: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes(
        bytes[offset..offset + 4]
            .try_into()
            .expect("RESC u32 bounds"),
    )
}

fn decode_resc_base32(value: &str) -> u32 {
    value.bytes().fold(0u32, |total, byte| {
        let digit = match byte {
            b'0'..=b'9' => byte - b'0',
            b'A'..=b'V' => byte - b'A' + 10,
            _ => panic!("invalid RESC base32 digit"),
        };
        total * 32 + u32::from(digit)
    })
}

fn page_spread_recipe() -> Vec<u8> {
    let package = r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/ops" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>RESC fixture</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language><meta name="cover" content="cover-image"/></metadata><manifest><item id="cover-image" href="images/cover.png" media-type="image/png" properties="cover-image"/><item id="ncx" href="toc.ncx" media-type="application/x-dtbncx+xml"/><item id="cover" href="cover.xhtml" media-type="application/xhtml+xml"/><item id="left" href="left.xhtml" media-type="application/xhtml+xml"/><item id="right" href="right.xhtml" media-type="application/xhtml+xml"/><item id="center" href="center.xhtml" media-type="application/xhtml+xml"/></manifest><spine toc="ncx"><itemref idref="cover" linear="no" properties="rendition:page-spread-left"/><itemref idref="left" properties="rendition:page-spread-left"/><itemref idref="right" properties="page-spread-right"/><itemref idref="center" properties="rendition:page-spread-center"/></spine></package>"#;
    let cover = br#"<html xmlns="http://www.w3.org/1999/xhtml" xmlns:epub="http://www.idpf.org/2007/ops"><body epub:type="cover"><p>COVER_SENTINEL</p></body></html>"#;
    let left =
        br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>LEFT_SENTINEL</p></body></html>"#;
    let right =
        br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>RIGHT_SENTINEL</p></body></html>"#;
    let center =
        br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>CENTER_SENTINEL</p></body></html>"#;
    zip_epub(
        package,
        &[
            ("cover.xhtml", cover.to_vec()),
            (
                "toc.ncx",
                br#"<?xml version="1.0"?><ncx xmlns="http://www.daisy.org/z3986/2005/ncx/" version="2005-1"><navMap><navPoint id="left" playOrder="1"><navLabel><text>Left</text></navLabel><content src="left.xhtml"/></navPoint><navPoint id="right" playOrder="2"><navLabel><text>Right</text></navLabel><content src="right.xhtml"/></navPoint><navPoint id="center" playOrder="3"><navLabel><text>Center</text></navLabel><content src="center.xhtml"/></navPoint></navMap></ncx>"#.to_vec(),
            ),
            ("left.xhtml", left.to_vec()),
            ("right.xhtml", right.to_vec()),
            ("center.xhtml", center.to_vec()),
            ("images/cover.png", tiny_png()),
        ],
        None,
    )
}

fn rendition_recipe(spread: &str, flow: &str) -> Vec<u8> {
    let package = format!(
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/ops" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Rendition fixture</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language><meta property="rendition:layout">pre-paginated</meta><meta property="rendition:orientation">portrait</meta><meta property="rendition:spread">{spread}</meta><meta property="rendition:flow">{flow}</meta><meta property="rendition:viewport">width=600,height=800</meta></metadata><manifest><item id="item1" href="item1.xhtml" media-type="application/xhtml+xml"/><item id="item2" href="item2.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="item1" properties="rendition:orientation-auto rendition:spread-portrait rendition:flow-auto rendition:flow-paginated rendition:flow-scrolled-continuous rendition:flow-scrolled-doc rendition:align-x-center facing-page-left layout-blank"/><itemref idref="item2" properties="rendition:orientation-landscape rendition:spread-landscape facing-page-right"/></spine></package>"#
    );
    let item1 = br#"<html xmlns="http://www.w3.org/1999/xhtml"><head><meta name="viewport" content="width=600,height=800"/></head><body><p>ITEM1_SENTINEL</p><svg xmlns="http://www.w3.org/2000/svg"><text>ITEM1_PAGE</text></svg></body></html>"#;
    let item2 = br#"<html xmlns="http://www.w3.org/1999/xhtml"><head><meta name="viewport" content="width=600,height=800"/></head><body><p>ITEM2_SENTINEL</p><svg xmlns="http://www.w3.org/2000/svg"><text>ITEM2_PAGE</text></svg></body></html>"#;
    zip_epub(
        &package,
        &[
            ("item1.xhtml", item1.to_vec()),
            ("item2.xhtml", item2.to_vec()),
        ],
        None,
    )
}

fn orientation_recipe(layout: &str, orientation: &str, item_properties: &str) -> Vec<u8> {
    let package = format!(
        r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/ops" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Orientation fixture</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language>{layout}<meta property="rendition:orientation">{orientation}</meta></metadata><manifest><item id="item1" href="item1.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="item1" properties="{item_properties}"/></spine></package>"#
    );
    let item = br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>ORIENTATION_SENTINEL</p><svg xmlns="http://www.w3.org/2000/svg"><text>ORIENTATION_PAGE</text></svg></body></html>"#;
    zip_epub(&package, &[("item1.xhtml", item.to_vec())], None)
}

fn mixed_orientation_recipe() -> Vec<u8> {
    let package = r#"<?xml version="1.0"?><package xmlns="http://www.idpf.org/2007/ops" version="3.0"><metadata xmlns:dc="http://purl.org/dc/elements/1.1/"><dc:title>Mixed Orientation fixture</dc:title><dc:creator>Fixture Author</dc:creator><dc:language>en</dc:language><meta property="rendition:orientation">landscape</meta></metadata><manifest><item id="fixed" href="fixed.xhtml" media-type="application/xhtml+xml"/><item id="reflow" href="reflow.xhtml" media-type="application/xhtml+xml"/></manifest><spine><itemref idref="fixed" properties="rendition:layout-pre-paginated"/><itemref idref="reflow"/></spine></package>"#;
    let fixed = br#"<html xmlns="http://www.w3.org/1999/xhtml"><head><meta name="viewport" content="width=1200,height=800"/></head><body><svg xmlns="http://www.w3.org/2000/svg" width="1200" height="800"><text>MIXED_FIXED</text></svg></body></html>"#;
    let reflow =
        br#"<html xmlns="http://www.w3.org/1999/xhtml"><body><p>MIXED_REFLOW</p></body></html>"#;
    zip_epub(
        package,
        &[
            ("fixed.xhtml", fixed.to_vec()),
            ("reflow.xhtml", reflow.to_vec()),
        ],
        None,
    )
}

fn tiny_png() -> Vec<u8> {
    // A minimal valid PNG is supplied by the shared fixture helper through a
    // one-pixel encoded image, keeping this test independent of commercial data.
    use image::{DynamicImage, ImageFormat, Rgba, RgbaImage};
    let image = RgbaImage::from_pixel(2, 2, Rgba([40, 80, 140, 255]));
    let mut output = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut output, ImageFormat::Png)
        .expect("encode RESC fixture cover");
    output.into_inner()
}
