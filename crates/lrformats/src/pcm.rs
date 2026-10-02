use std::fmt;

const MAGIC: &[u8; 4] = b"ALP ";
const CODEC: &[u8; 5] = b"ADPCM";
const RATE_OFFSET: usize = 16;
const MIN_HEADER: usize = 20;

#[derive(Debug, PartialEq, Eq)]
pub enum PcmError {
    TooShort,
    BadMagic,
    UnsupportedCodec,
    UnsupportedChannels(u8),
}

impl fmt::Display for PcmError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PcmError::TooShort => write!(f, "file is shorter than the 20-byte PCM header"),
            PcmError::BadMagic => write!(f, "missing ALP magic"),
            PcmError::UnsupportedCodec => write!(f, "codec is not ADPCM"),
            PcmError::UnsupportedChannels(n) => write!(f, "{n} channels are not supported"),
        }
    }
}

impl std::error::Error for PcmError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Sound {
    pub sample_rate: u32,
    pub samples: Vec<i16>,
}

pub fn decode(file: &[u8]) -> Result<Sound, PcmError> {
    if file.len() < MIN_HEADER {
        return Err(PcmError::TooShort);
    }
    if &file[..4] != MAGIC {
        return Err(PcmError::BadMagic);
    }
    if &file[8..13] != CODEC {
        return Err(PcmError::UnsupportedCodec);
    }
    let channels = file[15];
    if channels != 1 {
        return Err(PcmError::UnsupportedChannels(channels));
    }
    let header_len = u32::from_le_bytes(file[4..8].try_into().unwrap()) as usize + 8;
    let sample_rate = u32::from_le_bytes(file[RATE_OFFSET..RATE_OFFSET + 4].try_into().unwrap());
    let data = file.get(header_len..).unwrap_or(&[]);
    Ok(Sound {
        sample_rate,
        samples: decode_adpcm(data),
    })
}

pub fn decode_adpcm(data: &[u8]) -> Vec<i16> {
    crate::adpcm::State::default().mono(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_nibbles_follow_the_step_table() {
        // 7 at step 7 -> (7*7)>>2 = 12, index 0+8; 6 at step 16 -> (6*16)>>2 = 24, total 36.
        assert_eq!(decode_adpcm(&[0x76]), vec![12, 36]);
    }

    #[test]
    fn sign_bit_subtracts_and_index_can_fall_back() {
        // 0xf at step 7 -> -12, index 8; 0x0 at step 16 -> no change, index 7.
        assert_eq!(decode_adpcm(&[0xf0]), vec![-12, -12]);
    }

    #[test]
    fn rejects_short_and_foreign_files() {
        assert_eq!(decode(&[0; 8]), Err(PcmError::TooShort));
        assert_eq!(decode(&[0; 24]), Err(PcmError::BadMagic));
    }

    #[test]
    fn parses_header_rate_and_payload() {
        let mut file = b"ALP \x0c\0\0\0ADPCM\0\0\x01".to_vec();
        file.extend_from_slice(&11025u32.to_le_bytes());
        file.push(0x76);
        let sound = decode(&file).unwrap();
        assert_eq!(sound.sample_rate, 11025);
        assert_eq!(sound.samples, vec![12, 36]);
    }
}
