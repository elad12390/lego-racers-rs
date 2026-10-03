//! Ordinary native two-owned-racer free-body response adapter.
use lrsim::racer_response::{self, Case};
use std::io::{self, Read};
fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut input = String::new();
  io::stdin().read_to_string(&mut input)?;
  let cases: Vec<Case> = serde_json::from_str(&input)?;
  let output:Vec<_>=cases.iter().map(|c| {
        let impulse=racer_response::impulse(c);
        serde_json::json!({"a":racer_response::unowned_contact(c),"b_velocity":std::array::from_fn::<_,3,_>(|i|c.b.velocity[i]-c.normal[i]*impulse*c.b.inverse_mass)})
    }).collect();
  println!("{}", serde_json::to_string(&output)?);
  Ok(())
}
