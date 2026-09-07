use std::path::PathBuf;
use std::process::Command;

use aozoraepub3_to_azw3::{Compression, ConvertOptions, convert_bytes, convert_file};

fn fixture() -> Vec<u8> {
    let path = "tests/fixtures/public-api-and-cli/source.epub";
    std::fs::read(path).unwrap_or_else(|error| {
        panic!("required repository fixture is missing or unreadable at {path}: {error}")
    })
}

fn unique_directory() -> PathBuf {
    std::env::temp_dir().join(format!(
        "AozoraEpub3-to-azw3-test-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock is after the Unix epoch")
            .as_nanos()
    ))
}

fn first_record(bytes: &[u8]) -> usize {
    u32::from_be_bytes(bytes[78..82].try_into().expect("PalmDB record offset")) as usize
}

#[test]
fn public_apis_compression_and_cli_convert_the_fixture() {
    let fixture = fixture();
    let default = ConvertOptions::default();
    let bytes = convert_bytes(&fixture, &default).expect("byte API conversion");
    assert_eq!(
        u16::from_be_bytes(bytes[first_record(&bytes)..][..2].try_into().unwrap()),
        2
    );

    let directory = unique_directory();
    std::fs::create_dir_all(&directory).expect("create unique test directory");
    let input = directory.join("fixture.epub");
    let file_output = directory.join("file.azw3");
    std::fs::write(&input, &fixture).expect("write test fixture");
    convert_file(&input, &file_output, &default).expect("file API conversion");
    assert_eq!(std::fs::read(&file_output).unwrap(), bytes);

    let uncompressed = convert_bytes(
        &fixture,
        &ConvertOptions {
            compression: Compression::None,
        },
    )
    .expect("-c0 conversion");
    assert_eq!(
        u16::from_be_bytes(
            uncompressed[first_record(&uncompressed)..][..2]
                .try_into()
                .unwrap()
        ),
        1
    );
    assert_ne!(uncompressed, bytes);

    let binary = env!("CARGO_BIN_EXE_AozoraEpub3-to-azw3");
    let cli_default = directory.join("cli-default.azw3");
    let output = Command::new(binary)
        .args([
            input.as_os_str(),
            "-o".as_ref(),
            cli_default.as_os_str(),
            "-c1".as_ref(),
            "-verbose".as_ref(),
            "-dont_append_source".as_ref(),
            "-donotaddsource".as_ref(),
        ])
        .output()
        .expect("run c1 CLI");
    assert!(output.status.success());
    assert!(!output.stderr.is_empty(), "verbose output is missing");
    assert_eq!(std::fs::read(&cli_default).unwrap(), bytes);

    let cli_uncompressed = directory.join("cli-c0.azw3");
    assert!(
        Command::new(binary)
            .args([
                input.as_os_str(),
                "-o".as_ref(),
                cli_uncompressed.as_os_str(),
                "-c0".as_ref()
            ])
            .status()
            .expect("run c0 CLI")
            .success()
    );
    assert_eq!(std::fs::read(&cli_uncompressed).unwrap(), uncompressed);
    let derived = input.with_extension("azw3");
    assert!(
        Command::new(binary)
            .args([input.as_os_str(), "-c1".as_ref()])
            .status()
            .expect("run derived-output CLI")
            .success()
    );
    assert_eq!(std::fs::read(&derived).unwrap(), bytes);
    assert!(
        Command::new(binary)
            .args([input.as_os_str(), "-c1".as_ref()])
            .status()
            .expect("overwrite derived output")
            .success()
    );
    assert!(
        !Command::new(binary)
            .args([input.as_os_str(), "-c0".as_ref(), "-c1".as_ref()])
            .status()
            .expect("run conflicting compression CLI")
            .success()
    );
    assert!(
        !Command::new(binary)
            .args([input.as_os_str(), "-c2".as_ref()])
            .status()
            .expect("run unsupported c2 CLI")
            .success()
    );

    std::fs::remove_dir_all(&directory).expect("remove exact test directory");
}

#[test]
fn independent_in_memory_conversions_are_thread_safe() {
    let fixture = fixture();
    let workers = (0..4)
        .map(|_| {
            let fixture = fixture.clone();
            std::thread::spawn(move || convert_bytes(&fixture, &ConvertOptions::default()))
        })
        .collect::<Vec<_>>();
    let outputs = workers
        .into_iter()
        .map(|worker| {
            worker
                .join()
                .expect("conversion thread did not panic")
                .expect("thread conversion")
        })
        .collect::<Vec<_>>();
    assert!(outputs.windows(2).all(|pair| pair[0] == pair[1]));
}
