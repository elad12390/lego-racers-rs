//! True-color Targa resources shipped alongside original MDB textures.
use crate::bmp::Image;
pub fn decode(bytes: &[u8]) -> Result<Image, String> {
  let header = bytes.get(..18).ok_or("truncated TGA header")?;
  let kind = header[2];
  if header[1] != 0 || !matches!(kind, 2 | 10) {
    return Err("unsupported TGA image type".into());
  }
  let width = u16::from_le_bytes([header[12], header[13]]);
  let height = u16::from_le_bytes([header[14], header[15]]);
  let depth = header[16];
  if width == 0 || height == 0 || !matches!(depth, 24 | 32) || header[17] & 0xc0 != 0 {
    return Err("unsupported TGA dimensions/depth/interleaving".into());
  }
  let channels = usize::from(depth / 8);
  let count = usize::from(width) * usize::from(height);
  let mut at = 18 + usize::from(header[0]);
  let mut pixels = Vec::with_capacity(count);
  let pixel = |at: &mut usize| -> Result<[u8; 4], String> {
    let p = bytes
      .get(*at..*at + channels)
      .ok_or("truncated TGA pixels")?;
    *at += channels;
    Ok([
      p[2],
      p[1],
      p[0],
      if channels == 4 && header[17] & 15 != 0 {
        p[3]
      } else {
        255
      },
    ])
  };
  while pixels.len() < count {
    if kind == 2 {
      pixels.push(pixel(&mut at)?);
      continue;
    }
    let packet = *bytes.get(at).ok_or("truncated TGA RLE packet")?;
    at += 1;
    let run = usize::from(packet & 127) + 1;
    if run > count - pixels.len() {
      return Err("TGA RLE overrun".into());
    }
    if packet & 128 != 0 {
      let value = pixel(&mut at)?;
      pixels.extend(std::iter::repeat_n(value, run));
    } else {
      for _ in 0..run {
        pixels.push(pixel(&mut at)?);
      }
    }
  }
  // Image upload expects row-major top-left origin; both TGA origin bits
  // must be respected, including alpha on the Rocket Racer sparkle atlas.
  let mut rgba = vec![0; count * 4];
  for (i, pixel) in pixels.into_iter().enumerate() {
    let x = i % usize::from(width);
    let y = i / usize::from(width);
    let x = if header[17] & 16 != 0 {
      usize::from(width) - 1 - x
    } else {
      x
    };
    let y = if header[17] & 32 == 0 {
      usize::from(height) - 1 - y
    } else {
      y
    };
    rgba[(y * usize::from(width) + x) * 4..(y * usize::from(width) + x + 1) * 4]
      .copy_from_slice(&pixel);
  }
  Ok(Image {
    width,
    height,
    palette: Vec::new(),
    indices: Vec::new(),
    direct_rgba: Some(rgba),
  })
}
#[cfg(test)]
mod tests {
  use super::*;
  #[test]
  fn alpha_origin_and_rle_bounds() {
    let mut bytes = vec![0; 18];
    bytes[2] = 2;
    bytes[12] = 2;
    bytes[14] = 1;
    bytes[16] = 32;
    bytes[17] = 0x38;
    bytes.extend_from_slice(&[3, 2, 1, 4, 7, 6, 5, 8]);
    assert_eq!(decode(&bytes).unwrap().to_rgba(), [5, 6, 7, 8, 1, 2, 3, 4]);
    bytes.truncate(18);
    bytes[2] = 10;
    bytes.extend_from_slice(&[0x81, 3, 2, 1, 4]);
    assert_eq!(decode(&bytes).unwrap().to_rgba(), [1, 2, 3, 4, 1, 2, 3, 4]);
    bytes[18] = 0x82;
    assert!(decode(&bytes).is_err());
  }
}
