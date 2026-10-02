//! Original Game_State2_Tick's3000ms race-release gate; not the cinematic intro.
use serde::{Deserialize,Serialize};

#[derive(Default,Debug,Deserialize,Serialize)]
pub struct StartGate {elapsed:f64}

impl StartGate {
    pub fn released(&self)->bool {self.elapsed>=3.0}

    pub fn numeral(&self)->Option<u32> {
        if self.released() {None} else {Some((3.0-self.elapsed).ceil() as u32)}
    }

    /// Return only playable time; do not leak countdown time into car/race clocks.
    pub fn advance(&mut self,dt:f64)->f64 {
        if !dt.is_finite() || dt<=0.0 {return 0.0;}
        let waiting=(3.0-self.elapsed).max(0.0);
        let consumed=dt.min(waiting);
        self.elapsed=(self.elapsed+consumed).min(3.0);
        dt-consumed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_playable_time_before_three_seconds_or_on_invalid_host_delta() {
        let mut gate=StartGate::default();
        assert_eq!(gate.numeral(),Some(3));
        assert_eq!(gate.advance(2.999),0.0);
        assert_eq!(gate.numeral(),Some(1));
        assert!(!gate.released());
        assert_eq!(gate.advance(f64::NAN),0.0);
        assert!(!gate.released());
        let playable=gate.advance(0.101);
        assert!((playable-0.1).abs()<1e-12);
        assert!(gate.released());
        assert_eq!(gate.numeral(),None);
        assert_eq!(gate.advance(0.25),0.25);
    }
}
