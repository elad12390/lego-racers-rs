//! Distinct original red/blue weapon families. Native trajectories are
//! provisional; original target cones/models, not original physics parity.
use crate::{contact::{dot,length,normalized,sub},powerups::{Inventory,Racer}};
use serde::{Serialize,Deserialize};

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize)]
pub enum Attack {Cannon,Grapple,Lightning,Missile,Oil,Barrel,MagneticMine,Curse}
#[derive(Clone,Deserialize)]
pub struct Rules {
    pub range:f32,pub cannon_cone:f32,pub grapple_cone:f32,pub lightning_cone:f32,
    pub launch_distance:f32,pub launch_height:f32,pub spread:f32,pub grapple_ms:u32,pub grapple_pull_speed:f32,
    pub oil_ms:u32,pub curse_ms:u32,pub mine_radius:f32,pub mine_arm_ms:u32,pub lightning_flash_ms:u32,
    pub hit_drag_per_second:f32,pub curse_speed_scale:f32,
}
pub struct Projectile {pub owner:usize,pub attack:Attack,pub position:[f32;3],pub velocity:[f32;3],pub remaining_ms:f32,pub target:Option<usize>}
pub struct Zone {pub owner:usize,pub attack:Attack,pub position:[f32;3],pub remaining_ms:f32,pub armed_ms:f32}
pub struct Flash {pub owner:usize,pub target:usize,pub attack:Attack,pub remaining_ms:f32}
#[derive(Default)]
pub struct Weapons {pub projectiles:Vec<Projectile>,pub zones:Vec<Zone>,pub flashes:Vec<Flash>,pub impacts:u32,pub blocked:u32,pub reflected:u32,pub hits:[u32;8]}
impl Weapons {
    pub fn launch(&mut self,owner:usize,tier:u8,kind:u8,racers:&[Racer],inventories:&mut [Inventory],rules:&crate::powerups::Rules) {
        let attacker=racers[owner];let forward=if kind==4 {attacker.forward.map(|v|-v)} else {attacker.forward};
        let attack=if kind==1 {[Attack::Cannon,Attack::Grapple,Attack::Lightning,Attack::Missile][tier as usize]} else {[Attack::Oil,Attack::Barrel,Attack::MagneticMine,Attack::Curse][tier as usize]};
        let cone=match attack {Attack::Cannon=>rules.weapons.cannon_cone,Attack::Grapple=>rules.weapons.grapple_cone,Attack::Lightning=>rules.weapons.lightning_cone,_=>rules.weapons.cannon_cone};
        let target=racers.iter().enumerate().filter(|(i,r)|*i!=owner&&!r.finished).filter_map(|(i,r)| {
            let delta=sub(r.position,attacker.position);let distance=length(delta);
            (distance>0.0&&distance<=rules.weapons.range&&dot(normalized(delta),forward)>cone).then_some((i,distance))
        }).min_by(|a,b|a.1.total_cmp(&b.1)).map(|(i,_)|i);
        if attack==Attack::Lightning {
            if let Some(target)=target {self.impact(owner,target,attack,inventories,rules);self.flashes.push(Flash {owner,target,attack,remaining_ms:rules.weapons.lightning_flash_ms as f32});}
            return;
        }
        let mut position=std::array::from_fn(|i|attacker.position[i]+forward[i]*rules.weapons.launch_distance);
        if matches!(attack,Attack::Oil|Attack::MagneticMine) {
            self.zones.push(Zone {owner,attack,position,remaining_ms:rules.hazard_lifetime_ms as f32,armed_ms:rules.weapons.mine_arm_ms as f32});return;
        }
        position[2]+=rules.weapons.launch_height;
        for variant in 0..if matches!(attack,Attack::Missile|Attack::Curse) {3} else {1} {
            let side=match variant {1=>-rules.weapons.spread,2=>rules.weapons.spread,_=>0.0};
            let direction=[forward[0]-forward[1]*side,forward[1]+forward[0]*side,forward[2]];
            self.projectiles.push(Projectile {owner,attack,position,velocity:normalized(direction).map(|v|v*rules.projectile_speed),remaining_ms:rules.projectile_lifetime_ms as f32,target});
        }
    }
    pub fn impact(&mut self,owner:usize,target:usize,attack:Attack,inventories:&mut [Inventory],rules:&crate::powerups::Rules)->bool {
        if inventories[target].shield_ms>0.0 {self.blocked+=1;return false;}
        match attack {
            Attack::Grapple=>{inventories[owner].grapple_target=Some(target);inventories[owner].grapple_ms=rules.weapons.grapple_ms as f32;},
            Attack::Oil=>inventories[target].oil_ms=rules.weapons.oil_ms as f32,
            Attack::Curse=>inventories[target].curse_ms=rules.weapons.curse_ms as f32,
            _=>inventories[target].hit_ms=rules.hit_delay_ms as f32,
        }
        if attack!=Attack::Grapple {inventories[target].turbo_ms=0.0;}
        self.impacts+=1;self.hits[attack as usize]+=1;true
    }
    pub fn advance(&mut self,dt:f32,racers:&[Racer],inventories:&mut [Inventory],rules:&crate::powerups::Rules,mut blocks:impl FnMut([f32;3],[f32;3])->bool) {
        if dt<=0.0 {return;}
        let ms=dt*1000.0;
        for flash in &mut self.flashes {flash.remaining_ms-=ms;}self.flashes.retain(|f|f.remaining_ms>0.0);
        let mut projectiles=std::mem::take(&mut self.projectiles);
        for p in &mut projectiles {
            p.remaining_ms-=ms;if p.remaining_ms<=0.0 {continue;}
            if matches!(p.attack,Attack::Grapple|Attack::Missile|Attack::Barrel|Attack::Curse) {
                if let Some(target)=p.target.filter(|i|!racers[*i].finished) {p.velocity=normalized(sub(racers[target].position,p.position)).map(|v|v*rules.projectile_speed);}
            }
            let old=p.position;p.position=std::array::from_fn(|i|old[i]+p.velocity[i]*dt);
            if blocks(old,p.position) {p.remaining_ms=0.0;continue;}
            for (target,racer) in racers.iter().enumerate() {
                if target==p.owner||racer.finished {continue;}
                let delta=sub(p.position,old);let square=dot(delta,delta);let fraction=if square>0.0 {(dot(sub(racer.position,old),delta)/square).clamp(0.0,1.0)} else {0.0};
                let nearest=std::array::from_fn(|i|old[i]+delta[i]*fraction);
                if length(sub(nearest,racer.position))>rules.hit_radius {continue;}
                if inventories[target].shield_ms>0.0&&inventories[target].shield_tier==3 {
                    let attacker=p.owner;p.owner=target;p.target=Some(attacker);p.velocity=p.velocity.map(|v|-v);self.reflected+=1;
                    break;
                }
                self.impact(p.owner,target,p.attack,inventories,rules);p.remaining_ms=0.0;break;
            }
        }
        projectiles.retain(|p|p.remaining_ms>0.0);self.projectiles=projectiles;
        let mut zones=std::mem::take(&mut self.zones);
        for zone in &mut zones {
            zone.remaining_ms-=ms;zone.armed_ms=(zone.armed_ms-ms).max(0.0);
            if zone.remaining_ms<=0.0||zone.armed_ms>0.0 {continue;}
            let radius=if zone.attack==Attack::MagneticMine {rules.weapons.mine_radius} else {rules.hit_radius};
            for (target,racer) in racers.iter().enumerate() {
                if target==zone.owner||racer.finished||length(sub(racer.position,zone.position))>radius {continue;}
                self.impact(zone.owner,target,zone.attack,inventories,rules);
                if zone.attack==Attack::MagneticMine {self.flashes.push(Flash {owner:zone.owner,target,attack:zone.attack,remaining_ms:rules.weapons.lightning_flash_ms as f32});}
                zone.remaining_ms=0.0;break;
            }
        }
        zones.retain(|z|z.remaining_ms>0.0);self.zones=zones;
    }
}
