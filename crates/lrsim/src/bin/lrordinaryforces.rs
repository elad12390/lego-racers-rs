use std::io::{self, Read};
fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut text = String::new();
  io::stdin().read_to_string(&mut text)?;
  let cases: Vec<lrsim::ordinary_forces::Case> = serde_json::from_str(&text)?;
  println!(
    "{}",
    serde_json::to_string(
      &cases
        .iter()
        .map(lrsim::ordinary_forces::solve)
        .collect::<Vec<_>>()
    )?
  );
  Ok(())
}
