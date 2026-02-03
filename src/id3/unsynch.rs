use crate::common::error::Result;

/// Decode unsynchronised data.
/// Removes 0x00 bytes that follow 0xFF bytes.
/// In ID3v2, 0xFF 0x00 sequences are used to avoid false sync signals.
pub fn decode(data: &[u8]) -> Result<Vec<u8>> {
    if data.is_empty() {
        return Ok(Vec::new());
    }

    let mut output = Vec::with_capacity(data.len());
    let mut i = 0;
    while i < data.len() {
        output.push(data[i]);
        if data[i] == 0xFF && i + 1 < data.len() && data[i + 1] == 0x00 {
            // Skip the 0x00 byte after 0xFF
            i += 2;
        } else {
            i += 1;
        }
    }
    Ok(output)
}

/// Encode data with unsynchronisation.
/// Inserts 0x00 after every 0xFF byte.
pub fn encode(data: &[u8]) -> Vec<u8> {
    if data.is_empty() {
        return Vec::new();
    }

    let mut output = Vec::with_capacity(data.len() + data.len() / 10);
    for &b in data {
        output.push(b);
        if b == 0xFF {
            output.push(0x00);
        }
    }
    output
}
