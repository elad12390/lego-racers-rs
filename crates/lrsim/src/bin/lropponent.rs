//! Opponent pursuit oracle adapter, not a contested race or diagnostic driver.
use lrsim::opponent_pursuit::{self,Case};
use std::io::{self,Read};
fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut input=String::new();io::stdin().read_to_string(&mut input)?;
    let cases:Vec<Case>=serde_json::from_str(&input)?;
    let output:Vec<_>=cases.into_iter().map(opponent_pursuit::far_target).collect();
    println!("{}",serde_json::to_string(&output)?);Ok(())
}
