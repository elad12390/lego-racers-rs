//! Ordinary Racer::HandleEvent/UpdateEffectTimers collision-size latch.
//! Power reactions, voices/sparks and full effect state are separate systems.
use serde::{Deserialize,Serialize};

const HIT:u32=0x10000000;

#[derive(Clone,Copy,Deserialize,Serialize)]
pub struct HitState {pub flags:u32,pub remaining_ms:u32,pub collision_scale:f32}
impl Default for HitState {
    fn default()->Self {Self {flags:0,remaining_ms:0,collision_scale:1.0}}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn repeated_ai_contact_shrinks_box_but_a_single_latch_restores_next_tick() {
        let mut single=HitState::default();single.contact(true);single.advance(16);
        assert_eq!(single.remaining_ms,734);assert!(single.collision_scale<1.0);
        single.advance(16);assert_eq!(single.remaining_ms,0);assert_eq!(single.collision_scale,1.0);
        let mut repeated=HitState::default();
        for _ in 0..16 {repeated.contact(true);repeated.advance(16);}
        assert_eq!(repeated.remaining_ms,494);assert_eq!(repeated.collision_scale,0.0);
        repeated.advance(16);assert_eq!(repeated.collision_scale,1.0);
    }
    #[test]
    fn player_contact_does_not_start_ai_box_shrink_timer_and_countdown_retains_state() {
        let mut player=HitState::default();player.contact(false);player.advance(16);
        assert_eq!(player.remaining_ms,0);assert_eq!(player.collision_scale,1.0);
        let mut stopped=HitState {flags:2|HIT,remaining_ms:750,collision_scale:0.5};
        stopped.advance(1000);assert_eq!(stopped.remaining_ms,750);assert_eq!(stopped.collision_scale,0.5);assert_ne!(stopped.flags&HIT,0);
    }
}
impl HitState {
    pub fn contact(&mut self,both_ai:bool) {
        if both_ai && self.collision_scale==1.0 {self.remaining_ms=750;}
        self.flags|=HIT;
    }
    /// Original timer requires a newly latched hit each tick to retain shrinking.
    /// It does not create a generic750ms invulnerability window on one contact.
    pub fn advance(&mut self,elapsed:u32) {
        if self.flags&2!=0 {return;}
        if elapsed<self.remaining_ms && self.flags&HIT!=0 {
            self.remaining_ms-=elapsed;
            self.collision_scale=if self.remaining_ms<501 {0.0} else {
                (f64::from(self.remaining_ms-500)*f64::from(0.004f32)) as f32
            };
        } else {self.remaining_ms=0;self.collision_scale=1.0;}
        self.flags&=!HIT;
    }
}
