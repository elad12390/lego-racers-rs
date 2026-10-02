//! Playable original pickup/tier families, modern effect simulation.
//! Interaction trajectories/tuning are provisional, not original parity claims.
use serde::{Deserialize,Serialize};
use crate::contact::{length,sub};

#[derive(Deserialize,Clone)]
pub struct Models {pub pickup:String,pub white:String,pub missile:String,pub cannon:String,pub grapple:String,pub barrel:String,pub mine:String,pub curse:String,pub beam:String,pub tether:String,pub oil:String,pub shield:[String;4],pub warp:String,pub turbo:[String;3]}
#[derive(Deserialize,Clone)]
pub struct Rules {
    pub pickup_radius:f32,pub rotation_radians_per_second:f32,pub swap_delay_ms:u32,pub spawn_fade_ms:u32,
    pub turbo_duration_ms:[u32;3],pub turbo_speed_limit:f32,pub turbo_force_scale:f32,pub turbo_route_speed:f32,
    pub shield_duration_ms:[u32;4],pub projectile_speed:f32,pub projectile_lifetime_ms:u32,pub hit_radius:f32,pub hit_delay_ms:u32,
    pub hazard_lifetime_ms:u32,pub warp_prepare_ms:u32,pub warp_transit_ms:u32,pub models:Models,pub labels:[[String;4];4],
    pub weapons:crate::power_weapons::Rules,
    pub warp_route_speed:f32,
}
impl Rules {
    pub fn load()->Result<Self,String> {
        let rules:Self=serde_json::from_str(include_str!("../../../assets/native/powerups.json")).map_err(|e|e.to_string())?;
        if [rules.pickup_radius,rules.projectile_speed,rules.hit_radius,rules.turbo_speed_limit,rules.turbo_force_scale,rules.turbo_route_speed].iter().any(|v|!v.is_finite()||*v<=0.0) {return Err("invalid powerup tuning".into());}
        Ok(rules)
    }
}
pub struct Pickup {pub source:lrformats::powerup::Pickup,pub kind:u8,pub cooldown_ms:f32,pub held_by:Option<usize>}
#[derive(Default,Clone,Serialize)]
pub struct Inventory {pub kind:u8,pub whites:u8,pub turbo_ms:f32,pub turbo_tier:u8,pub shield_ms:f32,pub shield_tier:u8,pub hit_ms:f32,pub warp_ms:f32,pub uses:u32,pub pickups:u32,pub held_ms:f32,pub oil_ms:f32,pub curse_ms:f32,pub grapple_ms:f32,pub grapple_target:Option<usize>}
#[derive(Clone,Copy)]
pub struct Racer {pub position:[f32;3],pub forward:[f32;3],pub finished:bool}
pub struct Powerups {pub rules:Rules,pub pickups:Vec<Pickup>,pub inventories:Vec<Inventory>,pub weapons:crate::power_weapons::Weapons,pub phase:f32}

impl Powerups {
    pub fn load(library:&lrformats::library::Library,table:&str,racers:usize)->Result<Self,String> {
        let archive=lrformats::tok::parse(library.find_in(&format!("{table}.RAB"),table).ok_or("missing race archive")?).map_err(|e|e.to_string())?;
        let body=archive.iter().find_map(|n|if let lrformats::tok::Node::Block(b)=n {Some(b)}else {None}).ok_or("missing race archive body")?;
        let binding=body.iter().find_map(|n|if let lrformats::tok::Node::StringRecord {kind:0x18,fields}=n {fields.first()}else {None}).ok_or("missing original pickup binding")?;
        let file=format!("{}b",binding.strip_suffix('f').ok_or("invalid original pickup filename")?);
        Self::new(lrformats::powerup::parse(library.find_in(&file,table).ok_or("missing race pickups")?,false)?,racers)
    }
    pub fn new(placements:Vec<lrformats::powerup::Pickup>,racers:usize)->Result<Self,String> {
        let rules=Rules::load()?;
        Ok(Self {pickups:placements.into_iter().map(|source|Pickup {kind:source.kind,source,cooldown_ms:0.0,held_by:None}).collect(),
            inventories:vec![Inventory::default();racers],rules,weapons:Default::default(),phase:0.0})
    }
    pub fn reset(&mut self) {for pickup in &mut self.pickups {pickup.kind=pickup.source.kind;pickup.cooldown_ms=0.0;pickup.held_by=None;}self.inventories.fill(Inventory::default());self.weapons=Default::default();self.phase=0.0;}
    pub fn collect(&mut self,racers:&[Racer]) {
        for pickup in &mut self.pickups {
            if pickup.cooldown_ms>0.0||pickup.held_by.is_some() {continue;}
            for (index,racer) in racers.iter().enumerate() {
                if racer.finished||length(sub(racer.position,pickup.source.position))>self.rules.pickup_radius {continue;}
                let inventory=&mut self.inventories[index];
                if pickup.kind==0 {
                    if inventory.whites==3 {continue;}
                    inventory.whites+=1;pickup.held_by=Some(index);
                } else {
                    let old=inventory.kind;inventory.kind=pickup.kind;inventory.held_ms=0.0;
                    pickup.kind=if old==0 {pickup.source.kind} else {old};
                    pickup.cooldown_ms=if old==0 {(pickup.source.delay_ms+self.rules.spawn_fade_ms) as f32} else {self.rules.swap_delay_ms as f32};
                }
                inventory.pickups+=1;break;
            }
        }
    }
    pub fn use_power(&mut self,owner:usize,racers:&[Racer])->bool {
        if owner>=racers.len()||owner>=self.inventories.len() {return false;}
        if racers[owner].finished||self.inventories[owner].kind==0 {return false;}
        let inventory=&mut self.inventories[owner];let kind=inventory.kind;let tier=inventory.whites;
        inventory.kind=0;inventory.whites=0;inventory.uses+=1;inventory.held_ms=0.0;
        for pickup in &mut self.pickups {if pickup.held_by==Some(owner) {pickup.held_by=None;pickup.cooldown_ms=(pickup.source.delay_ms+self.rules.spawn_fade_ms) as f32;}}
        match kind {
            2=>{inventory.shield_ms=self.rules.shield_duration_ms[tier as usize] as f32;inventory.shield_tier=tier;},
            3 if tier<3=>{inventory.turbo_ms=self.rules.turbo_duration_ms[tier as usize] as f32;inventory.turbo_tier=tier;},
            3=>inventory.warp_ms=(self.rules.warp_prepare_ms+self.rules.warp_transit_ms) as f32,
            _=>self.weapons.launch(owner,tier,kind,racers,&mut self.inventories,&self.rules),
        }
        true
    }
    pub fn advance(&mut self,dt:f32,racers:&[Racer],blocks:impl FnMut([f32;3],[f32;3])->bool) {
        if dt<=0.0 {return;}
        let ms=dt*1000.0;self.phase=(self.phase+dt*self.rules.rotation_radians_per_second)%std::f32::consts::TAU;
        for pickup in &mut self.pickups {pickup.cooldown_ms=(pickup.cooldown_ms-ms).max(0.0);}
        for inventory in &mut self.inventories {
            if inventory.kind!=0 {inventory.held_ms+=ms;}
            for timer in [&mut inventory.turbo_ms,&mut inventory.shield_ms,&mut inventory.hit_ms,&mut inventory.warp_ms,&mut inventory.oil_ms,&mut inventory.curse_ms,&mut inventory.grapple_ms] {*timer=(*timer-ms).max(0.0);}
            if inventory.grapple_ms==0.0 {inventory.grapple_target=None;}
        }
        self.weapons.advance(dt,racers,&mut self.inventories,&self.rules,blocks);
    }
}
