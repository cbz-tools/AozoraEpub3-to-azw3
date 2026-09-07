use std::io::BufWriter;
use std::path::Path;

use crate::{Compression, ConvertOptions, Error, Result, epub, kf8, kindle};

/// Convert one EPUB path to an AZW3 path, overwriting the destination if it exists.
pub fn convert_file(
    input: impl AsRef<Path>,
    output: impl AsRef<Path>,
    options: &ConvertOptions,
) -> Result<()> {
    let input = input.as_ref();
    if input
        .extension()
        .and_then(|extension| extension.to_str())
        .is_none_or(|extension| !extension.eq_ignore_ascii_case("epub"))
    {
        return Err(Error::UnsupportedInput(
            "only EPUB input is supported".to_owned(),
        ));
    }
    let input_bytes = std::fs::read(input).map_err(|source| Error::Io {
        path: input.display().to_string(),
        source,
    })?;
    let book = epub::parse_epub(&input_bytes)?;
    let kindle_book = kindle::normalize(book);
    let compression = match options.compression {
        Compression::None => kf8::TextCompression::None,
        Compression::PalmDoc => kf8::TextCompression::PalmDoc,
    };
    let kf8_book =
        kf8::build(kindle_book, compression).map_err(|error| Error::Kf8Build(error.to_string()))?;
    let output = output.as_ref();
    let file = std::fs::File::create(output).map_err(|source| Error::Io {
        path: output.display().to_string(),
        source,
    })?;
    let mut writer = BufWriter::new(file);
    kf8_book
        .serialize_to_writer(&mut writer, &output.display().to_string())
        .map_err(|error| match error {
            Error::Io { source, .. } => Error::Io {
                path: output.display().to_string(),
                source,
            },
            error => Error::Container(error.to_string()),
        })?;
    std::io::Write::flush(&mut writer).map_err(|source| Error::Io {
        path: output.display().to_string(),
        source,
    })
}
