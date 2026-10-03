use std::fmt;

#[derive(Debug, PartialEq, Eq)]
pub enum LzError {
  Truncated,
  BadDistance { distance: usize, produced: usize },
}

impl fmt::Display for LzError {
  fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
    match self {
            LzError::Truncated => write!(f, "compressed block ended before its end marker"),
            LzError::BadDistance { distance, produced } => write!(
                f,
                "back-reference distance {distance} reaches before the start of the output ({produced} bytes produced)"
            ),
        }
  }
}

impl std::error::Error for LzError {}

/// Expands one block of the game's LZ format.
///
/// The block starts with a single literal byte. After that, each flag byte controls eight
/// items, most significant bit first: 0 is a literal byte, 1 is a back-reference. A
/// reference is two bytes: the high nibble of the first byte and the whole second byte give
/// a 12-bit distance, and the low nibble gives `18 - nibble` bytes to copy. A zero nibble
/// means a third byte follows holding `length - 18`. A distance of zero ends the block.
pub fn decompress(src: &[u8]) -> Result<Vec<u8>, LzError> {
  let mut reader = Reader { src, pos: 0 };
  let mut out = vec![reader.next()?];
  loop {
    let flags = reader.next()?;
    for bit in (0..8).rev() {
      if flags >> bit & 1 == 0 {
        out.push(reader.next()?);
        continue;
      }
      let first = reader.next()?;
      let second = reader.next()?;
      let distance = usize::from(first >> 4) << 8 | usize::from(second);
      if distance == 0 {
        return Ok(out);
      }
      let length = match first & 0x0f {
        0 => usize::from(reader.next()?) + 18,
        nibble => 18 - usize::from(nibble),
      };
      if distance > out.len() {
        return Err(LzError::BadDistance {
          distance,
          produced: out.len(),
        });
      }
      for _ in 0..length {
        out.push(out[out.len() - distance]);
      }
    }
  }
}

struct Reader<'a> {
  src: &'a [u8],
  pos: usize,
}

impl Reader<'_> {
  fn next(&mut self) -> Result<u8, LzError> {
    let byte = *self.src.get(self.pos).ok_or(LzError::Truncated)?;
    self.pos += 1;
    Ok(byte)
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn repeats_a_byte_with_a_short_reference() {
    // literal 41, flags 1100_0000: ref(dist 1, nibble f -> 3 bytes), then end marker.
    let out = decompress(&[0x41, 0xc0, 0x0f, 0x01, 0x0f, 0x00]).unwrap();
    assert_eq!(out, vec![0x41; 4]);
  }

  #[test]
  fn mixes_literals_and_the_end_marker() {
    // flags 0010_0000: literal, literal, ref to end.
    let out = decompress(&[0x41, 0x20, 0x42, 0x43, 0x0f, 0x00]).unwrap();
    assert_eq!(out, vec![0x41, 0x42, 0x43]);
  }

  #[test]
  fn extended_length_adds_eighteen() {
    let out = decompress(&[0x41, 0xc0, 0x00, 0x01, 0x02, 0x0f, 0x00]).unwrap();
    assert_eq!(out, vec![0x41; 1 + 20]);
  }

  #[test]
  fn distance_reaching_before_the_output_is_rejected() {
    // first byte 0x1f: high nibble 1 and second byte 0 give distance 256.
    assert_eq!(
      decompress(&[0x41, 0x80, 0x1f, 0x00]),
      Err(LzError::BadDistance {
        distance: 256,
        produced: 1
      })
    );
  }

  #[test]
  fn reports_truncated_input() {
    assert_eq!(decompress(&[0x41]), Err(LzError::Truncated));
    assert_eq!(decompress(&[0x41, 0xc0, 0x0f]), Err(LzError::Truncated));
  }
}
