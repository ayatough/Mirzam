//! A minimal ZIP writer, enough for a `.pptx` package and for
//! `mirzam skill install --zip`.
//!
//! Written here rather than pulled in as a dependency because what this needs
//! is the smallest corner of the format: a handful of small text files, stored
//! uncompressed, in one archive that claude.ai's skill upload accepts. That is
//! two headers and a trailer per file, and no compressor at all - a dependency
//! would cost more to audit than the sixty lines it replaced.
//!
//! Deliberately deterministic: every entry carries the same fixed timestamp
//! (1980-01-01, the earliest a DOS date can express), so the same binary and
//! the same card always produce byte-identical archives. A release that
//! rebuilds the zip should not publish a "changed" file that differs only in
//! the minute it was written.

/// One stored file's local header, name and bytes.
struct Entry {
    name: String,
    crc: u32,
    size: u32,
    /// Offset of this entry's local header from the start of the archive.
    offset: u32,
}

/// The earliest date a DOS timestamp can hold: 1980-01-01, 00:00.
const DOS_DATE: u16 = 0x0021;
const DOS_TIME: u16 = 0;
/// `0o100644` in the high half, which is where a Unix mode lives in a central
/// directory record written by a Unix zip.
const EXTERNAL_ATTRS: u32 = 0o100_644 << 16;

/// Builds a ZIP archive holding `files` as `(path inside the archive, contents)`,
/// each stored uncompressed.
pub fn archive(files: &[(String, String)]) -> Vec<u8> {
    let bytes: Vec<(String, Vec<u8>)> = files
        .iter()
        .map(|(n, b)| (n.clone(), b.as_bytes().to_vec()))
        .collect();
    archive_bytes(&bytes)
}

/// The same archive over binary contents — what a `.pptx` needs, whose media
/// entries are PNGs. Still stored uncompressed: the images arrive compressed
/// already, and the XML parts beside them are small.
pub fn archive_bytes(files: &[(String, Vec<u8>)]) -> Vec<u8> {
    let mut out: Vec<u8> = Vec::new();
    let mut entries: Vec<Entry> = Vec::with_capacity(files.len());

    for (name, body) in files {
        let bytes: &[u8] = body;
        let entry = Entry {
            name: name.clone(),
            crc: crc32(bytes),
            size: bytes.len() as u32,
            offset: out.len() as u32,
        };
        // Local file header.
        out.extend_from_slice(&0x0403_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&0u16.to_le_bytes()); // method: stored
        out.extend_from_slice(&DOS_TIME.to_le_bytes());
        out.extend_from_slice(&DOS_DATE.to_le_bytes());
        out.extend_from_slice(&entry.crc.to_le_bytes());
        out.extend_from_slice(&entry.size.to_le_bytes()); // compressed
        out.extend_from_slice(&entry.size.to_le_bytes()); // uncompressed
        out.extend_from_slice(&(entry.name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra field length
        out.extend_from_slice(entry.name.as_bytes());
        out.extend_from_slice(bytes);
        entries.push(entry);
    }

    let cd_start = out.len() as u32;
    for e in &entries {
        out.extend_from_slice(&0x0201_4b50u32.to_le_bytes());
        out.extend_from_slice(&20u16.to_le_bytes()); // version made by
        out.extend_from_slice(&20u16.to_le_bytes()); // version needed
        out.extend_from_slice(&0u16.to_le_bytes()); // flags
        out.extend_from_slice(&0u16.to_le_bytes()); // method: stored
        out.extend_from_slice(&DOS_TIME.to_le_bytes());
        out.extend_from_slice(&DOS_DATE.to_le_bytes());
        out.extend_from_slice(&e.crc.to_le_bytes());
        out.extend_from_slice(&e.size.to_le_bytes());
        out.extend_from_slice(&e.size.to_le_bytes());
        out.extend_from_slice(&(e.name.len() as u16).to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes()); // extra
        out.extend_from_slice(&0u16.to_le_bytes()); // comment
        out.extend_from_slice(&0u16.to_le_bytes()); // disk number
        out.extend_from_slice(&0u16.to_le_bytes()); // internal attrs
        out.extend_from_slice(&EXTERNAL_ATTRS.to_le_bytes());
        out.extend_from_slice(&e.offset.to_le_bytes());
        out.extend_from_slice(e.name.as_bytes());
    }
    let cd_size = out.len() as u32 - cd_start;

    // End of central directory.
    out.extend_from_slice(&0x0605_4b50u32.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // this disk
    out.extend_from_slice(&0u16.to_le_bytes()); // disk with the directory
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
    out.extend_from_slice(&cd_size.to_le_bytes());
    out.extend_from_slice(&cd_start.to_le_bytes());
    out.extend_from_slice(&0u16.to_le_bytes()); // archive comment
    out
}

/// CRC-32 (IEEE), computed bit by bit. A table would be faster and this runs
/// over a few kilobytes once per install, so it is not worth one.
fn crc32(bytes: &[u8]) -> u32 {
    let mut crc: u32 = 0xffff_ffff;
    for b in bytes {
        crc ^= u32::from(*b);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xedb8_8320 & mask);
        }
    }
    !crc
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The standard check value for CRC-32/ISO-HDLC.
    #[test]
    fn crc32_matches_the_published_check_value() {
        assert_eq!(crc32(b"123456789"), 0xcbf4_3926);
        assert_eq!(crc32(b""), 0);
    }

    #[test]
    fn an_archive_carries_its_signatures_and_names() {
        let zip = archive(&[("s/SKILL.md".to_string(), "hello".to_string())]);
        assert_eq!(&zip[..4], b"PK\x03\x04", "local file header first");
        assert!(
            zip.windows(4).any(|w| w == b"PK\x01\x02"),
            "a central directory record"
        );
        assert!(
            zip.windows(4).any(|w| w == b"PK\x05\x06"),
            "an end-of-central-directory record"
        );
        let text = String::from_utf8_lossy(&zip);
        assert!(text.contains("s/SKILL.md"), "the entry is named");
        assert!(text.contains("hello"), "stored, so the bytes are readable");
    }

    /// Byte-identical output for identical input: no timestamp of the moment
    /// leaks in, so rebuilding the archive does not "change" it.
    #[test]
    fn the_same_input_produces_the_same_bytes() {
        let files = [("a/SKILL.md".to_string(), "x".to_string())];
        assert_eq!(archive(&files), archive(&files));
    }
}
