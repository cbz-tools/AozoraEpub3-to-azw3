//! Convert AozoraEpub3-generated EPUB files into KF8-only AZW3 files.
//!
//! [`convert_bytes`] returns an in-memory result, while [`convert_file`] writes
//! directly to the destination. Calls may run concurrently, except that callers
//! must coordinate writes to the same output path.
//!
//! The public conversion functions accept EPUB bytes or files and return a complete
//! AZW3 result or a stage-specific [`Error`].
#![warn(missing_docs)]

mod api;
mod book;
mod container;
mod epub;
mod error;
mod kf8;
mod kindle;
mod xhtml;

pub use api::{Compression, ConvertOptions, convert_bytes, convert_file};
pub use error::{Error, Result};
