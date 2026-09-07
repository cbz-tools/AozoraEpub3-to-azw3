use crate::{Compression, ConvertOptions, Error, Result, epub, kf8, kindle};

/// Convert an in-memory EPUB into a complete KF8-only AZW3 byte vector.
pub fn convert_bytes(input: &[u8], options: &ConvertOptions) -> Result<Vec<u8>> {
    let book = epub::parse_epub(input)?;
    let kindle_book = kindle::normalize(book);
    let compression = match options.compression {
        Compression::None => kf8::TextCompression::None,
        Compression::PalmDoc => kf8::TextCompression::PalmDoc,
    };
    let kf8_book =
        kf8::build(kindle_book, compression).map_err(|error| Error::Kf8Build(error.to_string()))?;
    kf8::serialize(kf8_book).map_err(|error| Error::Container(error.to_string()))
}
