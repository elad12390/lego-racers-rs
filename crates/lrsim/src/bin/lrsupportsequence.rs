//! Original-observed support inputs on persistent cache, not whole-game proof.
use lrformats::world::{CollisionMesh,CollisionTriangle};
use lrsim::{ground::Ground,support::{ContactCache,SupportCase,solve_cached,suspension_points}};
use serde::Deserialize;
use std::io::{self,Read};
#[derive(Deserialize)]
struct Fixture {triangle:[[f32;3];3],cases:Vec<SupportCase>}
fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut input=String::new();io::stdin().read_to_string(&mut input)?;
    let fixtures:Vec<Fixture>=serde_json::from_str(&input)?;
    let output:Vec<_>=fixtures.into_iter().map(|fixture| {
        let ground=Ground::new(CollisionMesh {vertices:fixture.triangle.to_vec(),triangles:vec![CollisionTriangle {indices:[0,1,2],surface:0}],..Default::default()});
        let mut cache=ContactCache::default();
        fixture.cases.iter().map(|case|serde_json::json!({"points":suspension_points(case),"support":solve_cached(case,&ground,&mut cache)})).collect::<Vec<_>>()
    }).collect();
    println!("{}",serde_json::to_string(&output)?);Ok(())
}
