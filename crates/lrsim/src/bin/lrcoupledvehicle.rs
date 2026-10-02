//! Actual gameplay Vehicle::step on a shared finite test plane, not game QA.
use lrformats::{cmb,library::Library,world::{CollisionMesh,CollisionTriangle,StartPosition}};
use lrsim::{contact::Contacts,vehicle::{Actions,Vehicle}};
use serde::Deserialize;
use std::io::{self,Read};

#[derive(Deserialize)]
struct Case {jam:String,chassis:String,triangle:[[f32;3];3],position:[f32;3],forward:[f32;3],up:[f32;3],velocity:[f32;3],throttle:f32,steer:f32,ticks:u32,frames:usize,#[serde(default)]observe_support:bool,#[serde(default)]controls:Option<Vec<[f32;2]>>}
fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut input=String::new();io::stdin().read_to_string(&mut input)?;
    let cases:Vec<Case>=serde_json::from_str(&input)?;let mut output=Vec::new();
    for c in cases {
        let library=Library::open(&c.jam)?;
        let chassis=cmb::parse(library.find_in("CHASSIS.CMB","COMMON").ok_or("missing chassis table")?)?
            .into_iter().find(|x|x.name.eq_ignore_ascii_case(&c.chassis)).ok_or("missing chassis")?;
        let contacts=Contacts::new(CollisionMesh {vertices:c.triangle.to_vec(),triangles:vec![CollisionTriangle {indices:[0,1,2],surface:0}],..Default::default()});
        let start=StartPosition {slot:0,position:c.position,forward:c.forward,up:c.up};
        let mut car=Vehicle::spawn(&start,&chassis,&contacts);car.set_velocity(c.velocity);
        let mut trace=Vec::new();
        if c.controls.as_ref().is_some_and(|v|v.len()!=c.frames) {return Err("controls must specify every requested frame".into());}
        for index in 0..c.frames {
            let mut inputs=Vec::new();
            let [throttle,steer]=c.controls.as_ref().map_or([c.throttle,c.steer],|v|v[index]);
            car.step_observed(Actions {throttle,steer},&contacts,c.ticks as f32/1000.0,&mut |input|if c.observe_support {inputs.push(input.clone());});
            let mut frame=serde_json::json!({"position":car.position,"velocity":car.velocity(),"forward":car.forward(),"up":car.up,"wheels":car.supported_wheels,"turn_rate":car.turn_rate});
            if c.observe_support {frame["support_inputs"]=serde_json::to_value(inputs)?;}
            trace.push(frame);
        }
        let normal=contacts.ground.at(0.0,0.0,1000.0).ok_or("fixture plane absent")?.normal;
        let offset=-(normal.into_iter().zip(c.triangle[0]).map(|(n,v)|f64::from(n)*f64::from(v)).sum::<f64>()) as f32;
        output.push(serde_json::json!({"plane":[normal[0],normal[1],normal[2],offset],"chassis":{"offset":chassis.offset,"mass":chassis.mass,"points_a":chassis.points_a,"gear_range":chassis.gear_range,
            "rating_a":chassis.rating_a,"rating_b":chassis.rating_b,"rating_c":chassis.rating_c},"trace":trace}));
    }
    println!("{}",serde_json::to_string(&output)?);Ok(())
}
