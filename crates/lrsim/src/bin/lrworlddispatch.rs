//! Full original primary/CPB query set on supplied probes, not coupled gameplay.
use std::io::{self,Read};
use serde::Deserialize;
use lrformats::{library::Library,collision_tree};
use lrsim::{chassis_dispatch::Collider,collider_transform::ColliderTransform,checkpoint_contacts::{CheckpointContacts,State},world_dispatch::ChassisWorld};
#[derive(Deserialize)]
struct Case {jam:String,table:String,primary:String,starts:[[f32;3];4],ends:[[f32;3];4],state:State,include_primary:bool,ticks:u32,flags:u32}
fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut input=String::new();io::stdin().read_to_string(&mut input)?;
    let cases:Vec<Case>=serde_json::from_str(&input)?;let mut output=Vec::new();
    for mut c in cases {
        let library=Library::open(&c.jam)?;let checkpoints=CheckpointContacts::load(&library,&c.table)?;
        let tree=std::sync::Arc::new(collision_tree::parse(library.find_in(&c.primary,&c.table).ok_or("missing primary")?)?);
        let primary=Collider {tree,transform:ColliderTransform {origin:[0.0;3],axes:[[1.0,0.0,0.0],[0.0,1.0,0.0],[0.0,0.0,1.0]]}};
        let world=ChassisWorld::new(primary,&checkpoints);let mut callbacks=Vec::new();
        let (selection,checkpoint_contacts)=world.dispatch(c.starts,c.ends,c.include_primary,&mut c.state,
            |_,_,_|c.flags&0x10000==0,|collider,probe,hit|callbacks.push(serde_json::json!({"collider":collider,"probe":probe,"surface":hit.surface,"point":hit.point,"normal":hit.normal})));
        let retry=if selection.improvement_count==0 {0} else {((f64::from(c.ticks)*f64::from(selection.fraction)) as u32).saturating_sub(5)};
        output.push(serde_json::json!({"selection":selection,"state":c.state,"checkpoint_contacts":checkpoint_contacts,"retry":retry,"callbacks":callbacks}));
    }
    println!("{}",serde_json::to_string(&output)?);Ok(())
}
