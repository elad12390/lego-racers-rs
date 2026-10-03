//! Persistent primary-JAM-tree support replay, not complete vehicle acceptance.
use lrformats::{collision_tree, library::Library};
use lrsim::{
  ground::Ground,
  support::{solve_cached, suspension_points, ContactCache, SupportCase},
};
use serde::Deserialize;
use std::io::{self, Read};
#[derive(Deserialize)]
struct Fixture {
  jam: String,
  table: String,
  mesh: String,
  cases: Vec<SupportCase>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut input = String::new();
  io::stdin().read_to_string(&mut input)?;
  let fixtures: Vec<Fixture> = serde_json::from_str(&input)?;
  let mut output = Vec::new();
  for fixture in fixtures {
    let library = Library::open(&fixture.jam)?;
    let tree = collision_tree::parse(
      library
        .find_in(&fixture.mesh, &fixture.table)
        .ok_or("missing original BVB")?,
    )?;
    let ground = Ground::with_tree(std::sync::Arc::new(tree), Vec::new());
    let mut cache = ContactCache::default();
    output.push(fixture.cases.iter().map(|case|serde_json::json!({"points":suspension_points(case),"support":solve_cached(case,&ground,&mut cache)})).collect::<Vec<_>>());
  }
  println!("{}", serde_json::to_string(&output)?);
  Ok(())
}
