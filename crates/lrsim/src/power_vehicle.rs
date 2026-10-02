//! Shared player power-effect dispatch over the calibrated free vehicle path.
use crate::{vehicle::{Vehicle,Actions},contact::Contacts,checkpoint_contacts::{CheckpointContacts,State},world_dispatch::ChassisWorld,powerups::{Rules,Inventory,Racer}};

pub fn advance(car:&mut Vehicle,mut input:Actions,effect:Option<(&Inventory,&Rules)>,actors:&[Racer],contacts:&Contacts,checkpoints:&CheckpointContacts,world:&ChassisWorld<'_>,state:&mut State,dt:f32)->Result<u32,String> {
    let ordinary=car.handling;
    if let Some((inventory,rules))=effect {
        if inventory.turbo_ms>0.0 {car.handling.acceleration*=rules.turbo_force_scale;car.handling.forward_limit=rules.turbo_speed_limit;}
        if inventory.hit_ms>0.0 {input.throttle=0.0;let drag=(-rules.weapons.hit_drag_per_second*dt).exp();car.set_velocity(car.velocity().map(|v|v*drag));}
        if inventory.curse_ms>0.0 {car.handling.acceleration*=rules.weapons.curse_speed_scale;car.handling.forward_limit*=rules.weapons.curse_speed_scale;}
        if inventory.oil_ms>0.0 {input.steer=(inventory.oil_ms/1000.0*std::f32::consts::TAU).sin();}
        if let Some(target)=inventory.grapple_target.filter(|_|inventory.grapple_ms>0.0).and_then(|i|actors.get(i)) {let direction=crate::contact::normalized(crate::contact::sub(target.position,car.position));car.set_velocity(direction.map(|v|v*rules.weapons.grapple_pull_speed));}
    }
    let result=if effect.is_some_and(|(i,r)|i.warp_ms>0.0&&i.warp_ms<=r.warp_transit_ms as f32) {
        let mut destination=car.position;let start=state.checkpoint.map(|i|checkpoints.table.records[i].next[0] as usize);
        crate::warp_motion::advance(&checkpoints.table,&mut destination,dt*600.0,start).map(|_|{car.effect_position(destination);0})
    }else {Ok(car.step_dispatched(input,contacts,dt,world,state))};
    car.handling=ordinary;result
}
