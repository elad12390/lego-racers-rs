//! Native contact-free route motion adapter, no visual/opponent acceptance claim.
use lrformats::route::RouteRecord;
use lrsim::route_motion::ContactFreeRoute;
use serde::Deserialize;
use std::io::{self,Read};
#[derive(Deserialize)]
struct Case {path:String,speed:f32,multiplier:f32,dt:u32,frames:usize}
fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut input=String::new();io::stdin().read_to_string(&mut input)?;
    let cases:Vec<Case>=serde_json::from_str(&input)?;
    let mut output=Vec::new();
    for c in cases {
        let record=RouteRecord::load(&std::fs::read(c.path)?,false)?;
        let mut motion=ContactFreeRoute::at_start(&record);
        motion.cursor.speed=c.speed;motion.multiplier=c.multiplier;
        output.push((0..c.frames).map(|_| {
            motion.advance(c.dt);
            serde_json::json!({"position":motion.position,"basis":motion.basis,"velocity":motion.velocity.map(|v|v/1000.0),"speed":motion.cursor.speed,"time":motion.cursor.time})
        }).collect::<Vec<_>>());
    }
    println!("{}",serde_json::to_string(&output)?);Ok(())
}
