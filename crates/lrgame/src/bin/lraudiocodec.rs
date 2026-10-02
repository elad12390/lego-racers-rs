//! Audio codec oracle adapter, no playback or audible-output claim.
use std::io::{self,Read};
use lrformats::adpcm::{self,State};
use serde::Deserialize;
#[derive(Deserialize)]
struct Case {channels:u8,bytes:Vec<u8>}
fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut input=String::new();io::stdin().read_to_string(&mut input)?;
    let cases:Vec<Case>=serde_json::from_str(&input)?;
    let output:Vec<_>=cases.into_iter().map(|c|match c.channels {
        1=>State::default().mono(&c.bytes),
        2=>adpcm::stereo(&c.bytes,&mut State::default(),&mut State::default()),
        _=>panic!("invalid codec fixture"),
    }).collect();
    println!("{}",serde_json::to_string(&output)?);Ok(())
}
