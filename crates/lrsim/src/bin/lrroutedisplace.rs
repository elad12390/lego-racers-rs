//! Ordinary native on-route displacement/impulse adapter.
use lrformats::route::RouteRecord;
use lrsim::route_motion::ContactFreeRoute;
use serde::Deserialize;
use std::io::{self, Read};
#[derive(Deserialize)]
struct Case {
  path: String,
  ticks: u32,
  displacement: [f32; 3],
  normal: [f32; 3],
  amount: f32,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut input = String::new();
  io::stdin().read_to_string(&mut input)?;
  let cases: Vec<Case> = serde_json::from_str(&input)?;
  let mut output = Vec::new();
  for c in cases {
    let record = RouteRecord::load(&std::fs::read(c.path)?, false)?;
    let mut m = ContactFreeRoute::at_start(&record);
    m.advance(c.ticks);
    m.displace(c.displacement);
    m.impulse(c.normal, c.amount);
    output.push(serde_json::json!({"position":m.position,"basis":m.basis,"speed":m.cursor.speed,"time":m.cursor.time,"slide":m.slide_offset}));
  }
  println!("{}", serde_json::to_string(&output)?);
  Ok(())
}
