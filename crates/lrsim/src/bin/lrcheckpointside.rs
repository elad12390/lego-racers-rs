//! Complete checkpoint contact-state adapter; world query ordering is separate.
use serde::Deserialize;
use std::io::{self,Read};
use lrsim::checkpoint_contacts::State;
#[derive(Deserialize)]
struct Hit {index:usize,progress:f32,normal:[f32;3],hit_normal:[f32;3]}
#[derive(Deserialize)]
struct Case {state:State,hits:Vec<Hit>}
fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut input=String::new();io::stdin().read_to_string(&mut input)?;
    let cases:Vec<Case>=serde_json::from_str(&input)?;
    let results:Vec<_>=cases.into_iter().map(|mut c| {
        c.hits.into_iter().map(|hit| {c.state.touch(hit.index,hit.progress,hit.normal,hit.hit_normal);c.state.clone()}).collect::<Vec<_>>()
    }).collect();
    println!("{}",serde_json::to_string(&results)?);Ok(())
}
