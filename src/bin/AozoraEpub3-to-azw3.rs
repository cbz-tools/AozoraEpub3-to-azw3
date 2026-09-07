use std::path::PathBuf;

use aozoraepub3_to_azw3::{Compression, ConvertOptions, convert_file};

fn usage() -> &'static str {
    "Usage: AozoraEpub3-to-azw3 <input.epub> [-o <output.azw3>] [-c0 | -c1] [-verbose] [-dont_append_source] [-donotaddsource]"
}

fn main() {
    let mut input = None;
    let mut output = None;
    let mut compression = Compression::PalmDoc;
    let mut compression_selected = false;
    let mut verbose = false;
    let mut arguments = std::env::args_os().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.to_string_lossy().as_ref() {
            "-h" | "--help" => {
                println!("{}", usage());
                return;
            }
            "-V" | "--version" => {
                println!("{}", env!("CARGO_PKG_VERSION"));
                return;
            }
            "-o" => match arguments.next() {
                Some(path) => output = Some(PathBuf::from(path)),
                None => return fail("-o requires an output path"),
            },
            "-c0" => {
                if compression_selected {
                    return fail("-c0 and -c1 cannot be combined");
                }
                compression = Compression::None;
                compression_selected = true;
            }
            "-c1" => {
                if compression_selected {
                    return fail("-c0 and -c1 cannot be combined");
                }
                compression = Compression::PalmDoc;
                compression_selected = true;
            }
            "-c2" => return fail("-c2 / HUFF-CDIC compression is not supported"),
            "-verbose" => verbose = true,
            "-dont_append_source" | "-donotaddsource" => {}
            value if value.starts_with('-') => {
                return fail(&format!("unsupported option: {value}"));
            }
            _ if input.is_none() => input = Some(PathBuf::from(argument)),
            _ => return fail("only one EPUB input path is supported"),
        }
    }
    let Some(input) = input else {
        return fail("an EPUB input path is required");
    };
    let output = output.unwrap_or_else(|| input.with_extension("azw3"));
    if verbose {
        eprintln!("EPUB open / package parse: {}", input.display());
        eprintln!("metadata, spine, resources, XHTML, and CSS: processing");
        eprintln!("Kindle CSS projection and normalization: processing");
        eprintln!("RawML, SKEL, FRAG, FDST, INDX, NCX: building");
        eprintln!(
            "compression: {}",
            match compression {
                Compression::None => "none (-c0)",
                Compression::PalmDoc => "PalmDOC (-c1)",
            }
        );
    }
    if let Err(error) = convert_file(&input, &output, &ConvertOptions { compression }) {
        return fail(&error.to_string());
    }
    if verbose {
        match std::fs::metadata(&output) {
            Ok(metadata) => eprintln!(
                "compression/text/resource records and PalmDB build complete: {} bytes -> {}",
                metadata.len(),
                output.display()
            ),
            Err(_) => eprintln!("PalmDB build complete: {}", output.display()),
        }
    }
}

fn fail(message: &str) {
    eprintln!("error: {message}\n{}", usage());
    std::process::exit(2);
}
