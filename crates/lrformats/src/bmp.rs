use std::fmt;

use crate::lz::{self, LzError};

const HEADER_LEN: usize = 6;
const NO_PALETTE_FLAG: u8 = 0x80;
const DEPTH_MASK: u8 = 0x3c;

#[derive(Debug, PartialEq, Eq)]
pub enum BmpError {
  Truncated,
  UnsupportedDepth(u8),
  Block(LzError),
}

impl fmt::Display for BmpError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
      BmpError::Truncated => write!(f, "image data ends early"),
      BmpError::UnsupportedDepth(depth) => write!(f, "unsupported color depth {depth}"),
      BmpError::Block(error) => write!(f, "bad compressed block: {error}"),
    }
  }
}

impl std::error::Error for BmpError {}

impl From<LzError> for BmpError {
  fn from(error: LzError) -> Self {
    BmpError::Block(error)
  }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
  pub width: u16,
  pub height: u16,
  pub palette: Vec<[u8; 3]>,
  /// One palette index per pixel, row by row.
  pub indices: Vec<u8>,
  /// Decoded RGB(A) pixels for original non-palettized 24/32-bit files.
  pub direct_rgba: Option<Vec<u8>>,
}

impl Image {
  pub fn to_rgba(&self) -> Vec<u8> {
    if let Some(rgba) = &self.direct_rgba {
      return rgba.clone();
    }
    let mut out = Vec::with_capacity(self.indices.len() * 4);
    for &index in &self.indices {
      let [r, g, b] = self
        .palette
        .get(usize::from(index))
        .copied()
        .unwrap_or([0, 0, 0]);
      out.extend_from_slice(&[r, g, b, 0xff]);
    }
    out
  }
  pub fn rgb(&self, index: usize) -> [u8; 3] {
    if let Some(rgba) = &self.direct_rgba {
      return rgba
        .get(index * 4..index * 4 + 3)
        .and_then(|v| v.try_into().ok())
        .unwrap_or([255, 0, 255]);
    }
    self
      .indices
      .get(index)
      .and_then(|v| self.palette.get(*v as usize))
      .copied()
      .unwrap_or([255, 0, 255])
  }
}

/// Decodes the game's palettized image files (the ones stored as `.BMP` in LEGO.JAM).
///
/// Layout: a depth byte (4 or 8 in bits 2..5), a palette-size byte (entries - 1), 16-bit
/// width and height, the BGR palette, then blocks of `[raw length u16][stored length u16]`
/// followed by the block bytes, compressed with [`lz::decompress`] whenever the stored
/// length is smaller than the raw length.
pub fn decode(file: &[u8]) -> Result<Image, BmpError> {
  if file.len() < HEADER_LEN {
    return Err(BmpError::Truncated);
  }
  let depth = file[0] & DEPTH_MASK;
  if matches!(depth, 24 | 32) {
    let width = u16::from_le_bytes([file[2], file[3]]);
    let height = u16::from_le_bytes([file[4], file[5]]);
    let bytes_per_pixel = usize::from(depth) / 8;
    let row_bytes = (usize::from(width) * bytes_per_pixel).div_ceil(4) * 4;
    let packed = read_blocks(&file[HEADER_LEN..], row_bytes * usize::from(height))?;
    let mut rgba = Vec::with_capacity(usize::from(width) * usize::from(height) * 4);
    for row in packed.chunks_exact(row_bytes.max(1)).take(height as usize) {
      for pixel in row[..width as usize * bytes_per_pixel].chunks_exact(bytes_per_pixel) {
        rgba.extend_from_slice(&[
          pixel[2],
          pixel[1],
          pixel[0],
          if depth == 32 { pixel[3] } else { 255 },
        ]);
      }
    }
    return Ok(Image {
      width,
      height,
      palette: Vec::new(),
      indices: Vec::new(),
      direct_rgba: Some(rgba),
    });
  }
  if depth != 4 && depth != 8 || file[0] & NO_PALETTE_FLAG != 0 {
    return Err(BmpError::UnsupportedDepth(depth));
  }
  let width = u16::from_le_bytes([file[2], file[3]]);
  let height = u16::from_le_bytes([file[4], file[5]]);
  let colors = usize::from(file[1]) + 1;
  let data_start = HEADER_LEN + colors * 3;
  let palette: Vec<[u8; 3]> = file
    .get(HEADER_LEN..data_start)
    .ok_or(BmpError::Truncated)?
    .chunks_exact(3)
    // Original BmpImage::ReadHeader004017d0 stores byte2 at R, byte1
    // at G and byte0 at B. Keep Image.palette in renderer RGB order.
    .map(|bgr| [bgr[2], bgr[1], bgr[0]])
    .collect();

  let row_bytes = if depth == 4 {
    usize::from(width).div_ceil(2)
  } else {
    usize::from(width)
  };
  let wanted = row_bytes * usize::from(height);
  let packed = read_blocks(&file[data_start..], wanted)?;

  let mut indices = Vec::with_capacity(usize::from(width) * usize::from(height));
  for row in packed
    .chunks_exact(row_bytes.max(1))
    .take(usize::from(height))
  {
    if depth == 4 {
      for x in 0..usize::from(width) {
        let byte = row[x / 2];
        indices.push(if x % 2 == 0 { byte >> 4 } else { byte & 0x0f });
      }
    } else {
      indices.extend_from_slice(row);
    }
  }
  Ok(Image {
    width,
    height,
    palette,
    indices,
    direct_rgba: None,
  })
}

fn read_blocks(mut data: &[u8], wanted: usize) -> Result<Vec<u8>, BmpError> {
  let mut out = Vec::with_capacity(wanted);
  while out.len() < wanted {
    let header = data.get(..4).ok_or(BmpError::Truncated)?;
    let raw_len = usize::from(u16::from_le_bytes([header[0], header[1]]));
    let stored_len = usize::from(u16::from_le_bytes([header[2], header[3]]));
    let body = data.get(4..4 + stored_len).ok_or(BmpError::Truncated)?;
    if stored_len < raw_len {
      out.extend_from_slice(&lz::decompress(body)?);
    } else {
      out.extend_from_slice(&body[..raw_len.min(body.len())]);
    }
    data = &data[4 + stored_len..];
  }
  if out.len() < wanted {
    return Err(BmpError::Truncated);
  }
  Ok(out)
}

#[cfg(test)]
mod tests {
  use super::*;

  fn tiny_image(depth: u8, pixels: &[u8]) -> Vec<u8> {
    let mut file = vec![depth, 1, 2, 0, 2, 0, 10, 20, 30, 200, 210, 220];
    file.extend_from_slice(&(pixels.len() as u16).to_le_bytes());
    file.extend_from_slice(&(pixels.len() as u16).to_le_bytes());
    file.extend_from_slice(pixels);
    file
  }

  #[test]
  fn decodes_a_stored_4bit_image_high_nibble_first() {
    // 2x2 at 4 bits: one byte per row.
    let image = decode(&tiny_image(4, &[0x01, 0x10])).unwrap();
    assert_eq!((image.width, image.height), (2, 2));
    assert_eq!(image.indices, vec![0, 1, 1, 0]);
    assert_eq!(image.palette, vec![[30, 20, 10], [220, 210, 200]]);
    assert_eq!(
      &image.to_rgba()[..8],
      &[30, 20, 10, 255, 220, 210, 200, 255]
    );
  }

  #[test]
  fn decodes_a_stored_8bit_image() {
    let image = decode(&tiny_image(8, &[0, 1, 1, 0])).unwrap();
    assert_eq!(image.indices, vec![0, 1, 1, 0]);
  }

  #[test]
  fn decodes_a_compressed_block() {
    // 4x4 at 8 bits: a literal plus one 15-byte run expands 6 stored bytes to 16 raw bytes.
    let mut file = vec![8, 1, 4, 0, 4, 0, 0, 0, 0, 255, 255, 255];
    file.extend_from_slice(&[16, 0, 6, 0]);
    file.extend_from_slice(&[0x01, 0xc0, 0x03, 0x01, 0x0f, 0x00]);
    let image = decode(&file).unwrap();
    assert_eq!(image.indices, vec![1; 16]);
  }

  #[test]
  fn rejects_bad_headers() {
    assert_eq!(decode(&[0; 3]), Err(BmpError::Truncated));
    assert_eq!(
      decode(&tiny_image(16, &[0; 4])),
      Err(BmpError::UnsupportedDepth(16))
    );
  }
}
