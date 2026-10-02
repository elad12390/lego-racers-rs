//! Constant-velocity chassis collision oracle adapter, not a driving controller.
use std::io::{self,Read};
use lrsim::{collision_step,contact::Contacts,handling::wall_response};
use lrformats::world::{CollisionMesh,CollisionTriangle};
use serde::{Deserialize,Serialize};

#[derive(Deserialize)]
struct Case {vertices:Vec<[f32;3]>,probes:[[f32;3];4],position:[f32;3],velocity:[f32;3],ticks:Vec<u32>}
#[derive(Clone,Serialize)]
struct Motion {position:[f32;3],velocity:[f32;3],collisions:u32}

fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut input=String::new();io::stdin().read_to_string(&mut input)?;
    let cases:Vec<Case>=serde_json::from_str(&input)?;
    let output:Vec<_>=cases.into_iter().map(|c| {
        let mesh=CollisionMesh {vertices:c.vertices,triangles:vec![CollisionTriangle {indices:[0,1,2],surface:0}],..Default::default()};
        let contacts=Contacts::new(mesh);
        let mut motion=Motion {position:c.position,velocity:c.velocity,collisions:0};
        let mut trace=Vec::new();
        for ticks in c.ticks {
            let probes=|m:&Motion|c.probes.map(|p|std::array::from_fn(|i|p[i]+m.position[i]));
            let complete=collision_step::advance(&mut motion,ticks as f32/1000.0,
                |m,t|for i in 0..3 {m.position[i]+=m.velocity[i]*t;},
                |a,b|collision_step::sweep(&contacts,probes(a),probes(b)),
                |m,n| {let old=m.velocity;m.velocity=wall_response(m.velocity,n);m.collisions+=1;old!=m.velocity});
            trace.push(serde_json::json!({"motion":motion,"complete":complete}));
        }
        trace
    }).collect();
    println!("{}",serde_json::to_string(&output)?);Ok(())
}
