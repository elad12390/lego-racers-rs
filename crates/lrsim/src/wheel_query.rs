//! Secondary wheel rays from00448d90;00445dc0 accepts the first geometric hit
//! per wheel, with no chassis owner callback or chassis material-side rejection.
use crate::{chassis_dispatch::Collider,collider_transform::ColliderTransform,support::{SupportCase,suspension_points}};
use serde::Serialize;

#[derive(Serialize)]
pub struct Rays {pub starts:[[f32;3];4],pub ends:[[f32;3];4]}

pub fn rays(case:&SupportCase,transform:ColliderTransform)->Rays {
    let anchor=transform.inverse_point(suspension_points(case)[1]);
    let upward=case.radius+case.downward_movement.max(0.0);
    let downward=if case.grounded {case.dt*40.0} else {0.0};
    let up=transform.axes.map(|axis|axis[2]*upward);
    let down=transform.axes.map(|axis|axis[2]*downward);
    let length=transform.inverse_vector(case.forward).map(|v|v*case.length);
    let width=transform.inverse_vector(case.left).map(|v|v*case.width);
    let minus_width=std::array::from_fn::<_,3,_>(|i|anchor[i]-width[i]);
    let anchors=[minus_width,anchor,std::array::from_fn(|i|minus_width[i]-length[i]),std::array::from_fn(|i|anchor[i]-length[i])];
    Rays {starts:anchors.map(|p|std::array::from_fn(|i|p[i]+up[i])),ends:anchors.map(|p|std::array::from_fn(|i|p[i]-down[i]))}
}

pub(crate) struct Contact {pub point:[f32;3],pub normal:[f32;3],pub surface:u32,pub collider:usize,pub depth:f64}

pub(crate) fn trace(case:&SupportCase,colliders:&[Collider])->[Option<Contact>;4] {
    let mut contacts=std::array::from_fn(|_|None);
    for (index,collider) in colliders.iter().enumerate() {
        let rays=rays(case,collider.transform);
        for wheel in 0..4 {
            if contacts[wheel].is_some() {continue;}
            let Some(hit)=crate::mesh_query::trace(&collider.tree,rays.starts[wheel],rays.ends[wheel]) else {continue;};
            // Secondary depth is measured in collider-local space, not from
            // the transformed world contact or the primary vertical height.
            let delta=std::array::from_fn::<_,3,_>(|i|rays.ends[wheel][i]-hit.point[i]);
            let depth=delta[0]*delta[0]+delta[1]*delta[1]+delta[2]*delta[2];
            contacts[wheel]=Some(Contact {point:collider.transform.point(hit.point),normal:collider.transform.rotate(hit.normal),
                surface:hit.surface,collider:index+1,depth:f64::from(depth)});
        }
    }
    contacts
}
