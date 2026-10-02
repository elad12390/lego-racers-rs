//! Original CDB/WDB/SDB/ADB scenes in native menu viewports.
//! Original geometry/camera/material timelines; CEB overlays/audio dispatch
//! separately. Particle emitters and nonordinary material paths remain open.
use lrformats::{library::Library,cinematic::{Timeline,Object},model::Model,scene::SceneBindings,animation::{self,Animation},tok::{self,Node}};
use crate::{gpu::{TrackGpu,world_position},skeleton::Skeleton,skinned_gpu::SkinnedGpu,menu_ui::MenuUi};
use macroquad::prelude::*;

struct Animated {gpu:Option<SkinnedGpu>,skeleton:Skeleton,animation:Animation}
enum Geometry {Static(TrackGpu),Animated(Animated),Sprite {texture:Texture2D,width:f32,height:f32}}
struct Entry {track:Object,geometry:Geometry,materials:Option<crate::cinematic_material::Player>}
struct Camera {index:Option<usize>,joint:usize,bind:Mat4,fov:f32,start:u32,duration:u32}
pub struct Scene {entries:Vec<Entry>,cameras:Vec<Camera>,pub duration:f32,pub fps:f32,pub events:Vec<lrformats::cinematic::Event>,pub material_gaps:Vec<String>,material:Material}

fn original_transform(object:&Object)->Mat4 {
    let f=Vec3::from_array(object.forward);let u=Vec3::from_array(object.up);
    Mat4::from_cols(f.extend(0.0),u.cross(f).extend(0.0),u.extend(0.0),Vec3::from_array(object.position).extend(1.0))
}
fn native_transform(matrix:Mat4)->Mat4 {
    let convert=|v:Vec4|world_position(v.truncate().to_array(),1.0).extend(v.w);
    // Original columns X/Y/Z -> native X/Y/Z (Z up; Y flipped).
    Mat4::from_cols(convert(matrix.x_axis),convert(matrix.z_axis),-convert(matrix.y_axis),convert(matrix.w_axis))
}
impl Scene {
    pub fn load(library:&Library,table:&str,cdb:&str)->Result<Self,String> {
        Self::load_named(library,table,cdb,None,None)
    }
    pub fn load_named(library:&Library,table:&str,cdb:&str,name:Option<&str>,driver:Option<&crate::custom_driver::Build>)->Result<Self,String> {
        Self::load_custom(library,table,cdb,name,driver,None)
    }
    pub fn load_custom(library:&Library,table:&str,cdb:&str,name:Option<&str>,driver:Option<&crate::custom_driver::Build>,car:Option<&Model>)->Result<Self,String> {
        let read=|name:&str|library.find_at(name,"MENUDATA",table).ok_or_else(||format!("missing {table}/{name}"));
        let timelines=Timeline::parse(read(&format!("{cdb}.CDB"))?)?;
        let timeline=if let Some(name)=name {timelines.into_iter().find(|t|t.name.eq_ignore_ascii_case(name)).ok_or("named CDB scene missing")?}else {if timelines.len()!=1 {return Err("menu preview requires one named CDB timeline".into());}timelines.into_iter().next().unwrap()};
        let wdb=read(&format!("{cdb}.WDB"))?;
        let mut bindings=SceneBindings::load(library,table,wdb)?;
        let worlds=timeline.worlds.iter().map(|name|read(&format!("{name}.WDB")).and_then(|bytes|tok::parse(bytes).map_err(|e|e.to_string()))).collect::<Result<Vec<_>,_>>()?;
        for name in &timeline.worlds {let bytes=read(&format!("{name}.WDB"))?;if !bytes.is_empty() {bindings.materials.extend(SceneBindings::load(library,table,bytes)?.materials);}}
        let nodes=tok::parse(wdb).map_err(|e|e.to_string())?;
        let camera_fields=|name:&str|nodes.windows(3).find_map(|v|if let [Node::Keyword(0x43),Node::Count(_),Node::Block(b)]=v {b.windows(3).find_map(|v|if let [Node::Keyword(0x43),Node::Str(n),Node::Block(f)]=v {n.eq_ignore_ascii_case(name).then_some(f.as_slice())}else {None})}else {None}).ok_or("missing WDB camera");
        let mut entries=Vec::new();
        for object in timeline.objects {
            let mut materials=None;
            let is_camera=camera_fields(&object.name).is_ok();
            let geometry=if let Some((world,index))=object.sprite {
                let sprite_world=worlds.get(world).ok_or("CDB sprite world outside declared WDB list")?;
                let sprites=sprite_world.windows(3).find_map(|v|if let [Node::Keyword(0x37),Node::Count(_),Node::Block(b)]=v {Some(b)}else {None}).ok_or("missing WDB sprites")?;
                let fields=sprites.windows(2).filter_map(|v|if let [Node::Keyword(0x37),Node::Block(b)]=v {Some(b)}else {None}).nth(index).ok_or("sprite index outside WDB")?;
                let image=lrformats::named_records::string(fields,0x39)?;
                let float=|key|match lrformats::named_records::value(fields,key) {Some(Node::Float(f))=>Ok(*f),_=>Err("missing WDB sprite size")};
                let bitmap=lrformats::bmp::decode(read(&format!("{image}.BMP"))?).map_err(|e|e.to_string())?;
                let mut rgba=bitmap.to_rgba();for p in rgba.chunks_exact_mut(4) {if p[..3]==[0,0,0] {p[3]=0;}}
                let texture=Texture2D::from_rgba8(bitmap.width,bitmap.height,&rgba);texture.set_filter(FilterMode::Nearest);
                Geometry::Sprite {texture,width:float(0x3a)?,height:float(0x3b)?}
            }else if object.animated {
                let model=if is_camera {None}else if object.name=="guy1"&&driver.is_some() {Some(crate::custom_driver::Data::load(library)?.menu_model(library,driver.unwrap())?)}else {Some(Model::load_with_materials(library,&object.name,Some(table),&bindings.materials).map_err(|e|e.to_string())?)};
                let skeleton=Skeleton::load(read(&format!("{}.SDB",object.name))?,model.as_ref().map_or(1.0,|m|m.mesh.scale))?;
                let animation=animation::parse(read(&format!("{}.ADB",object.name))?)?;
                let clip=animation.clips.get(object.clip).ok_or_else(||format!("{table}/{} CDB clip {} outside {} original animations",object.name,object.clip,animation.clips.len()))?;
                let pose=skeleton.pose(Some((&animation,&clip.name,0.0)))?;
                if let Some(model)=&model {materials=Some(crate::cinematic_material::Player::load(library,table,&timeline.worlds,model,&object,&bindings.materials,if object.name.eq_ignore_ascii_case("guy1") {driver}else {None})?);}
                let gpu=model.as_ref().map(|m| {let mut gpu=SkinnedGpu::upload(m,&pose)?;gpu.enable_scene_render(m)?;Ok::<_,String>(gpu)}).transpose()?;
                Geometry::Animated(Animated {gpu,skeleton,animation})
            }else {
                let model=if object.name.eq_ignore_ascii_case("carbody")&&car.is_some() {None}else {Some(Model::load_with_materials(library,&object.name,Some(table),&bindings.materials).map_err(|e|e.to_string())?)};
                let model=model.as_ref().or(car).ok_or("missing cinematic model")?;let mut gpu=TrackGpu::upload(model)?;gpu.enable_scene_render(model)?;Geometry::Static(gpu)
            };
            if matches!(geometry,Geometry::Static(_))&&!object.materials.is_empty() {
                let model=if object.name.eq_ignore_ascii_case("carbody")&&car.is_some() {None}else {Some(Model::load_with_materials(library,&object.name,Some(table),&bindings.materials).map_err(|e|e.to_string())?)};
                materials=Some(crate::cinematic_material::Player::load(library,table,&timeline.worlds,model.as_ref().or(car).ok_or("missing scene model")?,&object,&bindings.materials,None)?);
            }
            entries.push(Entry {track:object,geometry,materials});
        }
        let cameras=timeline.cameras.into_iter().map(|c| {
            let fields=camera_fields(&c.name)?;let mut position=Vec3::ZERO;let mut forward=Vec3::X;let mut up=Vec3::Z;
            for node in fields {if let Node::Record {kind,fields}=node {let v=fields.iter().filter_map(|v|v.as_f32()).collect::<Vec<_>>();match (*kind,v.as_slice()) {(0x17,[x,y,z])=>position=vec3(*x,*y,*z),(0x18,[x,y,z,a,b,c])=>{forward=vec3(*x,*y,*z);up=vec3(*a,*b,*c);},_=>{},}}}
            let fov=match lrformats::named_records::value(fields,0x47) {Some(Node::Float(f))=>f.to_radians(),_=>return Err("missing WDB camera FOV".into())};
            // WDB camera 0x2f binds an animated object AND a skeleton joint.
            // Award cameras are children of their orbit dummy, not the root.
            let joint=fields.windows(3).find_map(|v|if let [Node::Keyword(0x2f),Node::Int(_),Node::Int(joint)]=v {Some(*joint)}else {None}).unwrap_or(0);
            let joint=usize::try_from(joint).map_err(|_|"negative cinematic camera joint")?;
            let index=entries.iter().position(|e|e.track.name.eq_ignore_ascii_case(&c.name));
            if let Some(index)=index {if let Geometry::Animated(camera)=&entries[index].geometry {if joint>=camera.skeleton.pose(None)?.len() {return Err("cinematic camera joint outside original rig".into());}}}
            Ok(Camera {index,joint,bind:Mat4::from_cols(forward.extend(0.0),up.cross(forward).extend(0.0),up.extend(0.0),position.extend(1.0)),fov,start:c.start,duration:c.duration})
        }).collect::<Result<Vec<_>,String>>()?;
        let mut material_gaps=bindings.unavailable_texture_catalogs;material_gaps.extend(entries.iter().filter_map(|e|e.materials.as_ref()).flat_map(|m|m.missing_expressions.clone()));
        Ok(Self {entries,cameras,duration:timeline.duration as f32/timeline.fps,fps:timeline.fps,events:timeline.events,material_gaps,material:crate::scene_material::load()?})
    }
    pub fn draw(&mut self,rect:Rect,seconds:f32)->Result<(),String> {
        let frame=seconds.rem_euclid(self.duration)*self.fps;
            let selected=self.cameras.iter().find(|c|frame>=c.start as f32&&frame<c.start.saturating_add(c.duration) as f32).or_else(||self.cameras.iter().filter(|c|frame>=c.start as f32).max_by_key(|c|c.start)).ok_or("CDB frame outside camera tracks")?;
        let matrix=if let Some(index)=selected.index {let entry=&self.entries[index];let Geometry::Animated(camera)=&entry.geometry else {return Err("nonanimated cinematic camera".into());};let clip=&camera.animation.clips[entry.track.clip];let ticks=(frame-entry.track.start as f32)*clip.frames_per_second/self.fps;let pose=camera.skeleton.pose(Some((&camera.animation,&clip.name,ticks)))?;original_transform(&entry.track)*pose[selected.joint]}else {selected.bind};
        let position=world_position(matrix.transform_point3(Vec3::ZERO).to_array(),1.0);
        let forward=world_position(matrix.transform_vector3(Vec3::X).to_array(),1.0);let up=world_position(matrix.transform_vector3(Vec3::Z).to_array(),1.0);
        let scale=MenuUi::scale();let origin=MenuUi::origin();
        set_camera(&Camera3D {position,target:position+forward,up,aspect:Some(rect.w/rect.h),fovy:selected.fov,viewport:Some(((origin.x+rect.x*scale) as i32,(screen_height()-origin.y-(rect.y+rect.h)*scale) as i32,(rect.w*scale) as i32,(rect.h*scale) as i32)),z_near:0.05,z_far:1000.0,..Default::default()});
        gl_use_material(&self.material);
        for entry in &mut self.entries {
            if frame<entry.track.start as f32||frame>=entry.track.start.saturating_add(entry.track.duration) as f32 {continue;}
            let matrix=native_transform(original_transform(&entry.track));
            if let Some(materials)=&mut entry.materials {materials.advance((frame-entry.track.start as f32)/self.fps,|name,surface|match &mut entry.geometry {Geometry::Static(gpu)=>gpu.set_scene_surface(name,surface),Geometry::Animated(animated)=>if let Some(gpu)=&mut animated.gpu {gpu.set_scene_surface(name,surface)},_=>{}})?;}
            match &mut entry.geometry {
                Geometry::Static(gpu)=>gpu.draw_at(matrix),
                Geometry::Sprite {texture,width,height}=> {
                    gl_use_material(&self.material);
                    let center=world_position(entry.track.position,1.0);let right=forward.cross(up).normalize()*(*width/2.0);let vertical=up.normalize()*(*height/2.0);
                    let points=[center-right-vertical,center+right-vertical,center+right+vertical,center-right+vertical];
                    let vertices=points.into_iter().zip([[0.0,1.0],[1.0,1.0],[1.0,0.0],[0.0,0.0]]).map(|(p,uv)|Vertex::new(p.x,p.y,p.z,uv[0],uv[1],WHITE)).collect();draw_mesh(&Mesh {vertices,indices:vec![0,1,2,0,2,3],texture:Some(texture.clone())});
                },
                Geometry::Animated(animated)=>if let Some(gpu)=&mut animated.gpu {
                    let clip=&animated.animation.clips[entry.track.clip];let ticks=(frame-entry.track.start as f32)*clip.frames_per_second/self.fps;
                    gpu.set_pose(&animated.skeleton.pose(Some((&animated.animation,&clip.name,ticks)))?)?;gpu.draw_at(matrix);
                },
            }
        }
        gl_use_default_material();set_default_camera();Ok(())
    }
}
