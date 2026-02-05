pub mod header;
pub mod unsynch;
pub mod specs;
pub mod frames;
pub mod tags;
pub mod id3v1;
pub mod writer;

use std::fs::File;
use std::io::{Read, Write, Seek, SeekFrom};
use crate::common::error::{MutagenError, Result};
use crate::id3::header::ID3Header;
use crate::id3::tags::ID3Tags;

/// Load ID3v2 tags from a file path using direct read (faster than mmap for small data).
pub fn load_id3(path: &str) -> Result<(ID3Tags, Option<ID3Header>)> {
    let mut file = File::open(path)?;

    // Read just the first 10 bytes to check for ID3 header
    let mut header_buf = [0u8; 10];
    let n = file.read(&mut header_buf)?;
    if n < 10 {
        // Very small file, read it all
        let mut data = header_buf[..n].to_vec();
        file.read_to_end(&mut data)?;
        return load_id3_from_data(&data);
    }

    match ID3Header::parse(&header_buf, 0) {
        Ok(h) => {
            // Read just the tag data (not the entire file!)
            let tag_size = h.size as usize;
            let mut tag_data = vec![0u8; tag_size];
            file.read_exact(&mut tag_data)?;

            let mut tags = ID3Tags::new();

            // Apply whole-tag unsynchronisation (ID3v2.3 and earlier)
            if h.flags.unsynchronisation && h.version.0 < 4 {
                tag_data = unsynch::decode(&tag_data)?;
            }

            tags.read_frames(&tag_data, &h)?;

            // Check for ID3v1 at end - read only last 128 bytes
            let file_len = file.metadata()?.len();
            if file_len >= 128 {
                file.seek(SeekFrom::Start(file_len - 128))?;
                let mut v1_buf = [0u8; 128];
                if file.read_exact(&mut v1_buf).is_ok() && &v1_buf[0..3] == b"TAG" {
                    let v1_frames = id3v1::parse_id3v1(&v1_buf)?;
                    for frame in v1_frames {
                        let key = frame.hash_key();
                        if !tags.contains_key(&key) {
                            tags.add(frame);
                        }
                    }
                }
            }

            Ok((tags, Some(h)))
        }
        Err(MutagenError::ID3NoHeader) => {
            // No ID3v2 - check for ID3v1
            let mut tags = ID3Tags::new();
            let file_len = file.metadata()?.len();
            if file_len >= 128 {
                file.seek(SeekFrom::Start(file_len - 128))?;
                let mut v1_buf = [0u8; 128];
                if file.read_exact(&mut v1_buf).is_ok() && &v1_buf[0..3] == b"TAG" {
                    let v1_frames = id3v1::parse_id3v1(&v1_buf)?;
                    for frame in v1_frames {
                        tags.add(frame);
                    }
                }
            }
            Ok((tags, None))
        }
        Err(e) => Err(e),
    }
}

/// Load ID3v2 tags from a byte slice (used when data is already in memory).
pub fn load_id3_from_data(data: &[u8]) -> Result<(ID3Tags, Option<ID3Header>)> {
    let mut tags = ID3Tags::new();

    let header = match ID3Header::parse(data, 0) {
        Ok(h) => h,
        Err(MutagenError::ID3NoHeader) => {
            if let Some(_offset) = id3v1::find_id3v1(data) {
                let v1_frames = id3v1::parse_id3v1(data)?;
                for frame in v1_frames {
                    tags.add(frame);
                }
            }
            return Ok((tags, None));
        }
        Err(e) => return Err(e),
    };

    let tag_start = 10;
    let tag_end = (10 + header.size as usize).min(data.len());
    let mut tag_data = data[tag_start..tag_end].to_vec();

    if header.flags.unsynchronisation && header.version.0 < 4 {
        tag_data = unsynch::decode(&tag_data)?;
    }

    tags.read_frames(&tag_data, &header)?;

    if let Some(_offset) = id3v1::find_id3v1(data) {
        let v1_frames = id3v1::parse_id3v1(data)?;
        for frame in v1_frames {
            let key = frame.hash_key();
            if !tags.contains_key(&key) {
                tags.add(frame);
            }
        }
    }

    Ok((tags, Some(header)))
}

/// Save ID3v2 tags to a file.
pub fn save_id3(path: &str, tags: &ID3Tags, v2_version: u8) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?;

    let mut existing = Vec::new();
    file.read_to_end(&mut existing)?;

    let old_tag_size = match ID3Header::parse(&existing, 0) {
        Ok(h) => h.full_size() as usize,
        Err(_) => 0,
    };

    let new_tag = writer::render_tag(tags, v2_version)?;

    let audio_start = old_tag_size;
    let audio_data = &existing[audio_start..];

    file.seek(SeekFrom::Start(0))?;
    file.set_len(0)?;
    file.write_all(&new_tag)?;
    file.write_all(audio_data)?;
    file.flush()?;

    Ok(())
}

/// Delete ID3v2 tags from a file.
pub fn delete_id3(path: &str) -> Result<()> {
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?;

    let mut existing = Vec::new();
    file.read_to_end(&mut existing)?;

    let old_tag_size = match ID3Header::parse(&existing, 0) {
        Ok(h) => h.full_size() as usize,
        Err(_) => return Ok(()),
    };

    let audio_data = existing[old_tag_size..].to_vec();

    file.seek(SeekFrom::Start(0))?;
    file.set_len(0)?;
    file.write_all(&audio_data)?;
    file.flush()?;

    let file_len = file.metadata()?.len();
    if file_len >= 128 {
        file.seek(SeekFrom::Start(file_len - 128))?;
        let mut tag_check = [0u8; 3];
        file.read_exact(&mut tag_check)?;
        if &tag_check == b"TAG" {
            file.set_len(file_len - 128)?;
        }
    }

    Ok(())
}
