//! Original COMMON geometry/materials for pickups and active powerups.
use crate::gpu::TrackGpu;
use lrformats::{library::Library,model::Model,scene::SceneBindings};
use lrsim::powerups::{Powerups,Racer};
use lrsim::power_weapons::Attack;
use macroquad::prelude::*;
use std::collections::HashMap;

pub struct PowerupGpu {bricks:Vec<TrackGpu>,missile:TrackGpu,cannon:TrackGpu,grapple:TrackGpu,barrel:TrackGpu,mine:TrackGpu,curse:TrackGpu,beam:TrackGpu,tether:TrackGpu,oil:Texture2D,shields:Vec<TrackGpu>,warp:TrackGpu,turbos:Vec<TrackGpu>}
impl PowerupGpu {
    pub fn load(library:&Library,rules:&lrsim::powerups::Rules)->Result<Self,String> {
        let bindings=SceneBindings::load(library,"COMMON",library.find_in("POWERUP.WDB","COMMON").ok_or("missing powerup world")?)?;
        let load=|name:&str|TrackGpu::upload(&Model::load_with_materials(library,name,Some("COMMON"),&bindings.materials).map_err(|e|e.to_string())?);
        let mut bricks=vec![load(&rules.models.white)?];
        for material in ["pbrickP","pbrickM","pbrickS","pbrickT"] {
            let replacement=bindings.materials.get(&material.to_ascii_lowercase()).ok_or_else(||format!("missing original {material}"))?.clone();
            let overrides=HashMap::from([("ptrailm".into(),replacement)]);
            bricks.push(TrackGpu::upload(&Model::load_with_materials(library,&rules.models.pickup,Some("COMMON"),&overrides).map_err(|e|e.to_string())?)?);
        }
        let image=lrformats::bmp::decode(library.find_in(&rules.models.oil,"COMMON").ok_or("missing original oil image")?).map_err(|e|e.to_string())?;
        let mut rgba=image.to_rgba();for (pixel,index) in rgba.chunks_exact_mut(4).zip(&image.indices) {if *index==0 {pixel[3]=0;}}
        let oil=Texture2D::from_rgba8(image.width,image.height,&rgba);
        Ok(Self {bricks,missile:load(&rules.models.missile)?,cannon:load(&rules.models.cannon)?,grapple:load(&rules.models.grapple)?,barrel:load(&rules.models.barrel)?,
            mine:load(&rules.models.mine)?,curse:load(&rules.models.curse)?,beam:load(&rules.models.beam)?,tether:load(&rules.models.tether)?,oil,
            shields:rules.models.shield.iter().map(|name|load(name)).collect::<Result<_,_>>()?,warp:load(&rules.models.warp)?,
            turbos:rules.models.turbo.iter().map(|name|load(name)).collect::<Result<_,_>>()?})
    }
    pub fn draw(&self,state:&Powerups,racers:&[Racer],view:crate::race_view::RaceView) {
        for pickup in &state.pickups {
            if pickup.cooldown_ms>0.0||pickup.held_by.is_some() {continue;}
            self.bricks[pickup.kind as usize].draw_at(Mat4::from_translation(view.native(pickup.source.position))*Mat4::from_rotation_y(state.phase));
        }
        for projectile in &state.weapons.projectiles {
            let position=view.native(projectile.position);let direction=view.native(lrsim::contact::normalized(projectile.velocity));
            let transform=if direction.length_squared()>0.0 {Mat4::from_cols(direction.extend(0.0),Vec3::Y.extend(0.0),direction.cross(Vec3::Y).extend(0.0),position.extend(1.0))} else {Mat4::from_translation(position)};
            let model=match projectile.attack {Attack::Cannon=>&self.cannon,Attack::Grapple=>&self.grapple,Attack::Barrel=>&self.barrel,Attack::Curse=>&self.curse,_=>&self.missile};model.draw_at(transform);
        }
        for zone in &state.weapons.zones {
            let position=view.native(zone.position);
            if zone.attack==Attack::MagneticMine {self.mine.draw_at(Mat4::from_translation(position));}
            else {draw_plane(position+Vec3::Y*0.1,vec2(state.rules.hit_radius*2.0,state.rules.hit_radius*2.0),Some(&self.oil),WHITE);}
        }
        let beam=|model:&TrackGpu,from:[f32;3],to:[f32;3]| {
            let from=view.native(from);let to=view.native(to);let direction=(to-from).normalize_or_zero();
            if direction.length_squared()>0.0 {let up=if direction.dot(Vec3::Y).abs()>0.99 {Vec3::Z} else {Vec3::Y};let side=direction.cross(up).normalize();let up=side.cross(direction);
                model.draw_at(Mat4::from_cols((direction*(to-from).length()/model.radius.max(1.0)/2.0).extend(0.0),up.extend(0.0),side.extend(0.0),((from+to)*0.5).extend(1.0)));}
        };
        for flash in &state.weapons.flashes {if let (Some(owner),Some(target))=(racers.get(flash.owner),racers.get(flash.target)) {beam(&self.beam,owner.position,target.position);}}
        for (racer,inventory) in racers.iter().zip(&state.inventories) {
            let transform=view.car(racer.position,racer.forward,[0.0,0.0,1.0]);
            if inventory.shield_ms>0.0 {self.shields[inventory.shield_tier as usize].draw_at(transform);}
            if inventory.warp_ms>0.0 {self.warp.draw_at(transform);}
            if inventory.turbo_ms>0.0 {self.turbos[inventory.turbo_tier.min(2) as usize].draw_at(transform);}
            if inventory.curse_ms>0.0 {self.curse.draw_at(transform);}
            if let Some(target)=inventory.grapple_target.filter(|_|inventory.grapple_ms>0.0).and_then(|i|racers.get(i)) {beam(&self.tether,racer.position,target.position);}
        }
    }
}
