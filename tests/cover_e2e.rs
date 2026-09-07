mod support;

use aozoraepub3_to_azw3::{ConvertOptions, convert_bytes};
use image::ImageReader;
use std::io::{Cursor, Read};
use support::{
    CoverRecipe, FixedLayoutDeclaration, convert_epub, cover_bodymatter_landmark_recipe,
    cover_recipe, fixed_layout_comic_recipe,
};
use zip::ZipArchive;

#[test]
fn cover_resource_and_library_thumbnail_contract_is_structurally_valid() {
    // Audit coverage: B-15..B-17 and E-04..E-10 (cover resource, EXTH
    // 201/202/129,
    // bounded decodable thumbnail, distinct resources and semantic rather
    // than byte/dimension identity); F-08/F-12 cover/resource preservation.
    for recipe in [
        CoverRecipe {
            cover_page: false,
            cover_navigation: false,
            cover_first: true,
            ordinary_svg: false,
            padding_images: 0,
        },
        CoverRecipe {
            cover_page: false,
            cover_navigation: false,
            cover_first: false,
            ordinary_svg: false,
            padding_images: 0,
        },
        CoverRecipe {
            cover_page: true,
            cover_navigation: true,
            cover_first: false,
            ordinary_svg: false,
            padding_images: 0,
        },
        CoverRecipe {
            cover_page: true,
            cover_navigation: false,
            cover_first: false,
            ordinary_svg: false,
            padding_images: 0,
        },
        CoverRecipe {
            cover_page: false,
            cover_navigation: false,
            cover_first: false,
            ordinary_svg: false,
            padding_images: 16,
        },
    ] {
        let azw3 = convert_epub(cover_recipe(recipe));
        let mobi = azw3.mobi();
        let exth = azw3.exth();
        let images = azw3.image_records();
        let source_image_count = 2 + recipe.padding_images;
        assert_eq!(
            exth.u32(125),
            Some((source_image_count + 1) as u32),
            "source images plus one thumbnail"
        );
        assert_eq!(
            images.len(),
            source_image_count + 1,
            "resource records must include source images and thumbnail"
        );
        assert_eq!(mobi.first_image, images[0].0);
        assert_eq!(mobi.last_image, images[source_image_count].0);
        let expected_cover_offset = if recipe.cover_first {
            0
        } else {
            1 + recipe.padding_images
        };
        assert_eq!(
            exth.u32(201),
            Some(expected_cover_offset as u32),
            "B-15/E-06: EXTH 201 must identify the full cover by first-image-relative offset"
        );
        let thumbnail_offset = exth.u32(202).expect("EXTH 202 thumbnail address");
        assert_eq!(
            thumbnail_offset, source_image_count as u32,
            "B-16/E-07: EXTH 202 must identify the appended thumbnail"
        );
        let embed = exth.text(129).expect("EXTH 129 thumbnail address");
        let expected_embed = format!("kindle:embed:{}", kindle_base32_encode(thumbnail_offset));
        assert_eq!(
            embed, expected_embed,
            "B-17/E-08: EXTH 129 must independently encode EXTH 202"
        );
        assert_eq!(
            kindle_base32_decode(embed.strip_prefix("kindle:embed:").unwrap()),
            thumbnail_offset
        );
        assert_eq!(
            mobi.first_image + thumbnail_offset as usize,
            images[source_image_count].0
        );
        assert!(
            images
                .iter()
                .all(|(_, data)| data.starts_with(&[0xff, 0xd8, 0xff])
                    || data.starts_with(b"\x89PNG")),
            "E-04: all embedded image resources must remain decodable binary images"
        );

        let full_cover_record = mobi.first_image + exth.u32(201).unwrap() as usize;
        let thumbnail_record = mobi.first_image + thumbnail_offset as usize;
        let full_cover = image_dimensions(azw3.record(full_cover_record));
        let thumbnail = image_dimensions(azw3.record(thumbnail_record));
        assert_eq!(
            full_cover,
            (660, 940),
            "E-04: full source cover semantics were changed"
        );
        assert!(
            azw3.record(thumbnail_record)
                .starts_with(&[0xff, 0xd8, 0xff]),
            "E-05: library thumbnail must be a JPEG resource"
        );
        assert!(
            thumbnail.0 <= 330 && thumbnail.1 <= 470 && thumbnail.0 > 0 && thumbnail.1 > 0,
            "E-10: thumbnail dimensions must be positive and bounded"
        );
        assert_eq!(
            thumbnail.0 * full_cover.1,
            thumbnail.1 * full_cover.0,
            "E-10: thumbnail aspect ratio was not preserved"
        );
        assert_ne!(
            full_cover, thumbnail,
            "E-09/E-10: thumbnail must be a distinct bounded representation"
        );
        assert_ne!(
            azw3.record(full_cover_record),
            azw3.record(thumbnail_record),
            "E-09: cover and thumbnail must resolve to distinct resources"
        );
    }

    let alphabetic = convert_epub(cover_recipe(CoverRecipe {
        cover_page: false,
        cover_navigation: false,
        cover_first: false,
        ordinary_svg: false,
        padding_images: 16,
    }));
    let alphabetic_offset = alphabetic.exth().u32(202).expect("alphabetic offset");
    assert!(alphabetic_offset >= 10);
    assert_eq!(alphabetic_offset, 18);
    assert_eq!(alphabetic.exth().u32(201), Some(17));
    assert_eq!(kindle_base32_encode(18), "000I");
    assert_eq!(kindle_base32_decode("000I"), 18);
    assert_eq!(
        alphabetic.exth().text(129).as_deref(),
        Some("kindle:embed:000I"),
        "B-17: alphabetic base32 digits must be emitted for offset 18"
    );

    let ordinary_svg = convert_epub(cover_recipe(CoverRecipe {
        cover_page: false,
        cover_navigation: false,
        cover_first: false,
        ordinary_svg: true,
        padding_images: 0,
    }));
    let ordinary_svg_rawml = String::from_utf8(ordinary_svg.rawml()).unwrap();
    assert!(
        ordinary_svg_rawml.contains("SVG_NON_COVER_SENTINEL"),
        "ordinary SVG child referencing a non-cover image was suppressed"
    );
}

#[test]
fn cover_xhtml_is_suppressed_but_cover_navigation_children_survive() {
    // Cover-page suppression is semantic normalization: it must not delete a
    // child body navigation target and must retain the actual cover resource.
    let with_cover_nav = convert_epub(cover_recipe(CoverRecipe {
        cover_page: true,
        cover_navigation: true,
        cover_first: false,
        ordinary_svg: false,
        padding_images: 0,
    }));
    let with_rawml = String::from_utf8(with_cover_nav.rawml()).unwrap();
    assert!(
        !with_rawml.contains("COVER_PAGE_SENTINEL"),
        "suppressed cover XHTML leaked into RawML"
    );
    assert!(
        !with_rawml.contains("cover.xhtml"),
        "suppressed cover navigation target remained as a dangling source href"
    );
    assert!(with_rawml.contains("BODY_PRESERVED"));
    assert!(
        with_rawml.contains("Body child"),
        "suppressed cover nav child was pruned"
    );
    assert_eq!(
        with_cover_nav.ncx_depths(),
        vec![0],
        "cover navigation child was not promoted after pruning"
    );

    let without_cover_nav = convert_epub(cover_recipe(CoverRecipe {
        cover_page: true,
        cover_navigation: false,
        cover_first: false,
        ordinary_svg: false,
        padding_images: 0,
    }));
    let without_rawml = String::from_utf8(without_cover_nav.rawml()).unwrap();
    assert!(!without_rawml.contains("COVER_PAGE_SENTINEL"));
    assert!(without_rawml.contains("Body"));
    assert!(!without_rawml.contains("Body child"));
}

#[test]
fn omitted_cover_bodymatter_landmarks_target_first_linear_section() {
    let azw3 = convert_epub(cover_bodymatter_landmark_recipe(false));
    let rawml = String::from_utf8(azw3.rawml()).expect("recipe RawML is UTF-8");
    assert!(!rawml.contains("COVER_XHTML_SENTINEL"));
    assert!(!rawml.contains("cover.xhtml"));
    assert!(rawml.contains("PAGE1_SENTINEL"));
    assert!(rawml.contains("PAGE2_SENTINEL"));

    let page1 = rawml.find("PAGE1_SENTINEL").expect("page 1 marker");
    let page2 = rawml.find("PAGE2_SENTINEL").expect("page 2 marker");
    let first_section = rawml.find("<html").expect("first body section");
    let start_reading = azw3.exth().u32(116).expect("Start Reading target") as usize;
    assert!(first_section <= start_reading && start_reading < page2);
    assert!(start_reading <= page1);

    let guide_targets = azw3.guide_targets();
    assert_eq!(guide_targets.len(), 3);
    assert!(
        guide_targets.iter().all(|(_, sequence, _)| *sequence == 0),
        "Guide bodymatter/text/body targets must resolve to page1's first fragment"
    );
    assert_eq!(azw3.index_report(azw3.mobi().guide).entry_count, 3);
    assert_eq!(azw3.exth().u32(201), Some(0));
    assert_eq!(azw3.exth().u32(202), Some(1));
    assert_eq!(azw3.exth().text(129).as_deref(), Some("kindle:embed:0001"));
    assert_eq!(azw3.image_records().len(), 2);
    let cover_record = azw3.mobi().first_image;
    assert_eq!(image_dimensions(azw3.record(cover_record)), (660, 940));
    for kind in [122, 123, 124, 126, 127, 128] {
        assert_eq!(azw3.exth().get(kind), None, "reflowable EXTH {kind} leaked");
    }
}

#[test]
fn fixed_layout_comic_metadata_and_cover_resource_contract_is_preserved() {
    for declaration in [
        FixedLayoutDeclaration::FixedLayoutTrue,
        FixedLayoutDeclaration::RenditionPrePaginated,
    ] {
        let epub = fixed_layout_comic_recipe(declaration);
        assert_fixed_layout_comic_source_contract(&epub, declaration);
        assert_fixed_layout_comic_contract(&epub);
    }
}

fn assert_fixed_layout_comic_source_contract(epub: &[u8], declaration: FixedLayoutDeclaration) {
    let mut archive = ZipArchive::new(Cursor::new(epub)).expect("comic fixture ZIP");
    let mut package = String::new();
    archive
        .by_name("package.opf")
        .expect("comic fixture package")
        .read_to_string(&mut package)
        .expect("comic fixture package is UTF-8");
    assert!(package.contains("<meta name=\"book-type\" content=\"comic\"/>"));
    assert!(package.contains(
        "<item id=\"image1\" href=\"image1.jpg\" media-type=\"image/jpeg\" properties=\"cover-image\"/>"
    ));
    assert!(package.contains(
        "<spine page-progression-direction=\"rtl\"><itemref idref=\"cover\" linear=\"yes\"/><itemref idref=\"page1\" linear=\"yes\"/><itemref idref=\"page2\" linear=\"yes\"/></spine>"
    ));
    let expected_layout = match declaration {
        FixedLayoutDeclaration::FixedLayoutTrue => "<meta name=\"fixed-layout\">true</meta>",
        FixedLayoutDeclaration::RenditionPrePaginated => {
            "<meta property=\"rendition:layout\">pre-paginated</meta>"
        }
    };
    assert!(package.contains(expected_layout));
    let mut cover = String::new();
    archive
        .by_name("cover.xhtml")
        .expect("comic fixture cover document")
        .read_to_string(&mut cover)
        .expect("comic fixture cover is UTF-8");
    assert!(cover.contains("epub:type=\"cover\""));
    assert!(cover.contains("src=\"image1.jpg\""));
}

fn assert_fixed_layout_comic_contract(epub: &[u8]) {
    let azw3 = convert_epub(epub.to_vec());
    let exth = azw3.exth();
    assert_eq!(exth.text(122).as_deref(), Some("true"));
    assert_eq!(exth.text(123).as_deref(), Some("comic"));
    assert_eq!(exth.text(124).as_deref(), Some("none"));
    assert_eq!(exth.text(126).as_deref(), Some("1125x1600"));
    assert_eq!(exth.get(127), None, "B-13: zero-gutter is not synthesized");
    assert_eq!(exth.get(128), None, "B-14: zero-margin is not synthesized");
    assert_eq!(exth.text(525).as_deref(), Some("horizontal-rl"));
    assert_eq!(exth.text(527).as_deref(), Some("rtl"));

    let rawml = String::from_utf8(azw3.rawml()).expect("fixed-layout RawML is UTF-8");
    assert!(rawml.contains("COVER_FIXED_LAYOUT_SENTINEL"));
    assert!(rawml.contains("kindle:flow:0002?mime=image/svg+xml"));
    assert!(rawml.contains("FIXED_PAGE1_SENTINEL"));
    assert!(rawml.contains("FIXED_PAGE2_SENTINEL"));

    let ranges = azw3.fdst_ranges();
    let svg_flows = ranges
        .iter()
        .skip(1)
        .filter(|(start, end)| rawml[*start..*end].contains("<svg"))
        .collect::<Vec<_>>();
    assert_eq!(svg_flows.len(), 3);
    assert!(rawml[svg_flows[0].0..svg_flows[0].1].contains("kindle:embed:0001"));

    let guide_targets = azw3.guide_targets();
    assert_eq!(guide_targets.len(), 1);
    assert_eq!(guide_targets[0].0, "text");
    assert_eq!(
        guide_targets[0].1, 0,
        "cover is first in source spine order"
    );
    assert_eq!(azw3.index_report(azw3.mobi().guide).entry_count, 1);
    assert_eq!(exth.u32(201), Some(0));
    assert_eq!(exth.u32(202), Some(2));
    assert_eq!(exth.text(129).as_deref(), Some("kindle:embed:0002"));

    let mobi = azw3.mobi();
    let images = azw3.image_records();
    assert_eq!(images.len(), 3);
    assert_eq!(mobi.first_image, images[0].0);

    let cover_offset = exth.u32(201).expect("EXTH 201 cover address");
    let thumbnail_offset = exth.u32(202).expect("EXTH 202 thumbnail address");
    let cover_record = mobi.first_image + cover_offset as usize;
    let thumbnail_record = mobi.first_image + thumbnail_offset as usize;
    assert_eq!(cover_record, images[0].0);
    assert_eq!(thumbnail_record, images[2].0);
    assert_eq!(image_dimensions(azw3.record(cover_record)), (400, 600));
    assert_eq!(image_dimensions(azw3.record(images[1].0)), (320, 480));
    assert_eq!(image_dimensions(azw3.record(thumbnail_record)), (313, 470));
}

#[test]
fn missing_non_cover_landmark_target_still_errors() {
    let error = convert_bytes(
        &cover_bodymatter_landmark_recipe(true),
        &ConvertOptions::default(),
    )
    .expect_err("missing non-cover landmark target must not be rewritten");
    let message = error.to_string();
    assert!(message.contains("position target does not resolve"));
    assert!(message.contains("missing.xhtml"));
}

fn kindle_base32_encode(mut value: u32) -> String {
    const DIGITS: &[u8; 32] = b"0123456789ABCDEFGHIJKLMNOPQRSTUV";
    let mut encoded = Vec::new();
    while value != 0 {
        encoded.push(DIGITS[(value % 32) as usize]);
        value /= 32;
    }
    if encoded.is_empty() {
        encoded.push(b'0');
    }
    while encoded.len() < 4 {
        encoded.push(b'0');
    }
    encoded.reverse();
    String::from_utf8(encoded).expect("Kindle base32 digits are ASCII")
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

fn image_dimensions(data: &[u8]) -> (u32, u32) {
    let image = ImageReader::new(Cursor::new(data))
        .with_guessed_format()
        .expect("image format")
        .decode()
        .expect("decodable bounded image");
    (image.width(), image.height())
}
