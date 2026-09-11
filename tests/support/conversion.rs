use std::io::Cursor;

use aozoraepub3_to_azw3::{Compression, ConvertOptions, convert_bytes};

use super::Azw3;

pub const SOVEREIGN_EPUB: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/sovereign-stars/Sovereign_Stars_Vol_1.epub"
);
pub const CRIME_EPUB: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/crime-and-punishment/source.epub"
);

pub fn convert_fixture(path: &str) -> (Vec<u8>, Azw3) {
    let input = std::fs::read(path).unwrap_or_else(|error| panic!("read {path}: {error}"));
    let output = convert_bytes(&input, &ConvertOptions::default())
        .unwrap_or_else(|error| panic!("convert {path}: {error}"));
    let inspected = Azw3::parse(output.clone());
    (input, inspected)
}

pub fn convert_epub(epub: Vec<u8>) -> Azw3 {
    let output = convert_bytes(&epub, &ConvertOptions::default()).expect("convert recipe EPUB");
    Azw3::parse(output)
}

pub fn convert_epub_uncompressed(epub: Vec<u8>) -> Azw3 {
    let output = convert_bytes(
        &epub,
        &ConvertOptions {
            compression: Compression::None,
        },
    )
    .expect("convert recipe EPUB without PalmDOC compression");
    Azw3::parse(output)
}

pub fn epub_text(path: &str) -> String {
    let bytes = std::fs::read(path).unwrap_or_else(|error| panic!("read {path}: {error}"));
    let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).expect("open EPUB ZIP");
    let mut text = String::new();
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).expect("read EPUB entry");
        if entry.name().ends_with(".xhtml")
            || entry.name().ends_with(".css")
            || entry.name().ends_with(".ncx")
            || entry.name().ends_with(".opf")
        {
            let mut value = String::new();
            std::io::Read::read_to_string(&mut entry, &mut value).expect("decode EPUB text");
            text.push_str(&value);
        }
    }
    text
}
