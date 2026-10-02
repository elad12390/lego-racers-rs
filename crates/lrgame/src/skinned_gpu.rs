//! Repose original rigid-jointed geometry without rebuilding textures each frame.
use macroquad::prelude::*;
use lrformats::model::Model;
use crate::gpu::{TrackGpu,world_position};

struct Binding {points:Vec<(usize,Vec3)>}
pub struct SkinnedGpu {gpu:TrackGpu,bindings:Vec<Binding>}

impl SkinnedGpu {
    pub fn enable_scene_render(&mut self,model:&Model)->Result<(),String> {self.gpu.enable_scene_render(model)}
    pub fn set_scene_surface(&mut self,name:&str,surface:&crate::cinematic_material::Surface) {self.gpu.set_scene_surface(name,surface)}
    pub fn upload(model:&Model,joints:&[Mat4])->Result<Self,String> {
        let gpu=TrackGpu::upload_skinned(model,joints)?;
        let bindings=model.surfaces.iter().flat_map(|surface| {
            surface.triangles.chunks(1400).enumerate().map(|(batch,triangles)|Binding {
                points:triangles.iter().enumerate().flat_map(|(row,t)|t.iter().enumerate().map(move |(corner,index)|(surface.joints[batch*1400+row][corner] as usize,Vec3::from_array(model.mesh.vertices[*index as usize].position)*model.mesh.scale))).collect(),
            }).collect::<Vec<_>>()
        }).collect();
        Ok(Self {gpu,bindings})
    }

    pub fn set_pose(&mut self,joints:&[Mat4])->Result<(),String> {
        for (mesh,binding) in self.gpu.meshes.iter_mut().zip(&self.bindings) {
            for (vertex,(index,point)) in mesh.vertices.iter_mut().zip(&binding.points) {
                let joint=joints.get(*index).ok_or("GDB animation joint absent in SDB")?;
                vertex.position=world_position(joint.transform_point3(*point).to_array(),1.0);
            }
        }
        Ok(())
    }

    pub fn draw_at(&self,transform:Mat4) {self.gpu.draw_at(transform);}
    pub fn tint(&mut self,tint:Color) {self.gpu.tint(tint);}
}
