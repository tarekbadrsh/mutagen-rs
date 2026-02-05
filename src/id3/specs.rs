use crate::common::error::{MutagenError, Result};

/// Text encoding types used in ID3v2 frames.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Encoding {
    Latin1 = 0,
    Utf16 = 1,
    Utf16Be = 2,
    Utf8 = 3,
}

impl Encoding {
    pub fn from_byte(b: u8) -> Result<Self> {
        match b {
            0 => Ok(Encoding::Latin1),
            1 => Ok(Encoding::Utf16),
            2 => Ok(Encoding::Utf16Be),
            3 => Ok(Encoding::Utf8),
            _ => Err(MutagenError::ID3(format!("Invalid encoding byte: {}", b))),
        }
    }

    /// Default encoding for a given ID3 version.
    pub fn default_for_version(version: u8) -> Self {
        if version >= 4 {
            Encoding::Utf8
        } else {
            Encoding::Utf16
        }
    }
}

/// Decode text from bytes using the specified encoding.
pub fn decode_text(data: &[u8], encoding: Encoding) -> Result<String> {
    match encoding {
        Encoding::Latin1 => {
            // Fast path: if all bytes are ASCII, avoid per-char conversion
            if data.iter().all(|&b| b < 128) {
                // SAFETY: all bytes are valid ASCII, which is valid UTF-8
                Ok(unsafe { String::from_utf8_unchecked(data.to_vec()) })
            } else {
                Ok(data.iter().map(|&b| b as char).collect())
            }
        }
        Encoding::Utf16 => {
            if data.len() < 2 {
                return Ok(String::new());
            }
            // Check BOM
            let (decoder, start) = if data[0] == 0xFF && data[1] == 0xFE {
                (encoding_rs::UTF_16LE, 2)
            } else if data[0] == 0xFE && data[1] == 0xFF {
                (encoding_rs::UTF_16BE, 2)
            } else {
                // Default to LE if no BOM
                (encoding_rs::UTF_16LE, 0)
            };
            let (result, _, had_errors) = decoder.decode(&data[start..]);
            if had_errors {
                // Still return what we got - mutagen is lenient
            }
            Ok(result.into_owned())
        }
        Encoding::Utf16Be => {
            let (result, _, _) = encoding_rs::UTF_16BE.decode(data);
            Ok(result.into_owned())
        }
        Encoding::Utf8 => {
            // Try strict first, fall back to lossy
            match std::str::from_utf8(data) {
                Ok(s) => Ok(s.to_string()),
                Err(_) => Ok(String::from_utf8_lossy(data).into_owned()),
            }
        }
    }
}

/// Encode text to bytes using the specified encoding.
pub fn encode_text(text: &str, encoding: Encoding) -> Vec<u8> {
    match encoding {
        Encoding::Latin1 => {
            text.chars().map(|c| {
                if c as u32 <= 0xFF { c as u8 } else { b'?' }
            }).collect()
        }
        Encoding::Utf16 => {
            let mut result = vec![0xFF, 0xFE]; // BOM (LE)
            for c in text.encode_utf16() {
                result.extend_from_slice(&c.to_le_bytes());
            }
            result
        }
        Encoding::Utf16Be => {
            let mut result = Vec::new();
            for c in text.encode_utf16() {
                result.extend_from_slice(&c.to_be_bytes());
            }
            result
        }
        Encoding::Utf8 => {
            text.as_bytes().to_vec()
        }
    }
}

/// Find the null terminator for the given encoding.
/// Returns the position of the null terminator (not including it).
pub fn find_null_terminator(data: &[u8], encoding: Encoding) -> Option<usize> {
    match encoding {
        Encoding::Latin1 | Encoding::Utf8 => {
            data.iter().position(|&b| b == 0)
        }
        Encoding::Utf16 | Encoding::Utf16Be => {
            let mut i = 0;
            while i + 1 < data.len() {
                if data[i] == 0 && data[i + 1] == 0 {
                    return Some(i);
                }
                i += 2;
            }
            None
        }
    }
}

/// Size of the null terminator for each encoding.
pub fn null_terminator_size(encoding: Encoding) -> usize {
    match encoding {
        Encoding::Latin1 | Encoding::Utf8 => 1,
        Encoding::Utf16 | Encoding::Utf16Be => 2,
    }
}

/// Read encoded text from data, returning (text, bytes_consumed).
/// The text is terminated by null or end of data.
pub fn read_encoded_text(data: &[u8], encoding: Encoding) -> Result<(String, usize)> {
    let term_size = null_terminator_size(encoding);
    match find_null_terminator(data, encoding) {
        Some(pos) => {
            let text = decode_text(&data[..pos], encoding)?;
            Ok((text, pos + term_size))
        }
        None => {
            let text = decode_text(data, encoding)?;
            Ok((text, data.len()))
        }
    }
}

/// Read Latin1 text (no encoding byte prefix).
pub fn read_latin1_text(data: &[u8]) -> Result<(String, usize)> {
    match data.iter().position(|&b| b == 0) {
        Some(pos) => {
            let text = decode_text(&data[..pos], Encoding::Latin1)?;
            Ok((text, pos + 1))
        }
        None => {
            let text = decode_text(data, Encoding::Latin1)?;
            Ok((text, data.len()))
        }
    }
}

/// Picture type enum matching ID3v2 APIC frame specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum PictureType {
    Other = 0,
    FileIcon = 1,
    OtherFileIcon = 2,
    CoverFront = 3,
    CoverBack = 4,
    LeafletPage = 5,
    Media = 6,
    LeadArtist = 7,
    Artist = 8,
    Conductor = 9,
    Band = 10,
    Composer = 11,
    Lyricist = 12,
    RecordingLocation = 13,
    DuringRecording = 14,
    DuringPerformance = 15,
    MovieCapture = 16,
    AFishEvenBrighter = 17,
    Illustration = 18,
    BandLogo = 19,
    PublisherLogo = 20,
}

impl PictureType {
    pub fn from_byte(b: u8) -> Self {
        match b {
            0 => PictureType::Other,
            1 => PictureType::FileIcon,
            2 => PictureType::OtherFileIcon,
            3 => PictureType::CoverFront,
            4 => PictureType::CoverBack,
            5 => PictureType::LeafletPage,
            6 => PictureType::Media,
            7 => PictureType::LeadArtist,
            8 => PictureType::Artist,
            9 => PictureType::Conductor,
            10 => PictureType::Band,
            11 => PictureType::Composer,
            12 => PictureType::Lyricist,
            13 => PictureType::RecordingLocation,
            14 => PictureType::DuringRecording,
            15 => PictureType::DuringPerformance,
            16 => PictureType::MovieCapture,
            17 => PictureType::AFishEvenBrighter,
            18 => PictureType::Illustration,
            19 => PictureType::BandLogo,
            20 => PictureType::PublisherLogo,
            _ => PictureType::Other,
        }
    }
}

/// ID3v1 genre list (index → genre name).
pub const GENRES: &[&str] = &[
    "Blues", "Classic Rock", "Country", "Dance", "Disco", "Funk", "Grunge",
    "Hip-Hop", "Jazz", "Metal", "New Age", "Oldies", "Other", "Pop", "R&B",
    "Rap", "Reggae", "Rock", "Techno", "Industrial", "Alternative", "Ska",
    "Death Metal", "Pranks", "Soundtrack", "Euro-Techno", "Ambient",
    "Trip-Hop", "Vocal", "Jazz+Funk", "Fusion", "Trance", "Classical",
    "Instrumental", "Acid", "House", "Game", "Sound Clip", "Gospel", "Noise",
    "AlternRock", "Bass", "Soul", "Punk", "Space", "Meditative",
    "Instrumental Pop", "Instrumental Rock", "Ethnic", "Gothic", "Darkwave",
    "Techno-Industrial", "Electronic", "Pop-Folk", "Eurodance", "Dream",
    "Southern Rock", "Comedy", "Cult", "Gangsta", "Top 40", "Christian Rap",
    "Pop/Funk", "Jungle", "Native American", "Cabaret", "New Wave",
    "Psychedelic", "Rave", "Showtunes", "Trailer", "Lo-Fi", "Tribal",
    "Acid Punk", "Acid Jazz", "Polka", "Retro", "Musical", "Rock & Roll",
    "Hard Rock", "Folk", "Folk-Rock", "National Folk", "Swing", "Fast Fusion",
    "Bebop", "Latin", "Revival", "Celtic", "Bluegrass", "Avantgarde",
    "Gothic Rock", "Progressive Rock", "Psychedelic Rock", "Symphonic Rock",
    "Slow Rock", "Big Band", "Chorus", "Easy Listening", "Acoustic", "Humour",
    "Speech", "Chanson", "Opera", "Chamber Music", "Sonata", "Symphony",
    "Booty Bass", "Primus", "Porn Groove", "Satire", "Slow Jam", "Club",
    "Tango", "Samba", "Folklore", "Ballad", "Power Ballad", "Rhythmic Soul",
    "Freestyle", "Duet", "Punk Rock", "Drum Solo", "A capella", "Euro-House",
    "Dance Hall", "Goa", "Drum & Bass", "Club-House", "Hardcore Techno",
    "Terror", "Indie", "BritPop", "Negerpunk", "Polsk Punk", "Beat",
    "Christian Gangsta Rap", "Heavy Metal", "Black Metal", "Crossover",
    "Contemporary Christian", "Christian Rock", "Merengue", "Salsa",
    "Thrash Metal", "Anime", "Jpop", "Synthpop", "Abstract", "Art Rock",
    "Baroque", "Bhangra", "Big Beat", "Breakbeat", "Chillout", "Downtempo",
    "Dub", "EBM", "Eclectic", "Electro", "Electroclash", "Emo", "Experimental",
    "Garage", "Global", "IDM", "Illbient", "Industro-Goth", "Jam Band",
    "Krautrock", "Leftfield", "Lounge", "Math Rock", "New Romantic",
    "Nu-Breakz", "Post-Punk", "Post-Rock", "Psytrance", "Shoegaze",
    "Space Rock", "Trop Rock", "World Music", "Neoclassical", "Audiobook",
    "Audio Theatre", "Neue Deutsche Welle", "Podcast", "Indie Rock",
    "G-Funk", "Dubstep", "Garage Rock", "Psybient",
];

/// Parse TCON (content type / genre) value.
/// Handles formats like: "Rock", "(17)", "(17)Rock", "17", "(RX)", "(CR)"
pub fn parse_genre(text: &str) -> Vec<String> {
    let mut genres = Vec::new();
    let trimmed = text.trim();

    if trimmed.is_empty() {
        return genres;
    }

    // Try to parse genre references like (17) or just 17
    let mut remaining = trimmed;

    while !remaining.is_empty() {
        if remaining.starts_with('(') {
            // Find matching close paren
            if let Some(close) = remaining.find(')') {
                let inner = &remaining[1..close];
                remaining = &remaining[close + 1..];

                if inner == "RX" {
                    genres.push("Remix".to_string());
                } else if inner == "CR" {
                    genres.push("Cover".to_string());
                } else if let Ok(num) = inner.parse::<usize>() {
                    if num < GENRES.len() {
                        genres.push(GENRES[num].to_string());
                    } else {
                        genres.push(format!("Unknown({})", num));
                    }
                } else {
                    genres.push(inner.to_string());
                }
            } else {
                // No close paren, take the rest as-is
                genres.push(remaining.to_string());
                break;
            }
        } else {
            // No parens - try numeric, otherwise take as text
            if let Ok(num) = remaining.parse::<usize>() {
                if num < GENRES.len() {
                    genres.push(GENRES[num].to_string());
                } else {
                    genres.push(remaining.to_string());
                }
            } else {
                // Check for null-separated genres (ID3v2.4)
                for part in remaining.split('\0') {
                    let part = part.trim();
                    if !part.is_empty() {
                        if let Ok(num) = part.parse::<usize>() {
                            if num < GENRES.len() {
                                genres.push(GENRES[num].to_string());
                            } else {
                                genres.push(part.to_string());
                            }
                        } else {
                            genres.push(part.to_string());
                        }
                    }
                }
            }
            break;
        }
    }

    if genres.is_empty() && !trimmed.is_empty() {
        genres.push(trimmed.to_string());
    }

    genres
}
