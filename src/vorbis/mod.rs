use crate::common::error::{MutagenError, Result};
use std::collections::HashMap;

/// A Vorbis comment: list of key=value pairs with a vendor string.
#[derive(Debug, Clone)]
pub struct VorbisComment {
    pub vendor: String,
    pub comments: Vec<(String, String)>,
}

impl VorbisComment {
    pub fn new() -> Self {
        VorbisComment {
            vendor: String::new(),
            comments: Vec::new(),
        }
    }

    /// Parse a Vorbis comment block from bytes.
    /// `framing` controls whether to expect a framing bit at the end (true for OGG, false for FLAC).
    pub fn parse(data: &[u8], framing: bool) -> Result<Self> {
        if data.len() < 4 {
            return Err(MutagenError::InvalidData("Vorbis comment too short".into()));
        }

        let mut pos = 0;

        // Vendor string length (LE32)
        let vendor_len = u32::from_le_bytes([
            data[pos], data[pos + 1], data[pos + 2], data[pos + 3],
        ]) as usize;
        pos += 4;

        if pos + vendor_len > data.len() {
            return Err(MutagenError::InvalidData("Vendor string extends past data".into()));
        }

        let vendor = String::from_utf8_lossy(&data[pos..pos + vendor_len]).into_owned();
        pos += vendor_len;

        if pos + 4 > data.len() {
            return Err(MutagenError::InvalidData("No comment count".into()));
        }

        // Comment count (LE32)
        let count = u32::from_le_bytes([
            data[pos], data[pos + 1], data[pos + 2], data[pos + 3],
        ]) as usize;
        pos += 4;

        let mut comments = Vec::with_capacity(count);

        for _ in 0..count {
            if pos + 4 > data.len() {
                break;
            }

            let comment_len = u32::from_le_bytes([
                data[pos], data[pos + 1], data[pos + 2], data[pos + 3],
            ]) as usize;
            pos += 4;

            if pos + comment_len > data.len() {
                break;
            }

            let comment_str = String::from_utf8_lossy(&data[pos..pos + comment_len]);
            pos += comment_len;

            // Split on first '='
            if let Some(eq_pos) = comment_str.find('=') {
                let key = comment_str[..eq_pos].to_uppercase();
                let value = comment_str[eq_pos + 1..].to_string();
                comments.push((key, value));
            }
        }

        Ok(VorbisComment { vendor, comments })
    }

    /// Serialize to bytes.
    pub fn render(&self, framing: bool) -> Vec<u8> {
        let mut data = Vec::new();

        // Vendor string
        let vendor_bytes = self.vendor.as_bytes();
        data.extend_from_slice(&(vendor_bytes.len() as u32).to_le_bytes());
        data.extend_from_slice(vendor_bytes);

        // Comment count
        data.extend_from_slice(&(self.comments.len() as u32).to_le_bytes());

        // Comments
        for (key, value) in &self.comments {
            let comment = format!("{}={}", key, value);
            let comment_bytes = comment.as_bytes();
            data.extend_from_slice(&(comment_bytes.len() as u32).to_le_bytes());
            data.extend_from_slice(comment_bytes);
        }

        if framing {
            data.push(1); // framing bit
        }

        data
    }

    /// Get as a case-insensitive dict (keys are uppercase).
    pub fn as_dict(&self) -> HashMap<String, Vec<String>> {
        let mut dict: HashMap<String, Vec<String>> = HashMap::new();
        for (key, value) in &self.comments {
            dict.entry(key.to_uppercase())
                .or_insert_with(Vec::new)
                .push(value.clone());
        }
        dict
    }

    /// Get all values for a key (case-insensitive).
    pub fn get(&self, key: &str) -> Vec<&str> {
        let upper = key.to_uppercase();
        self.comments
            .iter()
            .filter(|(k, _)| k == &upper)
            .map(|(_, v)| v.as_str())
            .collect()
    }

    /// Set all values for a key (replaces existing).
    pub fn set(&mut self, key: &str, values: Vec<String>) {
        let upper = key.to_uppercase();
        self.comments.retain(|(k, _)| k != &upper);
        for v in values {
            self.comments.push((upper.clone(), v));
        }
    }

    /// Delete all entries for a key.
    pub fn delete(&mut self, key: &str) {
        let upper = key.to_uppercase();
        self.comments.retain(|(k, _)| k != &upper);
    }

    /// Get all unique keys.
    pub fn keys(&self) -> Vec<String> {
        let mut seen = std::collections::HashSet::new();
        let mut keys = Vec::new();
        for (k, _) in &self.comments {
            if seen.insert(k.clone()) {
                keys.push(k.clone());
            }
        }
        keys
    }
}
