//! Original004203b0far-target pursuit geometry, not the full AI state machine.
//! Route initialization/advancement, grip/boost/noise/recovery remain required.
use crate::{contact::{dot,length,sub},handling::clamp_radius};
use serde::{Deserialize,Serialize};

#[derive(Deserialize)]
pub struct Case {
    pub position:[f32;3],pub left:[f32;3],pub target:[f32;3],pub speed:f32,pub reverse:bool,
}
#[derive(Serialize)]
pub struct Control {pub steer_target:f32,pub turn_radius:f32,pub throttle:f32}

pub fn far_target(c:Case)->Control {
    let offset=sub(c.target,c.position);let distance=length(offset);
    let lateral=dot(c.left,offset);
    let sine=lateral.abs()/distance;
    let mut radius=if sine>=0.0005 {distance/(sine+sine)} else {4096.0};
    if lateral<0.0 {radius=-radius;}
    if c.reverse {radius=-radius;}
    // Original Driver Tick's18/-54input passes through CarBody's1/54scale.
    let throttle=if c.reverse {-1.0} else {1.0/3.0};
    if radius!=0.0 && c.speed<40.0 {radius*=0.2;}
    Control {steer_target:radius,turn_radius:clamp_radius(radius),throttle}
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn far_offset_target_uses_original_circle_radius_and_forward_acceleration() {
        let control=far_target(Case {position:[0.0;3],left:[0.0,1.0,0.0],target:[100.0,40.0,0.0],speed:120.0,reverse:false});
        assert!((control.turn_radius-145.0).abs()<0.0001);
        assert!((control.throttle*54.0-18.0).abs()<0.0001);
    }
    #[test]
    fn reverse_recovery_inverts_radius_and_requests_original_reverse_input() {
        let control=far_target(Case {position:[0.0;3],left:[0.0,1.0,0.0],target:[100.0,40.0,0.0],speed:120.0,reverse:true});
        assert!((control.turn_radius+145.0).abs()<0.0001);
        assert_eq!(control.throttle*54.0,-54.0);
    }
}
