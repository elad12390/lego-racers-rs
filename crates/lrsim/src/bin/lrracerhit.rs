//! Hit-latch state adapter; complete original checker executes real receivers.
use lrsim::racer_hit::HitState;
use serde::Deserialize;
use std::io::{self,Read};
#[derive(Deserialize)]
struct Step {elapsed:u32,contact:bool,both_ai:bool}
#[derive(Deserialize)]
struct Case {initial:HitState,steps:Vec<Step>}
fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut input=String::new();io::stdin().read_to_string(&mut input)?;
    let cases:Vec<Case>=serde_json::from_str(&input)?;
    let output:Vec<_>=cases.into_iter().map(|c| {
        let mut state=c.initial;
        c.steps.into_iter().map(|s| {if s.contact {state.contact(s.both_ai);}state.advance(s.elapsed);state}).collect::<Vec<_>>()
    }).collect();
    println!("{}",serde_json::to_string(&output)?);Ok(())
}
