//! Original streamed ALP music with SoundBufferNode's22050Hz default.
use crate::adpcm::{self,State};

pub struct Tune {pub channels:u16,pub sample_rate:u32,pub samples:Vec<i16>}

pub fn decode(file:&[u8])->Result<Tune,String> {
    if file.len()<16 || &file[..4]!=b"ALP " || &file[8..13]!=b"ADPCM" {return Err("invalid ALP tune header".into());}
    let start=(u32::from_le_bytes(file[4..8].try_into().unwrap()) as usize).checked_add(8).ok_or("tune header overflow")?;
    if start<16 {return Err("tune header overlaps fixed fields".into());}
    let data=file.get(start..).ok_or("truncated tune header")?;
    let channels=file[15];
    let samples=match channels {
        1=>State::default().mono(data),
        2 if data.len()%2==0=>adpcm::stereo(data,&mut State::default(),&mut State::default()),
        2=>return Err("stereo tune has an incomplete channel pair".into()),
        _=>return Err(format!("unsupported tune channels{channels}")),
    };
    Ok(Tune {channels:u16::from(channels),sample_rate:22050,samples})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn streamed_header_has_no_embedded_sample_rate() {
        let tune=decode(b"ALP \x08\0\0\0ADPCM\0\0\x02\x76\xf0").unwrap();
        assert_eq!(tune.sample_rate,22050);assert_eq!(tune.channels,2);
        assert_eq!(tune.samples,[12,-12,36,-12]);
    }
    #[test]
    fn incomplete_stereo_data_and_truncated_headers_are_rejected() {
        assert!(decode(b"ALP \x08\0\0\0ADPCM\0\0\x02\x76").is_err());
        assert!(decode(b"ALP \x20\0\0\0ADPCM\0\0\x02").is_err());
    }
}
