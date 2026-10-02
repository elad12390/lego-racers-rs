//! Angular-integration oracle adapter. No gameplay input or asset substitution.
use std::io::{self,Read};
use lrsim::attitude::{self,Basis};
use serde::Deserialize;

#[derive(Deserialize)]
struct Case {basis:Basis,velocity:[f32;3],torque:[f32;3],dt:f32}

fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut text=String::new();io::stdin().read_to_string(&mut text)?;
    if std::env::args().nth(1).as_deref()==Some("--tilt") {
        let cases:Vec<Basis>=serde_json::from_str(&text)?;
        let output:Vec<_>=cases.into_iter().map(attitude::clamp_tilt).collect();
        println!("{}",serde_json::to_string(&output)?);return Ok(());
    }
    let cases:Vec<Case>=serde_json::from_str(&text)?;
    let output:Vec<_>=cases.into_iter().map(|c|serde_json::json!({
        "basis":attitude::integrate(c.basis,c.velocity,c.dt),
        "native_basis":attitude::integrate_rotation(c.basis,c.velocity,c.dt),
        "sleeps":attitude::sleeps(c.velocity,c.torque,c.dt),
    })).collect();
    println!("{}",serde_json::to_string(&output)?);Ok(())
}
