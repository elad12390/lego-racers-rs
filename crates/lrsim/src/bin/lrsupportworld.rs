//! Supplied-pose secondary/primary wheel-query replay, not gameplay acceptance.
use std::io::{self,Read};
use serde::Deserialize;
use lrformats::{library::Library,collision_tree};
use lrsim::{ground::Ground,chassis_dispatch::Collider,collider_transform::ColliderTransform,support::{ContactCache,SupportCase,solve_world,suspension_points},wheel_query};
#[derive(Deserialize)]
struct Placement {mesh:String,transform:ColliderTransform}
#[derive(Deserialize)]
struct Fixture {jam:String,table:String,primary:String,secondary:Vec<Placement>,cases:Vec<SupportCase>}
fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut input=String::new();io::stdin().read_to_string(&mut input)?;
    let fixtures:Vec<Fixture>=serde_json::from_str(&input)?;let mut output=Vec::new();
    for fixture in fixtures {
        let library=Library::open(&fixture.jam)?;
        let load=|name:&str|->Result<_,Box<dyn std::error::Error>> {Ok(std::sync::Arc::new(collision_tree::parse(library.find_in(name,&fixture.table).ok_or("missing BVB")?)?))};
        let ground=Ground::with_tree(load(&fixture.primary)?,Vec::new());
        let secondary=fixture.secondary.iter().map(|p|Ok(Collider {tree:load(&p.mesh)?,transform:p.transform})).collect::<Result<Vec<_>,Box<dyn std::error::Error>>>()?;
        let mut cache=ContactCache::default();
        output.push(fixture.cases.iter().map(|case|serde_json::json!({"points":suspension_points(case),"rays":secondary.iter().map(|c|wheel_query::rays(case,c.transform)).collect::<Vec<_>>(),
            "support":solve_world(case,&ground,&mut cache,&secondary)})).collect::<Vec<_>>());
    }
    println!("{}",serde_json::to_string(&output)?);Ok(())
}
