use crate::common::error::{MutagenError, Result};

/// MPEG audio version
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MPEGVersion {
    V1,
    V2,
    V25,
}

impl MPEGVersion {
    pub fn as_f64(&self) -> f64 {
        match self {
            MPEGVersion::V1 => 1.0,
            MPEGVersion::V2 => 2.0,
            MPEGVersion::V25 => 2.5,
        }
    }
}

/// MPEG audio layer
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MPEGLayer {
    Layer1,
    Layer2,
    Layer3,
}

impl MPEGLayer {
    pub fn as_u8(&self) -> u8 {
        match self {
            MPEGLayer::Layer1 => 1,
            MPEGLayer::Layer2 => 2,
            MPEGLayer::Layer3 => 3,
        }
    }
}

/// Channel mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ChannelMode {
    Stereo,
    JointStereo,
    DualChannel,
    Mono,
}

impl ChannelMode {
    pub fn num_channels(&self) -> u32 {
        match self {
            ChannelMode::Mono => 1,
            _ => 2,
        }
    }
}

// Bitrate tables [version_index][layer_index][bitrate_index]
// version_index: 0=V1, 1=V2/V2.5
// layer_index: 0=Layer1, 1=Layer2, 2=Layer3
const BITRATES: [[[u32; 16]; 3]; 2] = [
    // MPEG1
    [
        // Layer 1
        [0, 32, 64, 96, 128, 160, 192, 224, 256, 288, 320, 352, 384, 416, 448, 0],
        // Layer 2
        [0, 32, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 384, 0],
        // Layer 3
        [0, 32, 40, 48, 56, 64, 80, 96, 112, 128, 160, 192, 224, 256, 320, 0],
    ],
    // MPEG2, MPEG2.5
    [
        // Layer 1
        [0, 32, 48, 56, 64, 80, 96, 112, 128, 144, 160, 176, 192, 224, 256, 0],
        // Layer 2
        [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0],
        // Layer 3
        [0, 8, 16, 24, 32, 40, 48, 56, 64, 80, 96, 112, 128, 144, 160, 0],
    ],
];

// Sample rate tables [version_index][srate_index]
const SAMPLE_RATES: [[u32; 4]; 3] = [
    // MPEG1
    [44100, 48000, 32000, 0],
    // MPEG2
    [22050, 24000, 16000, 0],
    // MPEG2.5
    [11025, 12000, 8000, 0],
];

// Samples per frame [version_index][layer_index]
const SAMPLES_PER_FRAME: [[u32; 3]; 2] = [
    // MPEG1: Layer1, Layer2, Layer3
    [384, 1152, 1152],
    // MPEG2/2.5
    [384, 1152, 576],
];

/// A parsed MPEG audio frame header.
#[derive(Debug, Clone)]
pub struct MPEGFrame {
    pub version: MPEGVersion,
    pub layer: MPEGLayer,
    pub protected: bool,
    pub bitrate: u32,      // kbps
    pub sample_rate: u32,  // Hz
    pub padding: bool,
    pub channel_mode: ChannelMode,
    pub channels: u32,
    pub frame_length: u32, // bytes
    pub samples_per_frame: u32,
}

impl MPEGFrame {
    /// Parse a 4-byte MPEG frame header.
    #[inline]
    pub fn parse(header_bytes: &[u8]) -> Result<Self> {
        if header_bytes.len() < 4 {
            return Err(MutagenError::MP3("Frame header too short".into()));
        }

        let h = u32::from_be_bytes([
            header_bytes[0],
            header_bytes[1],
            header_bytes[2],
            header_bytes[3],
        ]);

        // Sync: 11 bits of 1s
        if h & 0xFFE00000 != 0xFFE00000 {
            return Err(MutagenError::MP3("Invalid sync".into()));
        }

        // Version: bits 19-20
        let version = match (h >> 19) & 0x03 {
            0 => MPEGVersion::V25,
            2 => MPEGVersion::V2,
            3 => MPEGVersion::V1,
            _ => return Err(MutagenError::MP3("Invalid MPEG version".into())),
        };

        // Layer: bits 17-18
        let layer = match (h >> 17) & 0x03 {
            1 => MPEGLayer::Layer3,
            2 => MPEGLayer::Layer2,
            3 => MPEGLayer::Layer1,
            _ => return Err(MutagenError::MP3("Invalid MPEG layer".into())),
        };

        // Protection bit (inverted: 0 = protected)
        let protected = (h >> 16) & 0x01 == 0;

        // Bitrate index: bits 12-15
        let bitrate_idx = ((h >> 12) & 0x0F) as usize;
        let version_idx = match version {
            MPEGVersion::V1 => 0,
            _ => 1,
        };
        let layer_idx = match layer {
            MPEGLayer::Layer1 => 0,
            MPEGLayer::Layer2 => 1,
            MPEGLayer::Layer3 => 2,
        };

        let bitrate = BITRATES[version_idx][layer_idx][bitrate_idx];
        if bitrate == 0 {
            return Err(MutagenError::MP3("Invalid bitrate".into()));
        }

        // Sample rate: bits 10-11
        let srate_idx = ((h >> 10) & 0x03) as usize;
        let srate_version_idx = match version {
            MPEGVersion::V1 => 0,
            MPEGVersion::V2 => 1,
            MPEGVersion::V25 => 2,
        };
        let sample_rate = SAMPLE_RATES[srate_version_idx][srate_idx];
        if sample_rate == 0 {
            return Err(MutagenError::MP3("Invalid sample rate".into()));
        }

        // Padding: bit 9
        let padding = (h >> 9) & 0x01 != 0;

        // Channel mode: bits 6-7
        let channel_mode = match (h >> 6) & 0x03 {
            0 => ChannelMode::Stereo,
            1 => ChannelMode::JointStereo,
            2 => ChannelMode::DualChannel,
            3 => ChannelMode::Mono,
            _ => unreachable!(),
        };

        let channels = channel_mode.num_channels();
        let spf = SAMPLES_PER_FRAME[version_idx][layer_idx];

        // Calculate frame length
        let frame_length = match layer {
            MPEGLayer::Layer1 => {
                (12 * bitrate * 1000 / sample_rate + if padding { 1 } else { 0 }) * 4
            }
            _ => {
                let slot_size = 1; // bytes
                spf / 8 * bitrate * 1000 / sample_rate + if padding { slot_size } else { 0 }
            }
        };

        Ok(MPEGFrame {
            version,
            layer,
            protected,
            bitrate,
            sample_rate,
            padding,
            channel_mode,
            channels,
            frame_length,
            samples_per_frame: spf,
        })
    }
}

/// Scan for the first valid MPEG sync frame in data.
/// Returns the offset and parsed frame if found.
#[inline]
pub fn find_sync(data: &[u8], start: usize) -> Option<(usize, MPEGFrame)> {
    use memchr::memchr;

    let mut pos = start;
    while pos < data.len().saturating_sub(4) {
        // Use SIMD-accelerated search for 0xFF
        match memchr(0xFF, &data[pos..]) {
            Some(offset) => {
                pos += offset;
                if pos + 4 > data.len() {
                    return None;
                }
                // Check if this is a valid frame header
                if data[pos + 1] & 0xE0 == 0xE0 {
                    if let Ok(frame) = MPEGFrame::parse(&data[pos..pos + 4]) {
                        // Validate: check that the next frame also has valid sync
                        let next_pos = pos + frame.frame_length as usize;
                        if next_pos + 4 <= data.len() {
                            if data[next_pos] == 0xFF && data[next_pos + 1] & 0xE0 == 0xE0 {
                                return Some((pos, frame));
                            }
                        } else {
                            // Near end of file, accept without next frame validation
                            return Some((pos, frame));
                        }
                    }
                }
                pos += 1;
            }
            None => return None,
        }
    }
    None
}
