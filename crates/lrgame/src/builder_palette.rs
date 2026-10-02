//! Original builder brick/category banner, geometry color strip and arrow controls.
use crate::{menu_ui::MenuUi,gpu::TrackGpu};
use lrsim::brick_build::{BuilderData,Build,PlacedBrick};
use lrformats::library::Library;
use macroquad::prelude::*;
pub struct Palette {key:String,models:Vec<(usize,TrackGpu)>}
impl Palette {
    pub fn new()->Self {Self {key:String::new(),models:Vec::new()}}
    pub fn draw(&mut self,library:&Library,data:&BuilderData,build:&Build,candidate:&PlacedBrick,color:&mut usize,ui:&MenuUi)->Result<i32,String> {
        let key=format!("{}/{}/{}",build.chassis,candidate.name,*color);
        if self.key!=key {
            self.models.clear();
            for offset in -2..=2 {let index=(*color as isize+offset).rem_euclid(data.rules.colors.len() as isize) as usize;
                let part=PlacedBrick {color:data.rules.colors[index].clone(),x:0,y:0,z:0,rotation:0,..candidate.clone()};
                self.models.push((index,TrackGpu::upload(&crate::brick_model::candidate(library,data,build,&part)?)?));
            }self.key=key;
        }
        ui.original_image("bricks",338.0,10.0);
        let step=if ui.icon_button("arrowlu",308.0,19.0) {-1} else if ui.icon_button("arrowru",492.0,19.0) {1} else {0};
        let mouse=(Vec2::from(mouse_position())-MenuUi::origin())/MenuUi::scale();
        for (i,(index,gpu)) in self.models.iter().enumerate() {
            let rect=Rect::new(231.0+i as f32*70.0,58.0,66.0,60.0);
            if i==2 {ui.preview_frame(rect.x,rect.y,rect.w,rect.h);}
            let radius=gpu.radius.max(0.8)*2.8;crate::racer_preview::camera(rect,gpu.center+vec3(radius,radius*0.8,radius),gpu.center);gpu.draw();set_default_camera();
            if rect.contains(mouse)&&is_mouse_button_pressed(MouseButton::Left) {*color=*index;}
        }
        Ok(step)
    }
}
