//! Numeric driver-selection adapter, not full playback certification.
use std::io::{self,Read};
fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut input=String::new();io::stdin().read_to_string(&mut input)?;
    let cases:Vec<lrsim::driver_selection::Input>=serde_json::from_str(&input)?;
    let results:Vec<_>=cases.iter().map(lrsim::driver_selection::select).collect();
    println!("{}",serde_json::to_string(&results)?);Ok(())
}
