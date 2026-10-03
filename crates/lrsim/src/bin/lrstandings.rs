//! Real CPB ranking snapshot adapter, not race-contact ordering acceptance.
use lrformats::{checkpoints::CheckpointTable, library::Library};
use lrsim::standings::{self, Racer};
use serde::Deserialize;
use std::io::{self, Read};
#[derive(Deserialize)]
struct Case {
  jam: String,
  table: String,
  racers: Vec<Racer>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut input = String::new();
  io::stdin().read_to_string(&mut input)?;
  let cases: Vec<Case> = serde_json::from_str(&input)?;
  let mut output = Vec::new();
  for case in cases {
    let library = Library::open(case.jam)?;
    let checkpoints = CheckpointTable::load(
      library
        .find_in("RACE.CPB", &case.table)
        .ok_or("missing original CPB")?,
      false,
    )?;
    output.push(serde_json::json!({"ranks":standings::ranks(&case.racers,&checkpoints.records),"checkpoints":checkpoints.records.iter().map(|c|
            serde_json::json!({"plane":c.plane,"center":c.center,"progress":c.progress,"next":c.next})).collect::<Vec<_>>()}));
  }
  println!("{}", serde_json::to_string(&output)?);
  Ok(())
}
