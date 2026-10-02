//! Static original WDB cameras, expressed in the runtime's right-handed basis.
use lrformats::tok::{self,Node};
use macroquad::prelude::*;
use crate::{gpu::world_position,menu_ui::MenuUi};

pub struct MenuCamera {pub position:Vec3,pub forward:Vec3,pub up:Vec3,pub fov:f32}
impl MenuCamera {
    pub fn load(bytes:&[u8])->Result<Self,String> {
        let nodes=tok::parse(bytes).map_err(|e|e.to_string())?;
        let list=nodes.windows(3).find_map(|p|match p {[Node::Keyword(0x43),Node::Count(_),Node::Block(b)]=>Some(b),_=>None}).ok_or("original menu camera list missing")?;
        let fields=list.windows(3).find_map(|p|match p {[Node::Keyword(0x43),Node::Str(_),Node::Block(b)]=>Some(b),_=>None}).ok_or("original menu camera missing")?;
        let record=|kind|fields.iter().find_map(|n|match n {Node::Record {kind:k,fields} if *k==kind=>Some(fields),_=>None}).ok_or("menu camera transform missing");
        let position=record(0x17)?.iter().map(|v|v.as_f32().ok_or("invalid camera position")).collect::<Result<Vec<_>,_>>()?;
        let axes=record(0x18)?.iter().map(|v|v.as_f32().ok_or("invalid camera axes")).collect::<Result<Vec<_>,_>>()?;
        if position.len()!=3||axes.len()!=6 {return Err("invalid menu camera transform dimensions".into());}
        let fov=match lrformats::named_records::value(fields,0x47) {Some(Node::Float(v)) if v.is_finite()&&*v>0.0&&*v<180.0=>v.to_radians(),_=>return Err("original menu camera FOV missing".into())};
        Ok(Self {position:world_position(position.try_into().unwrap(),1.0),forward:world_position(axes[..3].try_into().unwrap(),1.0),up:world_position(axes[3..].try_into().unwrap(),1.0),fov})
    }
    pub fn draw(&self,rect:Rect,offset:Vec3,orbit:f32) {
        let rotation=Mat4::from_rotation_y(orbit);let position=rotation.transform_point3(self.position-offset);let forward=rotation.transform_vector3(self.forward);let up=rotation.transform_vector3(self.up);
        let s=MenuUi::scale();let o=MenuUi::origin();
        set_camera(&Camera3D {position,target:position+forward,up,aspect:Some(rect.w/rect.h),fovy:self.fov,z_near:0.05,z_far:800.0,viewport:Some(((o.x+rect.x*s) as i32,(screen_height()-o.y-(rect.y+rect.h)*s) as i32,(rect.w*s) as i32,(rect.h*s) as i32)),..Default::default()});
    }
}
