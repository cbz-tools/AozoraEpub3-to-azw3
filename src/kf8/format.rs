//! Small KF8 formatting primitives shared by orchestration and transport code.

use crate::error::Result;

pub(crate) fn to_base32(value: u32) -> String {
    const DIGITS: &[u8; 32] = b"0123456789ABCDEFGHIJKLMNOPQRSTUV";
    let mut value = value;
    let mut digits = Vec::new();
    while value != 0 {
        digits.push(DIGITS[(value % 32) as usize]);
        value /= 32;
    }
    if digits.is_empty() {
        digits.push(b'0');
    }
    while digits.len() < 4 {
        digits.push(b'0');
    }
    digits.reverse();
    String::from_utf8(digits).expect("base32 alphabet is ASCII")
}

pub(crate) fn to_base32_fixed(value: u32, width: usize) -> Result<String> {
    let encoded = to_base32(value);
    if encoded.len() > width {
        return Err(crate::error::Error::Output(
            "base32 value exceeds fixed KF8 field width".to_owned(),
        ));
    }
    Ok(format!("{:0>width$}", encoded, width = width))
}
