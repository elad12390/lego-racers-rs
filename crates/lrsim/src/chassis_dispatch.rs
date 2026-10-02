//! Complete four-probe query selection order from004478b0.
//! Slot0primary is world-space; secondary slots use original inverse transforms.
//! Callers own contact callbacks, effects, restore/retry and collider lifetime.
use crate::{collider_transform::ColliderTransform,mesh_query::{self,Hit}};
use lrformats::collision_tree::CollisionTree;
use serde::Serialize;

#[derive(Clone)]
pub struct Collider {pub tree:std::sync::Arc<CollisionTree>,pub transform:ColliderTransform}
#[derive(Default,Serialize)]
pub struct Selection {
    pub probe_hits:[bool;4],pub improvement_count:u32,
    pub normal:[f32;3],pub fraction:f32,pub collider:Option<usize>,pub probe:Option<usize>,
}

pub fn dispatch(colliders:&[Collider],starts:[[f32;3];4],ends:[[f32;3];4],include_primary:bool,
    mut accept:impl FnMut(usize,usize,&Hit)->bool)->Selection {
    let mut selected=Selection::default();let mut depth=-f32::MAX;
    let order=(1..colliders.len()).chain(if include_primary && !colliders.is_empty() {Some(0)} else {None});
    for index in order {
        let collider=&colliders[index];
        for probe in 0..4 {
            if selected.probe_hits[probe] {continue;}
            let (start,end)=if index==0 {(starts[probe],ends[probe])}
                else {(collider.transform.inverse_point(starts[probe]),collider.transform.inverse_point(ends[probe]))};
            let Some(hit)=mesh_query::trace(&collider.tree,start,end) else {continue;};
            let delta=std::array::from_fn::<_,3,_>(|i|f64::from(hit.point[i])-f64::from(end[i]));
            let product=|i:usize|delta[i]*f64::from(hit.normal[i]);
            let side=(product(1)+product(2))+product(0);
            if (index==0 && side<0.0) || (index!=0 && side<=0.0) {continue;}
            // Owner callback is invoked BEFORE testing record improvement.
            // A rejected checkpoint/event is not a physical blocking face.
            if !accept(index,probe,&hit) {continue;}
            let square=|i:usize|delta[i]*delta[i];
            let distance=if index==0 {(square(1)+square(0))+square(2)}
                else {(square(2)+square(1))+square(0)};
            // FCOM compares retained distance with previously spilled depth.
            // Even equal geometry can improve a rounded-down previous record.
            if f64::from(depth)<distance {
                depth=distance as f32;selected.probe_hits[probe]=true;selected.improvement_count+=1;
                selected.normal=if index==0 {hit.normal} else {collider.transform.rotate(hit.normal)};
                selected.fraction=hit.fraction;selected.collider=Some(index);selected.probe=Some(probe);
            }
        }
    }
    selected
}
