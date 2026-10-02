//! Original wheel geometry/SDB/ADB with original speed-based playback rate.
use lrformats::{animation::{self,Animation},library::Library,model::Model};
use macroquad::prelude::*;
use crate::{skeleton::Skeleton,skinned_gpu::SkinnedGpu};

pub struct Wheels {gpu:SkinnedGpu,skeleton:Skeleton,animation:Animation,playback:lrsim::wheel_playback::WheelPlayback}

impl Wheels {
    pub fn load(library:&Library,name:&str)->Result<Self,String> {
        let read=|ext|library.find_in(&format!("{name}.{ext}"),"COMMON").ok_or_else(||format!("missing original wheel{name}.{ext}"));
        let model=Model::load(library,name,Some("COMMON")).map_err(|e|e.to_string())?;
        let skeleton=Skeleton::load(read("SDB")?,model.mesh.scale)?;
        let animation=animation::parse(read("ADB")?)?;
        let playback=lrsim::wheel_playback::WheelPlayback::default();
        let gpu=SkinnedGpu::upload(&model,&skeleton.pose(Some((&animation,playback.clip(),0.0)))?)?;
        Ok(Self {gpu,skeleton,animation,playback})
    }

    pub fn advance(&mut self,speed:f32,dt:f32)->Result<(),String> {
        let period=|name|self.animation.clips.iter().find(|c|c.name==name).map(|c|c.loop_duration).ok_or("original wheel clip missing");
        self.playback.advance(speed,dt,period("frwrd")?,period("rvrse")?);
        self.gpu.set_pose(&self.skeleton.pose(Some((&self.animation,self.playback.clip(),self.playback.time)))?)
    }

    pub fn reset(&mut self)->Result<(),String> {
        self.playback=lrsim::wheel_playback::WheelPlayback::default();
        self.gpu.set_pose(&self.skeleton.pose(Some((&self.animation,self.playback.clip(),0.0)))?)
    }

    pub fn draw_at(&self,transform:Mat4) {self.gpu.draw_at(transform);}
    pub fn tint(&mut self,tint:Color) {self.gpu.tint(tint);}
}
