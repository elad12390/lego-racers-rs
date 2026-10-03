//! Original numeric finish-place assignment adapter; award/camera/audio open.
use lrsim::{checkpoint_contacts::State, race_positions::freeze_finishers};
use serde::Deserialize;
use std::io::{self, Read};
#[derive(Deserialize)]
struct Case {
  ranks: Vec<u32>,
  flags: Vec<u32>,
  finish_count: u32,
  steps: Vec<Vec<bool>>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut input = String::new();
  io::stdin().read_to_string(&mut input)?;
  let cases: Vec<Case> = serde_json::from_str(&input)?;
  let output:Vec<_>=cases.into_iter().map(|mut c| {
        let mut states:Vec<_>=c.flags.into_iter().map(|flags|State {flags,..Default::default()}).collect();
        c.steps.into_iter().map(|step| {
            freeze_finishers(&mut states,&mut c.ranks,&mut c.finish_count,&step);
            serde_json::json!({"ranks":c.ranks,"flags":states.iter().map(|s|s.flags).collect::<Vec<_>>(),"finish_count":c.finish_count})
        }).collect::<Vec<_>>()
    }).collect();
  println!("{}", serde_json::to_string(&output)?);
  Ok(())
}
