//! Original00418100/00418310codec; high nibble first, no IMA half-step bias.
const INDEX_ADJUST: [i32; 16] = [-1, -1, -1, -1, 2, 4, 6, 8, -1, -1, -1, -1, 2, 4, 6, 8];
const STEP_SIZES: [i32; 89] = [
    7, 8, 9, 10, 11, 12, 13, 14, 16, 17, 19, 21, 23, 25, 28, 31, 34, 37, 41, 45, 50, 55, 60, 66,
    73, 80, 88, 97, 107, 118, 130, 143, 157, 173, 190, 209, 230, 253, 279, 307, 337, 371, 408,
    449, 494, 544, 598, 658, 724, 796, 876, 963, 1060, 1166, 1282, 1411, 1552, 1707, 1878, 2066,
    2272, 2499, 2749, 3024, 3327, 3660, 4026, 4428, 4871, 5358, 5894, 6484, 7132, 7845, 8630,
    9493, 10442, 11487, 12635, 13899, 15289, 16818, 18500, 20350, 22385, 24623, 27086, 29794,
    32767,
];

#[derive(Default)]
pub struct State {predictor:i32,index:i32}

impl State {
    fn sample(&mut self,nibble:u8)->i16 {
        let step=STEP_SIZES[self.index as usize];
        self.index=(self.index+INDEX_ADJUST[nibble as usize]).clamp(0,88);
        let delta=(i32::from(nibble&7)*step)>>2;
        self.predictor=(self.predictor+if nibble&8==0 {delta} else {-delta}).clamp(-32768,32767);
        self.predictor as i16
    }

    pub fn mono(&mut self,data:&[u8])->Vec<i16> {
        data.iter().flat_map(|byte|[self.sample(byte>>4),self.sample(byte&15)]).collect()
    }
}

/// Each byte pair contains left then right, with two frames per pair.
/// State survives512byte stream blocks and arbitrary even-sized chunks.
pub fn stereo(data:&[u8],left:&mut State,right:&mut State)->Vec<i16> {
    let mut output=Vec::with_capacity(data.len()*2);
    for pair in data.chunks_exact(2) {
        output.extend([left.sample(pair[0]>>4),right.sample(pair[1]>>4),left.sample(pair[0]&15),right.sample(pair[1]&15)]);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stereo_keeps_channels_independent_and_preserves_stream_state() {
        let data=[0x76,0xf0,0x11,0x77];
        let whole=stereo(&data,&mut State::default(),&mut State::default());
        let mut left=State::default();let mut right=State::default();
        let mut chunks=stereo(&data[..2],&mut left,&mut right);
        chunks.extend(stereo(&data[2..],&mut left,&mut right));
        assert_eq!(chunks,whole);
        assert_eq!(&whole[..4],&[12,-12,36,-12]);
    }
}
