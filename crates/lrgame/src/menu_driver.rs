//! Original selector minifigure template and shared CBANIM idle animation.
use crate::{custom_driver::{Build,Data},skinned_gpu::SkinnedGpu,skeleton::Skeleton};
use lrformats::{library::Library,animation::{self,Animation}};
use macroquad::prelude::Mat4;
pub struct MenuDriver {gpu:SkinnedGpu,skeleton:Skeleton,animation:Animation,time:f32}
impl MenuDriver {
    pub fn load(library:&Library,build:&Build)->Result<Self,String> {
        let data=Data::load(library)?;let model=data.menu_model(library,build)?;let name=data.menu_template(build);
        let read=|ext|library.find_at(&format!("{name}.{ext}"),"MENUDATA","MENUPART").ok_or("missing original selector animation/rig");
        let skeleton=Skeleton::load(read("SDB")?,model.mesh.scale)?;
        // RR/HR/RP/HP a0 is an empty export clip, not the builder animation.
        // CarBuilderAnimation loads the shared 29-channel CBANIM table.
        let animation=animation::parse(library.find_in("CBANIM.ADB","MENUDATA").ok_or("missing original builder animation")?)?;
        let clip=animation.clips.iter().find(|c|c.name=="breath1").ok_or("missing original breathing clip")?;
        let gpu=SkinnedGpu::upload(&model,&skeleton.pose(Some((&animation,&clip.name,0.0)))?)?;
        Ok(Self {gpu,skeleton,animation,time:0.0})
    }
    pub fn advance(&mut self,seconds:f32)->Result<(),String> {
        let clip=self.animation.clips.iter().find(|c|c.name=="breath1").ok_or("missing original breathing clip")?;self.time+=seconds.max(0.0)*clip.frames_per_second;
        self.gpu.set_pose(&self.skeleton.pose(Some((&self.animation,&clip.name,self.time)))?)
    }
    pub fn draw_at(&self,transform:Mat4) {self.gpu.draw_at(transform);}
}
