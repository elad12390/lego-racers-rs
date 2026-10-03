//! Actual JAM trees through complete primary/secondary four-probe selection.
use lrformats::{collision_tree, library::Library};
use lrsim::{
  chassis_dispatch::{self, Collider},
  collider_transform::ColliderTransform,
};
use serde::Deserialize;
use std::io::{self, Read};
#[derive(Deserialize)]
struct Definition {
  table: String,
  mesh: String,
  transform: ColliderTransform,
  flags: Vec<u32>,
}
#[derive(Deserialize)]
struct Case {
  jam: String,
  colliders: Vec<Definition>,
  starts: [[f32; 3]; 4],
  ends: [[f32; 3]; 4],
  include_primary: bool,
  ticks: u32,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut input = String::new();
  io::stdin().read_to_string(&mut input)?;
  let cases: Vec<Case> = serde_json::from_str(&input)?;
  let mut output = Vec::new();
  for c in cases {
    let library = Library::open(&c.jam)?;
    let colliders = c
      .colliders
      .iter()
      .map(|d| {
        let tree = collision_tree::parse(
          library
            .find_in(&d.mesh, &d.table)
            .ok_or("missing collider")?,
        )?;
        if d.flags.len() != tree.mesh.names.len() {
          return Err("incomplete surface flags".into());
        }
        Ok(Collider {
          tree: std::sync::Arc::new(tree),
          transform: d.transform,
        })
      })
      .collect::<Result<Vec<_>, String>>()?;
    let mut callbacks = Vec::new();
    let selected = chassis_dispatch::dispatch(
      &colliders,
      c.starts,
      c.ends,
      c.include_primary,
      |collider, probe, hit| {
        callbacks.push(serde_json::json!({"collider":collider,"probe":probe,"surface":hit.surface,"point":hit.point,"normal":hit.normal}));
        c.colliders[collider].flags[hit.surface as usize] & 0x10000 == 0
      },
    );
    let retry = if selected.improvement_count == 0 {
      0
    } else {
      ((f64::from(c.ticks) * f64::from(selected.fraction)) as u32).saturating_sub(5)
    };
    output.push(serde_json::json!({"selection":selected,"retry":retry,"callbacks":callbacks}));
  }
  println!("{}", serde_json::to_string(&output)?);
  Ok(())
}
