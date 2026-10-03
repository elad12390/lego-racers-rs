//! Original GolDP frustum corners projected with the native camera parameters.
//! Math adapter, not native GPU capture or cinematic acceptance.
#[path = "../camera_view.rs"]
mod camera_view;
#[path = "../platform/mod.rs"]
mod platform;
use platform::prelude::*;
use serde::Deserialize;
use std::io::{self, Read};
#[derive(Deserialize)]
struct Case {
  eye: [f32; 3],
  forward: [f32; 3],
  up: [f32; 3],
  aspect: f32,
  corners: Vec<[f32; 3]>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut input = String::new();
  io::stdin().read_to_string(&mut input)?;
  let cases: Vec<Case> = serde_json::from_str(&input)?;
  let output: Vec<_> = cases
    .into_iter()
    .map(|c| {
      let view = camera_view::view(c.eye, c.forward, c.up, c.aspect);
      // Exact Camera3D::matrix expressions, without its screen_width default
      // evaluating a graphics context in a headless math adapter.
      let matrix = Mat4::perspective_rh_gl(view.fovy, c.aspect, view.z_near, view.z_far)
        * Mat4::look_at_rh(view.position, view.target, view.up);
      c.corners
        .into_iter()
        .map(|[x, y, z]| {
          let p = matrix * vec4(x, z, -y, 1.0);
          (p.truncate() / p.w).to_array()
        })
        .collect::<Vec<_>>()
    })
    .collect();
  println!("{}", serde_json::to_string(&output)?);
  Ok(())
}
