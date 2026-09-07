mod support;

use std::io::{Cursor, Read};

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use support::{Azw3, convert_epub};
use zip::ZipArchive;

const EMBEDDED_FONT_EPUB: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/embedded-font/source.epub"
);

const FONT_MEDIA_TYPES: &[&str] = &[
    "application/font-sfnt",
    "application/x-font-ttf",
    "font/sfnt",
    "font/ttf",
    "font/truetype",
];

#[derive(Debug, Clone)]
struct ManifestItem {
    href: String,
    media_type: String,
}

#[test]
fn embedded_font_is_preserved_as_a_resolvable_kf8_resource() {
    let epub = std::fs::read(EMBEDDED_FONT_EPUB).expect("read embedded-font EPUB");
    let mut archive = ZipArchive::new(Cursor::new(epub.clone())).expect("open embedded-font EPUB");

    let container = read_zip_entry(&mut archive, "META-INF/container.xml");
    let rootfile = parse_rootfile(&container);
    let opf = read_zip_entry(&mut archive, &rootfile);
    let manifest = parse_manifest(&opf);
    let opf_base = rootfile.rsplit_once('/').map_or("", |(base, _)| base);

    let source_ttf_path = archive
        .file_names()
        .find(|path| path.to_ascii_lowercase().ends_with(".ttf"))
        .map(str::to_owned)
        .expect("E-12 source EPUB must contain a TTF ZIP entry");
    let source_font = read_zip_entry(&mut archive, &source_ttf_path);
    assert!(!source_font.is_empty(), "E-12 source TTF must be non-empty");
    assert_eq!(
        source_font.get(..4),
        Some(&[0x00, 0x01, 0x00, 0x00][..]),
        "E-12 source resource must be a TrueType font"
    );

    let font_item = manifest
        .iter()
        .find(|item| is_font_media_type(&item.media_type))
        .expect(
            "E-12 source EPUB OPF must manifest the embedded TTF with a recognized font media type",
        );
    assert!(
        font_item.href.to_ascii_lowercase().ends_with(".ttf"),
        "E-12 source font manifest item must identify the checked-in TTF"
    );
    let font_path = resolve_epub_path(opf_base, &font_item.href);
    assert_eq!(
        font_path, source_ttf_path,
        "E-12 OPF font manifest item must point to the source TTF"
    );

    let (css_path, css_font_target) = manifest
        .iter()
        .filter(|item| item.media_type.eq_ignore_ascii_case("text/css"))
        .find_map(|item| {
            let path = resolve_epub_path(opf_base, &item.href);
            let source = read_zip_entry(&mut archive, &path);
            let source = String::from_utf8(source).expect("source CSS is UTF-8");
            extract_font_face_url(&source).map(|target| (path, target))
        })
        .expect("E-12 source EPUB CSS must contain an @font-face rule");
    let resolved_css_font = resolve_epub_path(
        css_path.rsplit_once('/').map_or("", |(base, _)| base),
        &css_font_target,
    );
    assert_eq!(
        resolved_css_font, font_path,
        "E-12 CSS @font-face URL must resolve to the OPF font resource"
    );

    let azw3 = convert_epub(epub);
    let font_record_index = find_font_record(&azw3, &source_font);
    let font_record = azw3.record(font_record_index);
    println!(
        "E-12 source FONT: path={}, media_type={}, bytes={}; generated record={}, bytes={}",
        source_ttf_path,
        font_item.media_type,
        source_font.len(),
        font_record_index,
        font_record.len()
    );
    assert!(
        !font_record.is_empty(),
        "E-12 generated FONT record is empty"
    );
    assert!(
        font_record
            .windows(source_font.len())
            .any(|window| window == source_font.as_slice()),
        "E-12 generated FONT record must retain the source TTF payload"
    );

    assert!(
        font_record_index >= azw3.mobi().first_non_text,
        "E-12 generated FONT record must be after the text/control records"
    );
    assert!(
        font_record_index < azw3.mobi().flis,
        "E-12 generated FONT record must be before FLIS"
    );

    let generated_css = find_generated_css_with_font_reference(&azw3);
    let (embed_offset, embed_mime) = extract_embed_reference(&generated_css)
        .expect("E-12 generated CSS must reference the embedded font resource");
    assert_eq!(
        embed_mime, font_item.media_type,
        "E-12 generated CSS must preserve the font resource media type"
    );
    let mobi = azw3.mobi();
    let binary_resource_count = manifest
        .iter()
        .filter(|item| {
            !is_text_media_type(&item.media_type) && !is_css_media_type(&item.media_type)
        })
        .count();
    assert!(
        binary_resource_count > 0,
        "E-12 source has no binary resources"
    );
    let resource_record_start = mobi
        .flis
        .checked_sub(binary_resource_count)
        .expect("E-12 resource records precede FLIS");
    let embed_base = if mobi.first_image == u32::MAX as usize {
        resource_record_start
    } else {
        mobi.first_image
    };
    let resolved_record = embed_base
        .checked_add(embed_offset - 1)
        .expect("E-12 generated font reference record index does not overflow");
    assert!(
        resolved_record < azw3.record_count(),
        "E-12 generated CSS font reference resolves within the AZW3 record table"
    );
    assert_eq!(
        resolved_record, font_record_index,
        "E-12 generated CSS font reference resolves to the generated FONT record"
    );
    println!(
        "E-12 generated CSS: kindle:embed offset={}, mime={}, resolved_record={}",
        embed_offset, embed_mime, resolved_record
    );

    assert_image_geometry(&azw3);
}

fn read_zip_entry<R: Read + std::io::Seek>(archive: &mut ZipArchive<R>, path: &str) -> Vec<u8> {
    let mut entry = archive
        .by_name(path)
        .unwrap_or_else(|error| panic!("EPUB is missing {path}: {error}"));
    let mut data = Vec::new();
    entry
        .read_to_end(&mut data)
        .unwrap_or_else(|error| panic!("read EPUB entry {path}: {error}"));
    data
}

fn parse_rootfile(xml: &[u8]) -> String {
    let mut reader = Reader::from_reader(Cursor::new(xml));
    let mut buffer = Vec::new();
    loop {
        match reader
            .read_event_into(&mut buffer)
            .expect("parse container.xml")
        {
            Event::Empty(event) | Event::Start(event)
                if local_name(event.name().as_ref()) == b"rootfile" =>
            {
                return attribute(&event, b"full-path").expect("container rootfile full-path");
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    panic!("container.xml has no rootfile full-path");
}

fn parse_manifest(xml: &[u8]) -> Vec<ManifestItem> {
    let mut reader = Reader::from_reader(Cursor::new(xml));
    reader.config_mut().trim_text(true);
    let mut buffer = Vec::new();
    let mut manifest = Vec::new();
    loop {
        match reader
            .read_event_into(&mut buffer)
            .expect("parse package.opf")
        {
            Event::Empty(event) | Event::Start(event)
                if local_name(event.name().as_ref()) == b"item" =>
            {
                let Some(href) = attribute(&event, b"href") else {
                    buffer.clear();
                    continue;
                };
                let Some(media_type) = attribute(&event, b"media-type") else {
                    buffer.clear();
                    continue;
                };
                manifest.push(ManifestItem { href, media_type });
            }
            Event::Eof => break,
            _ => {}
        }
        buffer.clear();
    }
    manifest
}

fn attribute(event: &BytesStart<'_>, name: &[u8]) -> Option<String> {
    event
        .attributes()
        .filter_map(Result::ok)
        .find_map(|attribute| {
            (attribute.key.as_ref() == name).then(|| {
                attribute
                    .unescape_value()
                    .expect("OPF attribute is valid XML")
                    .into_owned()
            })
        })
}

fn local_name(name: &[u8]) -> &[u8] {
    name.rsplit(|byte| *byte == b':').next().unwrap_or(name)
}

fn is_font_media_type(media_type: &str) -> bool {
    FONT_MEDIA_TYPES
        .iter()
        .any(|candidate| media_type.eq_ignore_ascii_case(candidate))
}

fn resolve_epub_path(base: &str, href: &str) -> String {
    let combined = format!("{base}/{href}");
    let mut components = Vec::new();
    for component in combined.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                components.pop();
            }
            component => components.push(component),
        }
    }
    components.join("/")
}

fn extract_font_face_url(source: &str) -> Option<String> {
    let start = source.find("@font-face")?;
    let block = &source[start..source[start..].find('}')? + start];
    let url_start = block.find("url(")? + "url(".len();
    let url_end = block[url_start..].find(')')? + url_start;
    Some(
        block[url_start..url_end]
            .trim()
            .trim_matches(['\'', '"'])
            .to_owned(),
    )
}

fn find_font_record(azw3: &Azw3, source_font: &[u8]) -> usize {
    let mobi = azw3.mobi();
    (mobi.first_non_text..azw3.record_count())
        .find(|index| {
            azw3.record(*index)
                .windows(source_font.len())
                .any(|window| window == source_font)
        })
        .expect("E-12 generated AZW3 must contain a record carrying the source TTF")
}

fn find_generated_css_with_font_reference(azw3: &Azw3) -> String {
    let rawml = azw3.rawml();
    let generated = String::from_utf8(rawml).expect("E-12 generated RawML is UTF-8");
    assert!(
        generated.contains("@font-face") && generated.contains("kindle:embed:"),
        "E-12 generated AZW3 CSS must contain the @font-face resource reference"
    );
    generated
}

fn is_text_media_type(media_type: &str) -> bool {
    [
        "application/xhtml+xml",
        "text/html",
        "application/x-dtbncx+xml",
    ]
    .iter()
    .any(|candidate| media_type.eq_ignore_ascii_case(candidate))
}

fn is_css_media_type(media_type: &str) -> bool {
    media_type.eq_ignore_ascii_case("text/css")
}

fn extract_embed_reference(css: &str) -> Option<(usize, String)> {
    let start = css.find("kindle:embed:")? + "kindle:embed:".len();
    let end = css[start..].find(|character: char| {
        character == '?' || character == ')' || character.is_ascii_whitespace()
    })? + start;
    let offset = decode_base32(&css[start..end])?;
    let mime_start = css[end..].find("?mime=")? + end + "?mime=".len();
    let mime_end = css[mime_start..]
        .find(|character: char| {
            character == ')'
                || character == '\''
                || character == '"'
                || character.is_ascii_whitespace()
        })
        .map_or(css.len(), |relative| mime_start + relative);
    Some((offset, css[mime_start..mime_end].to_owned()))
}

fn decode_base32(value: &str) -> Option<usize> {
    value.bytes().try_fold(0usize, |decoded, digit| {
        let value = match digit {
            b'0'..=b'9' => usize::from(digit - b'0'),
            b'A'..=b'V' => usize::from(digit - b'A') + 10,
            _ => return None,
        };
        decoded.checked_mul(32)?.checked_add(value)
    })
}

fn assert_image_geometry(azw3: &Azw3) {
    let mobi = azw3.mobi();
    let image_records = azw3.image_records();
    if mobi.first_image == u32::MAX as usize {
        assert!(
            image_records.is_empty(),
            "AZW3 image range has no first image"
        );
        assert_eq!(mobi.last_image, u16::MAX as usize);
        return;
    }
    assert!(mobi.first_image <= mobi.last_image);
    assert!(mobi.last_image < azw3.record_count());
    assert_eq!(
        image_records.first().map(|(index, _)| *index),
        Some(mobi.first_image)
    );
    assert_eq!(
        image_records.last().map(|(index, _)| *index),
        Some(mobi.last_image)
    );
    assert!(
        image_records
            .iter()
            .all(|(index, data)| { *index < azw3.record_count() && !data.is_empty() })
    );
}
