pub fn encode_mono_16(sample_rate: u32, samples: &[i16]) -> Vec<u8> {
  encode_16(sample_rate, 1, samples)
}

/// Interleaved signed PCM16 WAV for the native mono/stereo audio backend.
pub fn encode_16(sample_rate: u32, channels: u16, samples: &[i16]) -> Vec<u8> {
  assert!((1..=2).contains(&channels));
  assert_eq!(samples.len() % usize::from(channels), 0);
  let data_len = (samples.len() * 2) as u32;
  let mut out = Vec::with_capacity(44 + data_len as usize);
  out.extend_from_slice(b"RIFF");
  out.extend_from_slice(&(36 + data_len).to_le_bytes());
  out.extend_from_slice(b"WAVEfmt ");
  out.extend_from_slice(&16u32.to_le_bytes());
  out.extend_from_slice(&1u16.to_le_bytes());
  out.extend_from_slice(&channels.to_le_bytes());
  out.extend_from_slice(&sample_rate.to_le_bytes());
  out.extend_from_slice(&(sample_rate * u32::from(channels) * 2).to_le_bytes());
  out.extend_from_slice(&(channels * 2).to_le_bytes());
  out.extend_from_slice(&16u16.to_le_bytes());
  out.extend_from_slice(b"data");
  out.extend_from_slice(&data_len.to_le_bytes());
  for sample in samples {
    out.extend_from_slice(&sample.to_le_bytes());
  }
  out
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn writes_a_canonical_44_byte_header() {
    let wav = encode_mono_16(11025, &[1, -1]);
    assert_eq!(&wav[..4], b"RIFF");
    assert_eq!(wav.len(), 48);
    assert_eq!(u32::from_le_bytes(wav[4..8].try_into().unwrap()), 40);
    assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 11025);
    assert_eq!(&wav[44..], &[1, 0, 0xff, 0xff]);
  }

  #[test]
  fn stereo_music_retains_channel_order_and_pcm_frame_stride() {
    let wav = encode_16(22050, 2, &[12, -12, 36, -12]);
    assert_eq!(u16::from_le_bytes(wav[22..24].try_into().unwrap()), 2);
    assert_eq!(u32::from_le_bytes(wav[28..32].try_into().unwrap()), 88200);
    assert_eq!(u16::from_le_bytes(wav[32..34].try_into().unwrap()), 4);
    assert_eq!(&wav[44..], &[12, 0, 244, 255, 36, 0, 244, 255]);
  }
}
