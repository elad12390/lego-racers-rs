//! Original wheel speed/rotation sampler adapter; no GPU/audio initialization.
use lrformats::{animation,library::Library};
use lrsim::wheel_playback::WheelPlayback;
use macroquad::prelude::*;
use serde::Deserialize;
use std::io::{self,Read};

#[derive(Deserialize)]
struct Case {speed:f32,dt:f32,time:f32,reversed:bool}

fn main()->Result<(),Box<dyn std::error::Error>> {
    let library=Library::open(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM"))?;
    let animation=animation::parse(library.find_in("BKJMW.ADB","COMMON").ok_or("missing wheels")?)?;
    let mut text=String::new();io::stdin().read_to_string(&mut text)?;
    let cases:Vec<Case>=serde_json::from_str(&text)?;
    let output:Vec<_>=cases.into_iter().map(|c| {
        let mut playback=WheelPlayback {time:c.time,reversed:c.reversed};
        playback.advance(c.speed,c.dt,animation.clips[0].loop_duration,animation.clips[1].loop_duration);
        let (a,b,f)=animation.rotation_keys(playback.clip(),1,playback.time).unwrap().unwrap();
        let a=Quat::from_array(a);let mut b=Quat::from_array(b);
        if a.dot(b)<=0.0 {b=-b;}
        let quaternion=a*(1.0-f)+b*f;
        //00449340normalizes matrix generation, not stored interpolated keys.
        let basis=Mat3::from_quat(quaternion.normalize().conjugate());
        serde_json::json!({"playback":playback,"quaternion":quaternion.to_array(),"basis":basis.to_cols_array()})
    }).collect();
    println!("{}",serde_json::to_string(&output)?);Ok(())
}
