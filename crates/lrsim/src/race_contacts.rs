//! Ordinary two-racer box contact assembly. Original power/effect locks,
//! secondary-collider interactions, reactions and audio are not complete yet.
use crate::{contact::Contacts,racer_box::{self,Body,BoxContact},racer_response::{self,Case},rivals::Rival,route_motion::ContactFreeRoute,vehicle::Vehicle};
use lrformats::cmb::Chassis;

fn shape(position:[f32;3],basis:[f32;9],center:[f32;3])->Body {
    Body {position:std::array::from_fn(|i|position[i]+(0..3).map(|j|basis[i+3*j]*center[j]).sum::<f32>()),basis,extents:[8.0,5.0,6.2],scale:1.0}
}

fn response_body(shape:&Body,velocity:[f32;3],mass:f32)->racer_response::Body {
    let [x,y,z]=shape.extents;
    // Original004412b0divides the usual box inertia by that axis extent.
    let inverse=[12.0*x/(mass*(y*y+z*z)),12.0*y/(mass*(x*x+z*z)),12.0*z/(mass*(x*x+y*y))];
    let axes=shape.axes();
    let matrix=std::array::from_fn(|i| {
        let row=i%3;let col=i/3;
        (0..3).map(|axis|axes[axis][row]*inverse[axis]*axes[axis][col]).sum()
    });
    racer_response::Body {position:shape.position,velocity,inverse_mass:1.0/mass,inverse_inertia_world:matrix}
}

/// Original ordinary free-body owned contact for two independently controlled
/// players. Retry displacement against the real track, then dispatch equal and
/// opposite velocity impulses; the reverse query observes the updated poses.
pub fn resolve_players(a:&mut Vehicle,ac:&Chassis,b:&mut Vehicle,bc:&Chassis,contacts:&Contacts,world:&crate::world_dispatch::ChassisWorld<'_>,states:(&mut crate::checkpoint_contacts::State,&mut crate::checkpoint_contacts::State))->(u32,u32) {
    let first=free_contact(a,ac,b,bc,contacts,world,states.0);let second=free_contact(b,bc,a,ac,contacts,world,states.1);
    (first.0+second.0,first.1+second.1)
}
fn free_contact(a:&mut Vehicle,ac:&Chassis,b:&mut Vehicle,bc:&Chassis,contacts:&Contacts,world:&crate::world_dispatch::ChassisWorld<'_>,state:&mut crate::checkpoint_contacts::State)->(u32,u32) {
    let lhs=shape(a.position,a.basis(),ac.offset);let rhs=shape(b.position,b.basis(),bc.offset);
    let Some(contact)=racer_box::collide(&lhs,&rhs).filter(|h|h.penetration>0.0) else {return (0,0);};
    let hits=a.try_displace_dispatched(contact.normal.map(|v|v*contact.penetration),contacts,world,state);
    let case=Case {a:response_body(&lhs,a.velocity(),ac.mass),b:response_body(&rhs,b.velocity(),bc.mass),point:contact.point,normal:contact.normal,penetration:contact.penetration};
    let impulse=racer_response::impulse(&case);a.add_velocity(contact.normal.map(|v|v*impulse/ac.mass));b.add_velocity(contact.normal.map(|v|-v*impulse/bc.mass));(1,hits)
}

fn route_shape(motion:&ContactFreeRoute<'_>,center:[f32;3])->Body {
    let mut body=shape(motion.position,motion.basis,center);
    body.basis=motion.collision_basis;
    body
}

fn rival_shape(r:&Rival<'_>)->Body {
    let mut shape=route_shape(&r.motion,r.data.chassis.offset);
    shape.scale=r.hit.collision_scale;
    shape
}

/// Complete ordinary on-route receiver portion of HandleEvent00438560.
/// Return the scalar impulse in world-units/second * mass, so the caller can
/// dispatch its negation to the other body without computing a second impulse.
pub fn route_contact(motion:&mut ContactFreeRoute<'_>,center:[f32;3],mass:f32,other:racer_response::Body,contact:&BoxContact)->f32 {
    motion.displace(contact.normal.map(|v|v*contact.penetration));
    let receiver=route_shape(motion,center);
    let case=Case {a:response_body(&receiver,motion.velocity,mass),b:other,point:contact.point,normal:contact.normal,penetration:contact.penetration};
    let impulse=racer_response::impulse(&case);
    motion.impulse(contact.normal,impulse/1000.0);
    impulse
}

fn apply_route_pair(a:&mut Rival<'_>,b:&mut Rival<'_>)->bool {
    let lhs=rival_shape(a);let rhs=rival_shape(b);
    let Some(contact)=racer_box::collide(&lhs,&rhs) else {return false;};
    if contact.penetration<=0.0 {return false;}
    let impulse=route_contact(&mut a.motion,a.data.chassis.offset,a.data.mass,response_body(&rhs,b.motion.velocity,b.data.mass),&contact);
    b.motion.impulse(contact.normal,-impulse/1000.0);
    a.hit.contact(true);b.hit.contact(true);
    true
}

pub fn resolve(player:&mut Vehicle,chassis:&Chassis,world:&Contacts,rivals:&mut [Rival<'_>])->u32 {
    resolve_with(player,chassis,rivals,|car,displacement|car.try_displace(displacement,world))
}

pub fn resolve_dispatched(player:&mut Vehicle,chassis:&Chassis,contacts:&Contacts,rivals:&mut [Rival<'_>],
    world:&crate::world_dispatch::ChassisWorld<'_>,state:&mut crate::checkpoint_contacts::State)->(u32,u32) {
    let mut checkpoint_contacts=0;
    let count=resolve_with(player,chassis,rivals,|car,displacement| {
        checkpoint_contacts+=car.try_displace_dispatched(displacement,contacts,world,state);
    });
    (count,checkpoint_contacts)
}

fn resolve_with(player:&mut Vehicle,chassis:&Chassis,rivals:&mut [Rival<'_>],mut displace:impl FnMut(&mut Vehicle,[f32;3]))->u32 {
    let mut count=0;
    for rival in rivals.iter_mut() {
        let a=shape(player.position,player.basis(),chassis.offset);let b=rival_shape(rival);
        if let Some(hit)=racer_box::collide(&a,&b).filter(|h|h.penetration>0.0) {
            displace(player,hit.normal.map(|v|v*hit.penetration));
            let case=Case {a:response_body(&a,player.velocity(),chassis.mass),b:response_body(&b,rival.motion.velocity,rival.data.mass),point:hit.point,normal:hit.normal,penetration:hit.penetration};
            let impulse=racer_response::impulse(&case);
            player.add_velocity(hit.normal.map(|n|n*impulse/chassis.mass));
            rival.motion.impulse(hit.normal,-impulse/1000.0);count+=1;
            rival.hit.contact(false);
        }
        // Original CollideAll also dispatches the opposite owned body. Query
        // again after displacement instead of reusing the old penetration.
        let a=rival_shape(rival);let b=shape(player.position,player.basis(),chassis.offset);
        if let Some(hit)=racer_box::collide(&a,&b).filter(|h|h.penetration>0.0) {
            let impulse=route_contact(&mut rival.motion,rival.data.chassis.offset,rival.data.mass,response_body(&b,player.velocity(),chassis.mass),&hit);
            player.add_velocity(hit.normal.map(|n|-n*impulse/chassis.mass));
            rival.hit.contact(false);
            count+=1;
        }
    }
    for i in 0..rivals.len() {
        let (left,right)=rivals.split_at_mut(i+1);let a=&mut left[i];
        for b in right.iter_mut() {
            count+=u32::from(apply_route_pair(a,b));count+=u32::from(apply_route_pair(b,a));
        }
    }
    count
}
