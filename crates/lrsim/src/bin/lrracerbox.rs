//! Native racer-box contact oracle adapter, no racing or response claim.
use lrsim::racer_box::{self,Body};
use serde::Deserialize;
use std::io::{self,Read};
#[derive(Deserialize)]
struct Case {a:Body,b:Body}
fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut input=String::new();io::stdin().read_to_string(&mut input)?;
    let cases:Vec<Case>=serde_json::from_str(&input)?;
    let output:Vec<_>=cases.into_iter().map(|c|serde_json::json!({"separation":racer_box::separating_contact(&c.a,&c.b),"contact":racer_box::collide(&c.a,&c.b)})).collect();
    println!("{}",serde_json::to_string(&output)?);Ok(())
}
