use crate::error::Result;

pub(crate) fn encode_fcis(text_length: u32) -> Result<Vec<u8>> {
    // KF8 FCIS uses the evidenced KindleGen/calibre-compatible two-entry
    // shape. This field and the record length are not derived from FDST flow
    // count; the exact semantic name of field @12 is undocumented. The
    // encoder is KF8-only, so legacy/MOBI7 callers must not reuse this policy.
    let entry_count = 2usize;
    let entry_count_u32 = 2u32;
    let capacity = 52usize;
    let mut bytes = Vec::with_capacity(capacity);
    bytes.extend_from_slice(b"FCIS");
    bytes.extend_from_slice(&20u32.to_be_bytes());
    bytes.extend_from_slice(&16u32.to_be_bytes());
    bytes.extend_from_slice(&entry_count_u32.to_be_bytes());
    bytes.extend_from_slice(&0u32.to_be_bytes());
    bytes.extend_from_slice(&text_length.to_be_bytes());
    bytes.extend_from_slice(&0u32.to_be_bytes());
    let block_size: u32 = 0x28;
    bytes.extend_from_slice(&block_size.to_be_bytes());
    for _ in 1..entry_count {
        bytes.extend_from_slice(&0u32.to_be_bytes());
        bytes.extend_from_slice(&block_size.to_be_bytes());
    }
    bytes.extend_from_slice(&8u32.to_be_bytes());
    bytes.extend_from_slice(&1u16.to_be_bytes());
    bytes.extend_from_slice(&1u16.to_be_bytes());
    bytes.extend_from_slice(&0u32.to_be_bytes());
    Ok(bytes)
}

pub(crate) fn encode_eof() -> Vec<u8> {
    b"\xe9\x8e\r\n".to_vec()
}
