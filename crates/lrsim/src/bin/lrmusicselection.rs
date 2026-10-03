use serde::Deserialize;
use std::io::{self, Read};
#[derive(Deserialize)]
struct Case {
  count: usize,
  circuit: bool,
  noise: u16,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut input = String::new();
  io::stdin().read_to_string(&mut input)?;
  let cases: Vec<Case> = serde_json::from_str(&input)?;
  println!(
    "{}",
    serde_json::to_string(
      &cases
        .into_iter()
        .map(|c| lrsim::music_selection::race_index(c.count, c.circuit, c.noise))
        .collect::<Vec<_>>()
    )?
  );
  Ok(())
}
