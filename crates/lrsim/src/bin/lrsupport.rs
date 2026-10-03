//! Complete native wheel-support probe for unchanged-x86 comparison fixtures.
use lrformats::world::{CollisionMesh, CollisionTriangle};
use lrsim::{
  ground::Ground,
  support::{solve, SupportCase},
};
use serde::Deserialize;
use std::io::{self, Read};

#[derive(Deserialize)]
struct Fixture {
  case: SupportCase,
  triangles: Vec<[[f32; 3]; 3]>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let mut input = String::new();
  io::stdin().read_to_string(&mut input)?;
  let fixtures: Vec<Fixture> = serde_json::from_str(&input)?;
  let results: Vec<_> = fixtures
    .into_iter()
    .map(|f| {
      let mut mesh = CollisionMesh::default();
      for triangle in f.triangles {
        let base = mesh.vertices.len() as u32;
        mesh.vertices.extend(triangle);
        mesh.triangles.push(CollisionTriangle {
          indices: [base, base + 1, base + 2],
          surface: 0,
        });
      }
      solve(&f.case, &Ground::new(mesh))
    })
    .collect();
  println!("{}", serde_json::to_string(&results)?);
  Ok(())
}
