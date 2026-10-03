//! Offline camera math adapter; does not initialize a GPU/audio window.
#[path = "../camera_rig.rs"]
#[allow(dead_code)]
mod camera_rig;
#[path = "../platform/mod.rs"]
mod platform;
use platform::prelude::*;
use serde::Deserialize;
use std::io::{self, Read};

#[derive(Deserialize)]
struct Case {
  preset: usize,
  position: [f32; 3],
  heading: [f32; 2],
  previous_position: [f32; 3],
  previous_basis: [f32; 9],
  dt_ms: f32,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut text = String::new();
  io::stdin().read_to_string(&mut text)?;
  let cases: Vec<Case> = serde_json::from_str(&text)?;
  let output:Vec<_>=cases.into_iter().map(|c| {
        let preset=camera_rig::PRESETS[c.preset];
        let desired=camera_rig::target(preset,Vec3::from_array(c.position),Vec2::from_array(c.heading));
        let pose=camera_rig::smooth(preset,(Vec3::from_array(c.previous_position),Mat3::from_cols_array(&c.previous_basis)),desired,c.dt_ms);
        serde_json::json!({"desired_position":desired.0.to_array(),"desired_basis":desired.1.to_cols_array(),"position":pose.0.to_array(),"basis":pose.1.to_cols_array()})
    }).collect();
  println!("{}", serde_json::to_string(&output)?);
  Ok(())
}
