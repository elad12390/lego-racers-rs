//! Native projection defaults from App0042b9d0 and GolDP1001bfc0.
use macroquad::prelude::*;

// GolDP multiplies the angle by pi/360 before tan, then width by aspect.
// The original angle is therefore vertical degrees, not horizontal or radians.
pub const FOV_DEGREES:f32=65.0;
pub const NEAR:f32=5.0;
pub const FAR:f32=800.0;

/// Shared by native rendering and the original-frustum projection adapter.
pub fn view(eye:[f32;3],forward:[f32;3],up:[f32;3],aspect:f32)->Camera3D {
    let native=|[x,y,z]:[f32;3]|vec3(x,z,-y);
    let position=native(eye);
    Camera3D {position,target:position+native(forward),up:native(up),aspect:Some(aspect),
        fovy:FOV_DEGREES.to_radians(),z_near:NEAR,z_far:FAR,..Default::default()}
}
