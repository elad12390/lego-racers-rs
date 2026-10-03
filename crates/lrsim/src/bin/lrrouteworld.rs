//! Complete ordinary route/CPB diagnostic, not human-play or full AI acceptance.
use lrformats::{collision_tree, library::Library, route::RouteRecord};
use lrsim::{
  chassis_dispatch::Collider,
  checkpoint_contacts::{CheckpointContacts, State},
  collider_transform::ColliderTransform,
  route_motion::ContactFreeRoute,
  world_dispatch::ChassisWorld,
};
use serde::Deserialize;
use std::io::{self, Read};
#[derive(Deserialize)]
struct Case {
  jam: String,
  table: String,
  path: String,
  speed: f32,
  multiplier: f32,
  dt: u32,
  frames: usize,
  gear_range: [f32; 2],
  flags: u32,
  #[serde(default)]
  displace: bool,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut input = String::new();
  io::stdin().read_to_string(&mut input)?;
  let cases: Vec<Case> = serde_json::from_str(&input)?;
  let mut output = Vec::new();
  for c in cases {
    let library = Library::open(&c.jam)?;
    let checkpoints = CheckpointContacts::load(&library, &c.table)?;
    let bindings = lrformats::race_archive::collisions(
      library
        .find_in(&format!("{}.RAB", c.table), &c.table)
        .ok_or("missing RAB")?,
    )?;
    let tree = std::sync::Arc::new(collision_tree::parse(
      library
        .find_in(&format!("{}.BVB", bindings.primary), &c.table)
        .ok_or("missing BVB")?,
    )?);
    let world = ChassisWorld::new(
      Collider {
        tree,
        transform: ColliderTransform {
          origin: [0.0; 3],
          axes: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
        },
      },
      &checkpoints,
    );
    let record = RouteRecord::load(&std::fs::read(&c.path)?, false)?;
    let mut motion = ContactFreeRoute::at_start(&record);
    motion.cursor.speed = c.speed;
    motion.multiplier = c.multiplier;
    let mut state = State::default();
    let mut trace = Vec::new();
    for frame in 0..c.frames {
      if c.displace && frame > 0 && frame % 28 == 0 {
        motion.displace([0.2, -0.3, 0.1]);
      }
      let mut contacts = 0;
      let blocked = motion.advance_queried(c.dt, c.gear_range, |starts, ends| {
        let (selected, count) = world.dispatch(
          starts,
          ends,
          false,
          &mut state,
          |_, _, _| c.flags & 0x10000 == 0,
          |_, _, _| {},
        );
        contacts += count;
        selected
      });
      trace.push(serde_json::json!({"position":motion.position,"basis":motion.basis,"velocity":motion.velocity.map(|v|v/1000.0),"speed":motion.cursor.speed,"time":motion.cursor.time,"state":state,"contacts":contacts,"blocked":blocked}));
    }
    output.push(trace);
  }
  println!("{}", serde_json::to_string(&output)?);
  Ok(())
}
