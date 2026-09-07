/// Controls the PalmDOC encoding used for KF8 text records.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Compression {
    /// Store text records without PalmDOC compression (KindleGen `-c0`).
    None,
    /// Use PalmDOC compression (KindleGen `-c1`).
    #[default]
    PalmDoc,
}

/// Options that affect conversion output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConvertOptions {
    /// The text-record compression mode. The default is PalmDOC.
    pub compression: Compression,
}
