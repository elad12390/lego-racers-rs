//! Complete ordinary on-route receiver event oracle adapter.
use lrformats::route::RouteRecord;
use lrsim::{
  race_contacts, racer_box::BoxContact, racer_response::Body, route_motion::ContactFreeRoute,
};
use serde::Deserialize;
use std::io::{self, Read};
#[derive(Deserialize)]
struct Case {
  path: String,
  ticks: u32,
  center: [f32; 3],
  mass: f32,
  other: Body,
  point: [f32; 3],
  normal: [f32; 3],
  penetration: f32,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut input = String::new();
  io::stdin().read_to_string(&mut input)?;
  let cases: Vec<Case> = serde_json::from_str(&input)?;
  let mut output = Vec::new();
  for c in cases {
    let record = RouteRecord::load(&std::fs::read(c.path)?, false)?;
    let mut motion = ContactFreeRoute::at_start(&record);
    motion.advance(c.ticks);
    let velocity = c.other.velocity;
    let inverse_mass = c.other.inverse_mass;
    let impulse = race_contacts::route_contact(
      &mut motion,
      c.center,
      c.mass,
      c.other,
      &BoxContact {
        penetration: c.penetration,
        normal: c.normal,
        point: c.point,
      },
    );
    output.push(serde_json::json!({"position":motion.position,"basis":motion.basis,"speed":motion.cursor.speed,"time":motion.cursor.time,"slide":motion.slide_offset,
            "other_velocity":std::array::from_fn::<_,3,_>(|i|velocity[i]-c.normal[i]*impulse*inverse_mass)}));
  }
  println!("{}", serde_json::to_string(&output)?);
  Ok(())
}
