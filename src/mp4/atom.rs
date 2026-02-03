use crate::common::error::{MutagenError, Result};

/// Container atom names that have children.
const CONTAINER_ATOMS: &[&[u8; 4]] = &[
    b"moov", b"udta", b"trak", b"mdia", b"minf", b"stbl",
    b"meta", b"ilst", b"moof", b"traf", b"edts", b"dinf",
];

/// An MP4 atom (box).
#[derive(Debug, Clone)]
pub struct Atom {
    pub name: [u8; 4],
    pub offset: usize,      // Position of atom start in file
    pub size: usize,         // Total atom size including header
    pub data_offset: usize,  // Start of data (after header)
    pub data_size: usize,    // Size of data
    pub header_size: u8,     // 8 or 16 (extended size)
}

impl Atom {
    pub fn name_str(&self) -> String {
        String::from_utf8_lossy(&self.name).to_string()
    }
}

/// Parse atoms from data within a range.
pub fn parse_atoms(data: &[u8], start: usize, end: usize) -> Result<Vec<Atom>> {
    let mut atoms = Vec::new();
    let mut pos = start;

    while pos + 8 <= end && pos + 8 <= data.len() {
        let size = u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        let name: [u8; 4] = [data[pos + 4], data[pos + 5], data[pos + 6], data[pos + 7]];

        let (atom_size, header_size) = if size == 1 {
            // Extended 64-bit size
            if pos + 16 > end || pos + 16 > data.len() {
                break;
            }
            let ext_size = u64::from_be_bytes([
                data[pos + 8], data[pos + 9], data[pos + 10], data[pos + 11],
                data[pos + 12], data[pos + 13], data[pos + 14], data[pos + 15],
            ]) as usize;
            (ext_size, 16u8)
        } else if size == 0 {
            // Atom extends to end of file
            (end - pos, 8u8)
        } else {
            (size, 8u8)
        };

        if atom_size < header_size as usize {
            break;
        }

        let data_offset = pos + header_size as usize;
        let data_size = atom_size - header_size as usize;

        // Clamp to available data
        let data_size = data_size.min(end.saturating_sub(data_offset)).min(data.len().saturating_sub(data_offset));

        atoms.push(Atom {
            name,
            offset: pos,
            size: atom_size,
            data_offset,
            data_size,
            header_size,
        });

        pos += atom_size;
        if pos <= start {
            break; // Prevent infinite loop
        }
    }

    Ok(atoms)
}

/// Find an atom by navigating a path like ["moov", "trak", "mdia"].
pub fn find_atom_path<'a>(data: &[u8], atoms: &'a [Atom], path: &[&[u8; 4]]) -> Option<Atom> {
    if path.is_empty() {
        return None;
    }

    let target = path[0];
    let found = atoms.iter().find(|a| &a.name == target)?;

    if path.len() == 1 {
        return Some(found.clone());
    }

    // Parse children and recurse
    let children = parse_atoms(data, found.data_offset, found.data_offset + found.data_size).ok()?;
    find_atom_path(data, &children, &path[1..])
}
