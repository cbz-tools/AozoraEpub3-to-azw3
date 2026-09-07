mod support;

use image::GenericImageView;
use quick_xml::events::Event;
use quick_xml::name::{Namespace, ResolveResult};
use quick_xml::reader::NsReader;
use std::io::{Cursor, Read};
use support::{FixedLayoutDeclaration, convert_epub, fixed_layout_comic_recipe};
use zip::ZipArchive;

#[test]
fn fixed_layout_page_images_use_secondary_svg_flows() {
    // EXTENDED SCOPE: A-17*, B-08..B-10, B-12, C-05..C-13, E-14. This does
    // not claim KindleGen numeric parity for EXTH 125 or DATP, nor device
    // acceptance or renderer capability.
    let epub = fixed_layout_comic_recipe(FixedLayoutDeclaration::FixedLayoutTrue);
    let mut archive = ZipArchive::new(Cursor::new(&epub)).expect("comic fixture ZIP");
    let mut package = String::new();
    archive
        .by_name("package.opf")
        .expect("comic fixture package")
        .read_to_string(&mut package)
        .expect("comic fixture package is UTF-8");
    let source_spine = "<spine page-progression-direction=\"rtl\"><itemref idref=\"cover\" linear=\"yes\"/><itemref idref=\"page1\" linear=\"yes\"/><itemref idref=\"page2\" linear=\"yes\"/></spine>";
    assert!(package.contains(source_spine));
    let expected_fixed_pages = [
        ("cover", "COVER_FIXED_LAYOUT_SENTINEL", "image1.jpg"),
        ("page1", "FIXED_PAGE1_SENTINEL", "image1.jpg"),
        ("page2", "FIXED_PAGE2_SENTINEL", "image2.jpg"),
    ];
    let expected_source_images = [("image1.jpg", (400, 600)), ("image2.jpg", (320, 480))];
    let expected_fixed_page_count = expected_fixed_pages.len();

    let azw3 = convert_epub(epub);
    let exth = azw3.exth();
    assert_eq!(exth.text(122).as_deref(), Some("true"));
    assert_eq!(exth.text(123).as_deref(), Some("comic"));
    assert_eq!(exth.text(124).as_deref(), Some("none"));
    assert_eq!(exth.text(126).as_deref(), Some("1125x1600"));
    assert_eq!(exth.get(127), None);
    assert_eq!(exth.get(128), None);

    let rawml = String::from_utf8(azw3.rawml()).expect("fixed-layout RawML is UTF-8");
    let ranges = azw3.fdst_ranges();
    let main = &rawml[..ranges[0].1];
    assert!(
        !main.contains("<svg"),
        "C-06: fixed SVG is not inline in main flow"
    );
    assert!(main.contains("FIXED_PAGE1_SENTINEL"));
    assert!(main.contains("FIXED_PAGE2_SENTINEL"));
    assert!(main.contains("COVER_FIXED_LAYOUT_SENTINEL"));
    assert!(main.contains("kindle:flow:0002?mime=image/svg+xml"));
    let page_flow_count = ranges
        .iter()
        .skip(1)
        .filter(|(start, end)| rawml[*start..*end].contains("<svg"))
        .count();
    assert_eq!(
        page_flow_count, expected_fixed_page_count,
        "C-07/C-09: each fixed page has one SVG flow"
    );
    let fixed_flows = ranges
        .iter()
        .skip(1)
        .filter(|(start, end)| rawml[*start..*end].contains("<svg"))
        .copied()
        .collect::<Vec<_>>();
    let cover_flow = &rawml[fixed_flows[0].0..fixed_flows[0].1];
    assert!(cover_flow.contains("kindle:embed:0001"));
    assert_fixed_page_svg_is_well_formed_and_binds_xlink(cover_flow);
    let main_fixed_page_references =
        extract_main_fixed_page_references(main, &expected_fixed_pages);
    assert_eq!(
        main_fixed_page_references.len(),
        expected_fixed_page_count,
        "C-08: every expected fixed page must have exactly one main flow reference"
    );
    let actual_page_order = main_fixed_page_references
        .iter()
        .map(|(page_id, _)| page_id.as_str())
        .collect::<Vec<_>>();
    let expected_page_order = expected_fixed_pages
        .iter()
        .map(|(page_id, _, _)| *page_id)
        .collect::<Vec<_>>();
    assert_eq!(
        actual_page_order, expected_page_order,
        "C-08/F-09: main fixed-page references must follow source spine order"
    );

    let fixed_flow_indices = fixed_flows
        .iter()
        .map(|flow| {
            ranges
                .iter()
                .position(|candidate| candidate == flow)
                .expect("fixed SVG flow belongs to FDST")
        })
        .collect::<Vec<_>>();
    assert_eq!(
        fixed_flow_indices.len(),
        expected_fixed_page_count,
        "C-07/C-09: every expected fixed page has one dedicated SVG flow"
    );
    let expected_main_flow_indices = fixed_flow_indices.clone();
    let actual_main_flow_indices = main_fixed_page_references
        .iter()
        .map(|(_, reference)| flow_reference_fdst_index(reference, &ranges))
        .collect::<Vec<_>>();
    assert_eq!(
        actual_main_flow_indices, expected_main_flow_indices,
        "C-08: every main fixed-page reference must map to its dedicated SVG flow"
    );

    let image_records = azw3.image_records();
    assert!(
        image_records.len() >= expected_source_images.len(),
        "E-14: fixture image resources must remain independently identifiable"
    );
    let mobi = azw3.mobi();
    for (page_index, ((page_id, _), (_, _, expected_image))) in main_fixed_page_references
        .iter()
        .zip(expected_fixed_pages.iter())
        .enumerate()
    {
        let (start, end) = fixed_flows[page_index];
        let embed_reference = fixed_page_embed_reference(&rawml[start..end]);
        let embed_offset = fixed_image_embed_offset(&embed_reference);
        let source_image_index = expected_source_images
            .iter()
            .position(|(image, _)| image == expected_image)
            .expect("expected fixed-page image belongs to fixture manifest")
            + 1;
        assert_eq!(
            embed_offset, source_image_index,
            "E-14: {page_id} SVG flow resolves to the expected fixture image"
        );
        let resolved_record = mobi.first_image + embed_offset - 1;
        assert!(
            azw3.record(resolved_record).starts_with(b"\xff\xd8\xff"),
            "E-14: {page_id} SVG flow resource does not resolve to a JPEG image"
        );
        let decoded_image = image::load_from_memory(azw3.record(resolved_record))
            .expect("E-14: fixed page JPEG resource is decodable");
        assert_eq!(
            decoded_image.dimensions(),
            expected_source_images[source_image_index - 1].1,
            "E-14: {page_id} SVG flow resource has the expected fixture dimensions"
        );
    }
    assert_eq!(ranges.last().unwrap().1, rawml.len());
    assert!(ranges.windows(2).all(|pair| pair[0].1 == pair[1].0));
}

fn extract_main_fixed_page_references(
    main: &str,
    expected_pages: &[(&str, &str, &str)],
) -> Vec<(String, String)> {
    let mut reader = NsReader::from_str(main);
    let mut current_page = None;
    let mut references = Vec::new();

    loop {
        let (_, event) = reader
            .read_resolved_event()
            .expect("main RawML is well-formed XML");
        match event {
            Event::Text(text) => {
                let text = text.unescape().expect("main RawML text is valid XML");
                if let Some((page_id, _, _)) = expected_pages
                    .iter()
                    .find(|(_, sentinel, _)| text.contains(sentinel))
                {
                    assert!(
                        current_page.is_none(),
                        "fixed page {page_id} sentinel appeared before its flow reference"
                    );
                    current_page = Some(*page_id);
                }
            }
            Event::Start(element) | Event::Empty(element) if element.name().as_ref() == b"img" => {
                let Some(reference) = element.attributes().find_map(|attribute| {
                    let attribute = attribute.expect("main RawML image attribute is valid XML");
                    (attribute.key.as_ref() == b"src").then(|| {
                        attribute
                            .unescape_value()
                            .expect("main RawML image reference is valid XML")
                            .into_owned()
                    })
                }) else {
                    continue;
                };
                if reference.starts_with("kindle:flow:")
                    && reference.ends_with("?mime=image/svg+xml")
                {
                    let page_id = current_page
                        .take()
                        .expect("main fixed-page flow reference has no page sentinel");
                    references.push((page_id.to_owned(), reference));
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    assert!(
        current_page.is_none(),
        "fixed page sentinel has no main flow reference"
    );
    references
}

fn flow_reference_fdst_index(reference: &str, ranges: &[(usize, usize)]) -> usize {
    let encoded = reference
        .strip_prefix("kindle:flow:")
        .and_then(|value| value.strip_suffix("?mime=image/svg+xml"))
        .expect("main fixed-page reference has the expected flow URI");
    let index = kindle_base32_decode(encoded) as usize;
    assert!(
        ranges.get(index).is_some(),
        "main fixed-page reference targets an unknown FDST flow"
    );
    index
}

fn fixed_page_embed_reference(flow: &str) -> String {
    let mut reader = NsReader::from_str(flow);
    let mut references = Vec::new();

    loop {
        let (_, event) = reader
            .read_resolved_event()
            .expect("fixed page SVG is well-formed XML");
        match event {
            Event::Start(element) | Event::Empty(element)
                if element.name().as_ref() == b"image" =>
            {
                for attribute in element.attributes() {
                    let attribute = attribute.expect("fixed page SVG attribute is valid XML");
                    if attribute.key.as_ref() == b"xlink:href" {
                        references.push(
                            attribute
                                .unescape_value()
                                .expect("fixed page SVG image reference is valid XML")
                                .into_owned(),
                        );
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    assert_eq!(
        references.len(),
        1,
        "each fixed page SVG has exactly one image resource"
    );
    references.pop().expect("fixed page SVG image reference")
}

fn fixed_image_embed_offset(reference: &str) -> usize {
    let encoded = reference
        .strip_prefix("kindle:embed:")
        .and_then(|value| value.strip_suffix("?mime=image/jpeg"))
        .expect("fixed page image reference has the expected JPEG embed URI");
    let offset = kindle_base32_decode(encoded) as usize;
    assert!(
        offset > 0,
        "fixed page image reference has a positive offset"
    );
    offset
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

fn assert_fixed_page_svg_is_well_formed_and_binds_xlink(flow: &str) {
    let mut reader = NsReader::from_str(flow);
    let mut saw_svg_root = false;
    let mut saw_xlink_declaration = false;
    let mut saw_xlink_href = false;

    loop {
        let (namespace, event) = reader
            .read_resolved_event()
            .expect("generated fixed page SVG is well-formed XML");
        match event {
            Event::Start(element) | Event::Empty(element) => {
                if element.name().as_ref() == b"svg" && !saw_svg_root {
                    saw_svg_root = true;
                    for attribute in element.attributes() {
                        let attribute = attribute.expect("generated SVG attribute is well-formed");
                        if attribute.key.as_ref() == b"xmlns:xlink" {
                            saw_xlink_declaration = attribute
                                .unescape_value()
                                .expect("xlink namespace declaration is valid")
                                == "http://www.w3.org/1999/xlink";
                        }
                    }
                    assert_eq!(
                        namespace,
                        ResolveResult::Bound(Namespace(b"http://www.w3.org/2000/svg"))
                    );
                }

                if element.name().as_ref() == b"image" {
                    for attribute in element.attributes() {
                        let attribute = attribute.expect("generated SVG attribute is well-formed");
                        if attribute.key.as_ref() == b"xlink:href" {
                            let (namespace, local_name) = reader.resolve_attribute(attribute.key);
                            assert_eq!(
                                namespace,
                                ResolveResult::Bound(Namespace(b"http://www.w3.org/1999/xlink"))
                            );
                            assert_eq!(local_name.as_ref(), b"href");
                            saw_xlink_href = true;
                        }
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    assert!(saw_svg_root, "generated fixed page flow has no SVG root");
    assert!(
        saw_xlink_declaration,
        "generated SVG root does not declare the XLink namespace"
    );
    assert!(saw_xlink_href, "generated SVG has no xlink:href attribute");
}
