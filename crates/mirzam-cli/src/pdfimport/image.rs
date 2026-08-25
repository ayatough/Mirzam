//! The image stored in a page, lifted out whole.
//!
//! When a figure is a photograph, a screenshot or a plot saved as a bitmap, the
//! page does not draw it — it places a picture that is already in the file. So
//! the best possible extraction is no extraction at all: hand over the bytes.
//! A JPEG comes out as the JPEG it was, and anything stored as raw samples is
//! packed into a PNG, which is lossless in both directions.
//!
//! This is the path that needs no tool installed, and it is also the *better*
//! path where it applies: rendering such a figure would resample it, and a
//! screenshot of it would resample it twice.

use flate2::write::ZlibEncoder;
use flate2::Compression;
use lopdf::{Document, Object, ObjectId};
use std::io::Write;

/// An image, ready to be written to disk.
pub struct Extracted {
    pub bytes: Vec<u8>,
    pub extension: &'static str,
    /// What it turned out to be, for the line the command prints.
    pub what: String,
}

/// Lifts one image XObject out of the document.
pub fn extract(doc: &Document, id: ObjectId) -> Result<Extracted, String> {
    let stream = doc
        .get_object(id)
        .and_then(|o| o.as_stream())
        .map_err(|e| format!("image {id:?}: {e}"))?;
    let filters: Vec<&[u8]> = stream.filters().unwrap_or_default();

    // A JPEG in a PDF is a JPEG file with the header taken off and put in the
    // dictionary. Written back out, it is the photograph the author had.
    if filters.last() == Some(&b"DCTDecode".as_slice()) {
        if filters.len() > 1 {
            return Err("a JPEG behind another filter, which is not unwrapped here".to_string());
        }
        return Ok(Extracted {
            bytes: stream.content.clone(),
            extension: "jpg",
            what: "JPEG".to_string(),
        });
    }
    if filters.last() == Some(&b"JPXDecode".as_slice()) {
        return Err("JPEG 2000, which no browser but Safari shows".to_string());
    }
    if filters
        .iter()
        .any(|f| matches!(*f, b"CCITTFaxDecode" | b"JBIG2Decode"))
    {
        return Err("a fax-coded scan, which is not decoded here".to_string());
    }

    let samples = stream
        .decompressed_content()
        .map_err(|e| format!("image {id:?}: {e}"))?;
    let dict = &stream.dict;
    let width = int(doc, dict.get(b"Width").ok()).ok_or("an image with no width")? as usize;
    let height = int(doc, dict.get(b"Height").ok()).ok_or("an image with no height")? as usize;
    let bits = int(doc, dict.get(b"BitsPerComponent").ok()).unwrap_or(8) as usize;
    let mask = dict
        .get(b"ImageMask")
        .ok()
        .and_then(|o| o.as_bool().ok())
        .unwrap_or(false);

    let space = if mask {
        Space::Gray
    } else {
        Space::read(doc, dict.get(b"ColorSpace").ok()).ok_or("a colour space this cannot read")?
    };
    if width == 0 || height == 0 || !matches!(bits, 1 | 2 | 4 | 8) {
        return Err(format!(
            "{width}×{height} at {bits} bits, which is not read here"
        ));
    }

    // `/Decode [1 0]` on a mask means the sense of the bits is reversed, which
    // is how a stencil is normally written.
    let inverted = dict
        .get(b"Decode")
        .ok()
        .and_then(|o| o.as_array().ok())
        .and_then(|a| a.first().and_then(|o| o.as_float().ok()))
        .is_some_and(|first| first > 0.5);

    let mut rgb = to_rgb(&samples, width, height, bits, &space, inverted)?;
    let mut colour = Colour::Rgb;
    let mut what = format!("{width}×{height} image");

    if let Some(alpha) = soft_mask(doc, dict, width, height) {
        rgb = rgb
            .chunks(3)
            .zip(alpha)
            .flat_map(|(pixel, a)| [pixel[0], pixel[1], pixel[2], a])
            .collect();
        colour = Colour::Rgba;
        what.push_str(" with its transparency");
    }

    Ok(Extracted {
        bytes: png(width, height, colour, &rgb)?,
        extension: "png",
        what,
    })
}

/// How the samples say what colour a pixel is.
enum Space {
    Gray,
    Rgb,
    /// A palette: each sample is an index into `lookup`, which holds RGB
    /// triples.
    Indexed(Vec<u8>),
}

impl Space {
    fn read(doc: &Document, object: Option<&Object>) -> Option<Space> {
        let object = deref(doc, object?)?;
        if let Ok(name) = object.as_name() {
            return match name {
                b"DeviceGray" | b"CalGray" | b"G" => Some(Space::Gray),
                b"DeviceRGB" | b"CalRGB" | b"RGB" => Some(Space::Rgb),
                _ => None,
            };
        }
        let array = object.as_array().ok()?;
        match array.first().and_then(|o| o.as_name().ok())? {
            b"ICCBased" => match int(
                doc,
                deref(doc, array.get(1)?)?
                    .as_stream()
                    .ok()?
                    .dict
                    .get(b"N")
                    .ok(),
            )? {
                1 => Some(Space::Gray),
                3 => Some(Space::Rgb),
                _ => None,
            },
            b"CalRGB" => Some(Space::Rgb),
            b"CalGray" => Some(Space::Gray),
            b"Indexed" | b"I" => {
                let base = Space::read(doc, array.get(1))?;
                let lookup = match deref(doc, array.get(3)?)? {
                    Object::String(bytes, _) => bytes.clone(),
                    Object::Stream(stream) => stream.decompressed_content().ok()?,
                    _ => return None,
                };
                // The palette is stored in the base space; widen a grey one so
                // every palette is three bytes per entry from here on.
                let lookup = match base {
                    Space::Gray => lookup.iter().flat_map(|&g| [g, g, g]).collect(),
                    Space::Rgb => lookup,
                    Space::Indexed(_) => return None,
                };
                Some(Space::Indexed(lookup))
            }
            _ => None,
        }
    }

    fn components(&self) -> usize {
        match self {
            Space::Gray | Space::Indexed(_) => 1,
            Space::Rgb => 3,
        }
    }
}

/// Samples in, three bytes a pixel out.
fn to_rgb(
    samples: &[u8],
    width: usize,
    height: usize,
    bits: usize,
    space: &Space,
    inverted: bool,
) -> Result<Vec<u8>, String> {
    let components = space.components();
    // Every row starts on a byte boundary, which matters at fewer than eight
    // bits: a 100-pixel row of 1-bit samples is 13 bytes, not 12.5.
    let row_bytes = (width * components * bits).div_ceil(8);
    if samples.len() < row_bytes * height {
        return Err(format!(
            "{} bytes of samples for a {width}×{height} image that needs {}",
            samples.len(),
            row_bytes * height
        ));
    }
    let max = ((1u32 << bits) - 1) as f64;

    let mut out = Vec::with_capacity(width * height * 3);
    for y in 0..height {
        let row = &samples[y * row_bytes..];
        for x in 0..width {
            let mut pixel = [0u8; 3];
            for (c, slot) in pixel.iter_mut().take(components).enumerate() {
                let at = x * components + c;
                let raw = read_bits(row, at, bits);
                let value = match space {
                    Space::Indexed(_) => raw,
                    _ => (raw as f64 / max * 255.0).round() as u32,
                };
                let value = if inverted && !matches!(space, Space::Indexed(_)) {
                    255 - value.min(255)
                } else {
                    value
                };
                *slot = value.min(255) as u8;
            }
            match space {
                Space::Gray => out.extend_from_slice(&[pixel[0], pixel[0], pixel[0]]),
                Space::Rgb => out.extend_from_slice(&pixel),
                Space::Indexed(lookup) => {
                    let at = pixel[0] as usize * 3;
                    let entry = lookup.get(at..at + 3).unwrap_or(&[0, 0, 0]);
                    out.extend_from_slice(entry);
                }
            }
        }
    }
    Ok(out)
}

/// One sample, however many bits wide it is.
fn read_bits(row: &[u8], index: usize, bits: usize) -> u32 {
    match bits {
        8 => row.get(index).copied().unwrap_or(0) as u32,
        _ => {
            let bit = index * bits;
            let byte = row.get(bit / 8).copied().unwrap_or(0) as u32;
            let shift = 8 - bits - (bit % 8);
            (byte >> shift) & ((1 << bits) - 1)
        }
    }
}

/// The `/SMask`'s samples as one alpha byte per pixel, when it is one this can
/// read and it matches the image pixel for pixel.
///
/// A mismatch in size is legal and means the alpha is scaled; rather than
/// resample it, the transparency is dropped — a figure with hard edges looks
/// wrong resampled, and a white background is right far more often than a
/// stretched mask.
fn soft_mask(
    doc: &Document,
    dict: &lopdf::Dictionary,
    width: usize,
    height: usize,
) -> Option<Vec<u8>> {
    let stream = deref(doc, dict.get(b"SMask").ok()?)?.as_stream().ok()?;
    let bits = int(doc, stream.dict.get(b"BitsPerComponent").ok())? as usize;
    let mask_width = int(doc, stream.dict.get(b"Width").ok())? as usize;
    let mask_height = int(doc, stream.dict.get(b"Height").ok())? as usize;
    if (mask_width, mask_height) != (width, height) {
        return None;
    }
    let samples = stream.decompressed_content().ok()?;
    let gray = to_rgb(&samples, width, height, bits, &Space::Gray, false).ok()?;
    Some(gray.chunks(3).map(|p| p[0]).collect())
}

enum Colour {
    Rgb,
    Rgba,
}

/// A PNG, written by hand: a signature, three chunks and a CRC each.
///
/// A dependency for this would be a large one — the crates that encode PNG
/// decode forty other formats on the way — and what is needed here is the
/// simplest file the format allows: eight bits a channel, no interlacing, and
/// every scanline left unfiltered so the only compression is the deflate that
/// `flate2`, already here to read the PDF, performs.
fn png(width: usize, height: usize, colour: Colour, pixels: &[u8]) -> Result<Vec<u8>, String> {
    let (colour_type, channels) = match colour {
        Colour::Rgb => (2u8, 3usize),
        Colour::Rgba => (6, 4),
    };
    let mut raw = Vec::with_capacity(height * (1 + width * channels));
    for y in 0..height {
        raw.push(0);
        let from = y * width * channels;
        raw.extend_from_slice(
            pixels
                .get(from..from + width * channels)
                .ok_or("an image shorter than its own size")?,
        );
    }
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(&raw)
        .and_then(|()| encoder.flush())
        .map_err(|e| format!("cannot compress the image: {e}"))?;
    let data = encoder
        .finish()
        .map_err(|e| format!("cannot compress the image: {e}"))?;

    let mut header = Vec::new();
    header.extend_from_slice(&(width as u32).to_be_bytes());
    header.extend_from_slice(&(height as u32).to_be_bytes());
    header.extend_from_slice(&[8, colour_type, 0, 0, 0]);

    let mut png = vec![0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    chunk(&mut png, b"IHDR", &header);
    chunk(&mut png, b"IDAT", &data);
    chunk(&mut png, b"IEND", &[]);
    Ok(png)
}

fn chunk(png: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    png.extend_from_slice(&(data.len() as u32).to_be_bytes());
    png.extend_from_slice(kind);
    png.extend_from_slice(data);
    let mut crc = flate2::Crc::new();
    crc.update(kind);
    crc.update(data);
    png.extend_from_slice(&crc.sum().to_be_bytes());
}

fn deref<'a>(doc: &'a Document, object: &'a Object) -> Option<&'a Object> {
    let mut object = object;
    for _ in 0..32 {
        match object {
            Object::Reference(id) => object = doc.get_object(*id).ok()?,
            other => return Some(other),
        }
    }
    None
}

fn int(doc: &Document, object: Option<&Object>) -> Option<i64> {
    deref(doc, object?)?.as_i64().ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_png_carries_its_own_size_and_checksums() {
        let pixels = vec![255u8, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
        let png = png(2, 2, Colour::Rgb, &pixels).unwrap();
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        assert_eq!(&png[12..16], b"IHDR");
        assert_eq!(&png[16..20], &2u32.to_be_bytes());
        assert_eq!(&png[20..24], &2u32.to_be_bytes());
        assert_eq!(png[24], 8, "eight bits a channel");
        assert_eq!(png[25], 2, "true colour");
        assert!(
            png.ends_with(&[0xae, 0x42, 0x60, 0x82]),
            "the IEND checksum"
        );
    }

    /// Four grey pixels at one bit each, in a row that is padded to a byte.
    #[test]
    fn a_bilevel_row_is_read_bit_by_bit() {
        let rgb = to_rgb(&[0b1010_0000], 4, 1, 1, &Space::Gray, false).unwrap();
        assert_eq!(rgb, vec![255, 255, 255, 0, 0, 0, 255, 255, 255, 0, 0, 0]);
    }

    #[test]
    fn a_palette_becomes_the_colour_it_points_at() {
        let space = Space::Indexed(vec![10, 20, 30, 40, 50, 60]);
        // Two four-bit samples in one byte: entry 0, then entry 1.
        let rgb = to_rgb(&[0b0000_0001], 2, 1, 4, &space, false).unwrap();
        assert_eq!(rgb, vec![10, 20, 30, 40, 50, 60]);
    }

    /// A sample pointing past the end of the palette is a broken file, and
    /// black is the one answer that cannot panic or leak the next entry.
    #[test]
    fn an_index_past_the_palette_is_black() {
        let space = Space::Indexed(vec![10, 20, 30]);
        let rgb = to_rgb(&[0b0100_0000], 1, 1, 4, &space, false).unwrap();
        assert_eq!(rgb, vec![0, 0, 0]);
    }
}
