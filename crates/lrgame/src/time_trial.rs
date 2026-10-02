//! Original GHB Veronica playback and independently saved personal-best ghost.
//! No ghost joins the race/contact/pickup roster. Native transparency provisional.
use macroquad::prelude::*;
use lrformats::{library::Library,model::Model};
use lrsim::ghost_run::{Pose,Run,Recorder};
use crate::{profile::{Profile,TrialGhost},options::Options,gpu::TrackGpu,driver_animation::DriverAnimation,wheels::Wheels};
struct GhostGpu {body:TrackGpu,wheels:Wheels,driver:DriverAnimation,seat:Mat4,run:Run,pose:Option<Pose>}
impl GhostGpu {
    fn load(library:&Library,ghost:&TrialGhost,tint:Color)->Result<Self,String> {
        ghost.run.validate()?;let catalog=crate::game_catalog::Catalog::load(library)?;
        let car=catalog.cars.iter().find(|c|c.name.eq_ignore_ascii_case(&ghost.car)).ok_or("missing saved ghost car")?;
        let driver=catalog.drivers.iter().find(|d|d.name.eq_ignore_ascii_case(&ghost.driver)).ok_or("missing saved ghost driver")?;
        let chassis=lrformats::cmb::parse(library.find_in("CHASSIS.CMB","COMMON").ok_or("missing original ghost chassis")?).map_err(|e|e.to_string())?.into_iter().find(|c|c.name.eq_ignore_ascii_case(&car.chassis)).ok_or("missing original ghost chassis entry")?;
        let model=if let Some(build)=&ghost.build {if !build.chassis.eq_ignore_ascii_case(&car.chassis) {return Err("ghost build chassis mismatch".into());}crate::brick_model::model(library,&lrsim::brick_build::BuilderData::load(library)?,build)?} else {Model::load(library,&car.models[0],Some("COMMON")).map_err(|e|e.to_string())?};
        let mut body=TrackGpu::upload(&model)?;body.tint(tint);
        let mut wheels=Wheels::load(library,&chassis.wheel_models.first().ok_or("missing original ghost wheels")?.1)?;wheels.tint(tint);
        let driver_model=if let Some(build)=&ghost.driver_build {crate::custom_driver::Data::load(library)?.model(library,build)?} else {Model::load(library,&driver.models[0],Some("COMMON")).map_err(|e|e.to_string())?};
        let mut driver=DriverAnimation::load(library,&driver_model)?;driver.tint(tint);
        Ok(Self {body,wheels,driver,seat:Mat4::from_translation(crate::gpu::world_position(chassis.size,1.0)),run:ghost.run.clone(),pose:None})
    }
    fn advance(&mut self,time:f64,dt:f32)->Result<(),String> {
        let pose=self.run.sample(time);
        let speed=if dt>0.0 {self.pose.as_ref().zip(pose.as_ref()).map_or(0.0,|(a,b)|lrsim::contact::length(lrsim::contact::sub(b.position,a.position))/dt)} else {0.0};
        self.wheels.advance(speed,dt)?;self.driver.advance(speed,1.0,0.0,dt)?;self.pose=pose;Ok(())
    }
    fn draw(&self,view:crate::race_view::RaceView) {
        if let Some(pose)=&self.pose {let b=lrsim::route_motion::quaternion_basis(pose.rotation);let transform=view.car(pose.position,b[..3].try_into().unwrap(),b[6..9].try_into().unwrap());
            self.body.draw_at(transform);self.wheels.draw_at(transform);self.driver.draw_at(transform*self.seat);}
    }
}
pub struct TimeTrial {pub original_time:f64,original:GhostGpu,best:Option<GhostGpu>,recorder:Recorder}
pub fn pose(car:&lrsim::vehicle::Vehicle)->Pose {
    let b=car.basis();let matrix=Mat3::from_cols(Vec3::from_array(b[..3].try_into().unwrap()),Vec3::from_array(b[3..6].try_into().unwrap()),Vec3::from_array(b[6..9].try_into().unwrap()));
    Pose {position:car.position,rotation:Quat::from_mat3(&matrix).conjugate().to_array()}
}
impl TimeTrial {
    pub fn load(library:&Library,options:&Options,car:&lrsim::vehicle::Vehicle)->Result<Self,String> {
        let run=Run::original(lrformats::ghost::Ghost::load(library.find_in("GHOST.GHB",&options.table).ok_or("missing original time-race ghost")?,false)?);
        let original_time=run.total();let original=GhostGpu::load(library,&TrialGhost {car:"VV".into(),driver:"VV".into(),build:None,driver_build:None,run},Color::new(1.0,0.6,0.85,0.55))?;
        let profile=Profile::load(&options.profile.clone().unwrap_or_else(Profile::path))?;
        let best=options.race_name.as_ref().and_then(|key|profile.trial_ghosts.get(key)).map(|ghost|GhostGpu::load(library,ghost,Color::new(0.6,0.9,1.0,0.4))).transpose()?;
        Ok(Self {original_time,original,best,recorder:Recorder::new(pose(car))})
    }
    pub fn reset(&mut self,car:&lrsim::vehicle::Vehicle) {self.recorder=Recorder::new(pose(car));self.original.pose=None;if let Some(best)=&mut self.best {best.pose=None;}}
    pub fn advance(&mut self,time:f64,car:&lrsim::vehicle::Vehicle,dt:f32)->Result<(),String> {
        self.recorder.observe(time,pose(car));self.original.advance(time,dt)?;if let Some(best)=&mut self.best {best.advance(time,dt)?;}Ok(())
    }
    pub fn draw(&self,view:crate::race_view::RaceView) {self.original.draw(view);if let Some(best)=&self.best {best.draw(view);}}
    pub fn draw_times(&self,ui:&crate::menu_ui::MenuUi) {
        ui.text(&format!("VERONICA {:.2}",self.original_time),20.0,65.0,15.0,WHITE);
        if let Some(best)=&self.best {ui.text(&format!("YOUR BEST {:.2}",best.run.total()),20.0,85.0,15.0,WHITE);}
    }
    pub fn finish(self,options:&Options,laps:Vec<f64>)->Result<TrialGhost,String> {Ok(TrialGhost {car:options.car.clone(),driver:options.driver.clone(),build:options.custom_build.clone(),driver_build:options.custom_driver.clone(),run:self.recorder.finish(laps)?})}
}
