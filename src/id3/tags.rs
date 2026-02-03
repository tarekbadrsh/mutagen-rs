use std::collections::HashMap;
use crate::common::error::{MutagenError, Result};
use crate::id3::header::{ID3Header, BitPaddedInt, determine_bpi};
use crate::id3::frames::{self, Frame, HashKey, convert_v22_frame_id, parse_v22_picture_frame};
use crate::id3::unsynch;

/// A lazy frame that stores raw data and decodes on first access.
#[derive(Debug, Clone)]
pub enum LazyFrame {
    /// Already decoded frame.
    Decoded(Frame),
    /// Raw frame data that hasn't been decoded yet.
    Raw { id: String, data: Vec<u8> },
}

impl LazyFrame {
    /// Get the hash key without full decoding.
    pub fn hash_key(&self) -> HashKey {
        match self {
            LazyFrame::Decoded(f) => f.hash_key(),
            LazyFrame::Raw { id, data } => {
                // For most frames, the hash key is just the ID.
                // For TXXX, WXXX, COMM, USLT, APIC, POPM we'd need to decode.
                // Optimistic: just use the ID for now; we decode on access.
                match id.as_str() {
                    "TXXX" | "WXXX" | "COMM" | "USLT" | "APIC" | "POPM" => {
                        // Must decode to get proper hash key
                        match frames::parse_frame(id, data) {
                            Ok(f) => f.hash_key(),
                            Err(_) => HashKey::new(id),
                        }
                    }
                    _ => HashKey::new(id),
                }
            }
        }
    }

    /// Get the frame ID.
    pub fn frame_id(&self) -> &str {
        match self {
            LazyFrame::Decoded(f) => f.frame_id(),
            LazyFrame::Raw { id, .. } => id,
        }
    }

    /// Force decode the frame, returning a reference to the decoded frame.
    pub fn decode(&mut self) -> Result<&Frame> {
        if let LazyFrame::Raw { id, data } = self {
            let frame = frames::parse_frame(id, data)?;
            *self = LazyFrame::Decoded(frame);
        }
        match self {
            LazyFrame::Decoded(f) => Ok(f),
            _ => unreachable!(),
        }
    }

    /// Get the decoded frame, decoding if necessary.
    pub fn get_decoded(&self) -> Option<&Frame> {
        match self {
            LazyFrame::Decoded(f) => Some(f),
            _ => None,
        }
    }

    /// Force decode and return the frame (consuming self).
    pub fn into_decoded(self) -> Result<Frame> {
        match self {
            LazyFrame::Decoded(f) => Ok(f),
            LazyFrame::Raw { id, data } => frames::parse_frame(&id, &data),
        }
    }
}

/// Container for ID3v2 frames, providing dict-like access.
#[derive(Debug, Clone)]
pub struct ID3Tags {
    pub frames: HashMap<HashKey, Vec<LazyFrame>>,
    pub version: (u8, u8),
    pub unknown_frames: Vec<(String, Vec<u8>)>,
}

impl ID3Tags {
    pub fn new() -> Self {
        ID3Tags {
            frames: HashMap::with_capacity(16),
            version: (4, 0),
            unknown_frames: Vec::new(),
        }
    }

    /// Add a decoded frame.
    pub fn add(&mut self, frame: Frame) {
        let key = frame.hash_key();
        self.frames.entry(key).or_insert_with(Vec::new).push(LazyFrame::Decoded(frame));
    }

    /// Add a raw (lazy) frame.
    pub fn add_raw(&mut self, id: String, data: Vec<u8>) {
        let lazy = LazyFrame::Raw { id: id.clone(), data };
        let key = lazy.hash_key();
        self.frames.entry(key).or_insert_with(Vec::new).push(lazy);
    }

    /// Get all frames with the given key (forces decode).
    pub fn getall(&self, key: &str) -> Vec<&Frame> {
        let hash_key = HashKey::new(key);
        match self.frames.get(&hash_key) {
            Some(frames) => {
                frames.iter().filter_map(|lf| {
                    match lf {
                        LazyFrame::Decoded(f) => Some(f),
                        LazyFrame::Raw { id, data } => {
                            // Can't decode in immutable context, skip raw frames
                            None
                        }
                    }
                }).collect()
            }
            None => vec![],
        }
    }

    /// Get all frames with given key, decoding if needed (mutable version).
    pub fn getall_mut(&mut self, key: &str) -> Vec<&Frame> {
        let hash_key = HashKey::new(key);
        if let Some(frames) = self.frames.get_mut(&hash_key) {
            for lf in frames.iter_mut() {
                let _ = lf.decode();
            }
        }
        self.getall(key)
    }

    /// Get the first frame with the given key (forces decode).
    pub fn get(&self, key: &str) -> Option<&Frame> {
        self.getall(key).into_iter().next()
    }

    /// Get first frame, decoding if needed.
    pub fn get_mut(&mut self, key: &str) -> Option<&Frame> {
        let hash_key = HashKey::new(key);
        if let Some(frames) = self.frames.get_mut(&hash_key) {
            if let Some(lf) = frames.first_mut() {
                let _ = lf.decode();
            }
        }
        self.get(key)
    }

    /// Set all frames for a given key (replaces existing).
    pub fn setall(&mut self, key: &str, frames_list: Vec<Frame>) {
        let hash_key = HashKey::new(key);
        self.frames.insert(hash_key, frames_list.into_iter().map(LazyFrame::Decoded).collect());
    }

    /// Delete all frames with the given key.
    pub fn delall(&mut self, key: &str) {
        let hash_key = HashKey::new(key);
        self.frames.remove(&hash_key);
    }

    /// Get all keys.
    pub fn keys(&self) -> Vec<String> {
        self.frames.keys().map(|k| k.0.clone()).collect()
    }

    /// Get all decoded frames as a flat list.
    pub fn values(&self) -> Vec<&Frame> {
        self.frames.values().flat_map(|v| {
            v.iter().filter_map(|lf| lf.get_decoded())
        }).collect()
    }

    /// Decode all frames and return as flat list.
    pub fn values_decoded(&mut self) -> Vec<&Frame> {
        // First decode all
        for frames in self.frames.values_mut() {
            for lf in frames.iter_mut() {
                let _ = lf.decode();
            }
        }
        self.values()
    }

    /// Number of unique keys.
    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// Parse frames from raw tag data.
    pub fn read_frames(&mut self, data: &[u8], header: &ID3Header) -> Result<()> {
        let version = header.version.0;
        let mut offset = 0usize;

        // Handle extended header
        if header.flags.extended && version >= 3 {
            if data.len() < 4 {
                return Ok(());
            }
            let ext_size = if version == 4 {
                BitPaddedInt::syncsafe(&data[0..4]) as usize
            } else {
                u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize
            };
            offset = if version == 4 {
                ext_size
            } else {
                ext_size + 4
            };
            if offset >= data.len() {
                return Ok(());
            }
        }

        // Determine BPI for v2.4
        let bpi = if version == 4 {
            determine_bpi(&data[offset..], data.len())
        } else {
            8
        };

        self.version = header.version;

        if version == 2 {
            self.read_v22_frames(data, offset)?;
        } else {
            self.read_v23_v24_frames(data, offset, version, bpi)?;
        }

        Ok(())
    }

    /// Read v2.2 frames (6-byte headers).
    fn read_v22_frames(&mut self, data: &[u8], mut offset: usize) -> Result<()> {
        while offset + 6 <= data.len() {
            if data[offset] == 0 {
                break;
            }

            let id_bytes = &data[offset..offset + 3];
            if !id_bytes.iter().all(|&b| b.is_ascii_uppercase() || b.is_ascii_digit()) {
                break;
            }

            let id = std::str::from_utf8(id_bytes).unwrap_or("XXX").to_string();
            let size = ((data[offset + 3] as usize) << 16)
                | ((data[offset + 4] as usize) << 8)
                | (data[offset + 5] as usize);

            offset += 6;

            if size == 0 || offset + size > data.len() {
                break;
            }

            let frame_data = &data[offset..offset + size];
            offset += size;

            if id == "PIC" {
                match parse_v22_picture_frame(frame_data) {
                    Ok(frame) => self.add(frame),
                    Err(_) => {}
                }
                continue;
            }

            let v24_id = match convert_v22_frame_id(&id) {
                Some(new_id) => new_id.to_string(),
                None => {
                    self.unknown_frames.push((id, frame_data.to_vec()));
                    continue;
                }
            };

            // Store as lazy (raw) frame
            self.add_raw(v24_id, frame_data.to_vec());
        }

        Ok(())
    }

    /// Read v2.3/v2.4 frames (10-byte headers).
    fn read_v23_v24_frames(
        &mut self,
        data: &[u8],
        mut offset: usize,
        version: u8,
        bpi: u8,
    ) -> Result<()> {
        while offset + 10 <= data.len() {
            if data[offset] == 0 {
                break;
            }

            let id_bytes = &data[offset..offset + 4];
            if !id_bytes
                .iter()
                .all(|&b| b.is_ascii_uppercase() || b.is_ascii_digit())
            {
                break;
            }

            let id = std::str::from_utf8(id_bytes)
                .unwrap_or("XXXX")
                .to_string();

            let size = BitPaddedInt::decode(&data[offset + 4..offset + 8], bpi) as usize;
            let flags = u16::from_be_bytes([data[offset + 8], data[offset + 9]]);

            offset += 10;

            if size == 0 || offset + size > data.len() {
                break;
            }

            let mut frame_data = data[offset..offset + size].to_vec();
            offset += size;

            // Handle frame-level flags
            let (compressed, encrypted, unsynchronised, has_data_length) = if version == 4 {
                (
                    flags & 0x0008 != 0,
                    flags & 0x0004 != 0,
                    flags & 0x0002 != 0,
                    flags & 0x0001 != 0,
                )
            } else {
                (
                    flags & 0x0080 != 0,
                    flags & 0x0040 != 0,
                    false,
                    flags & 0x0080 != 0,
                )
            };

            if encrypted {
                self.unknown_frames.push((id, frame_data));
                continue;
            }

            if has_data_length && frame_data.len() >= 4 {
                frame_data = frame_data[4..].to_vec();
            }

            if unsynchronised {
                frame_data = unsynch::decode(&frame_data)?;
            }

            if compressed {
                match decompress_zlib(&frame_data) {
                    Ok(decompressed) => frame_data = decompressed,
                    Err(_) => {
                        self.unknown_frames.push((id, frame_data));
                        continue;
                    }
                }
            }

            // Store as lazy (raw) frame - don't decode until accessed
            self.add_raw(id, frame_data);
        }

        Ok(())
    }

    /// Serialize all frames to bytes for writing.
    pub fn render(&self, version: u8) -> Result<Vec<u8>> {
        let mut data = Vec::with_capacity(4096);

        for frames_list in self.frames.values() {
            for lf in frames_list {
                let (id, frame_data) = match lf {
                    LazyFrame::Decoded(frame) => {
                        (frame.frame_id().to_string(), frame.write_data(version)?)
                    }
                    LazyFrame::Raw { id, data } => {
                        // Re-serialize raw data as-is
                        (id.clone(), data.clone())
                    }
                };

                if version == 4 {
                    data.extend_from_slice(id.as_bytes());
                    data.extend_from_slice(&BitPaddedInt::encode(
                        frame_data.len() as u32,
                        4,
                        7,
                    ));
                    data.extend_from_slice(&[0u8; 2]);
                    data.extend_from_slice(&frame_data);
                } else {
                    data.extend_from_slice(id.as_bytes());
                    data.extend_from_slice(&(frame_data.len() as u32).to_be_bytes());
                    data.extend_from_slice(&[0u8; 2]);
                    data.extend_from_slice(&frame_data);
                }
            }
        }

        Ok(data)
    }
}

fn decompress_zlib(data: &[u8]) -> Result<Vec<u8>> {
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    let mut decoder = ZlibDecoder::new(data);
    let mut result = Vec::new();
    decoder
        .read_to_end(&mut result)
        .map_err(|_| MutagenError::ID3BadCompressedData)?;
    Ok(result)
}
