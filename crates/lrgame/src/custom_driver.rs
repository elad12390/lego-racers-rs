//! Original selectable minifigure pieces and material remapping.
//! Menu templates preserve their original joint hierarchy; game bodies use the
//! original reduced GAMEPART/LEG_BOX body and combined head/hat geometry.
use lrformats::{library::Library,driver_parts,part_geometry,gdb::{self,Mesh,Part,Vertex},model::{Model,Surface},mdb::{self,Material},bmp};
use serde::{Serialize,Deserialize};
use std::collections::HashMap;

#[derive(Clone,Debug,PartialEq,Serialize,Deserialize,Default)]
pub struct Build {pub hat:usize,pub face:usize,pub torso:usize,pub legs:usize}
pub struct Data {pub parts:driver_parts::Catalog,menu:part_geometry::Geometry,game:part_geometry::Geometry,materials:HashMap<String,Material>}
impl Data {
    pub fn load(library:&Library)->Result<Self,String> {
        let read=|file:&str,table:&str|library.find_at(file,"MENUDATA",table).ok_or_else(||format!("missing original driver asset {table}/{file}"));
        Ok(Self {parts:driver_parts::Catalog::parse(read("BODYPART.PCB","PARTDB")?)?,menu:part_geometry::Geometry::parse(read("CBBODIES.GCB","MENUPART")?)?,game:part_geometry::Geometry::parse(read("ICB_CHAR.GCB","GAMEPART")?)?,
            materials:mdb::parse(read("BODYPART.MDB","PARTDB")?).map_err(|e|e.to_string())?.into_iter().map(|m|(m.name.to_ascii_lowercase(),m)).collect()})
    }
    pub fn validate(&self,build:&Build)->Result<(),String> {
        for (index,parts) in [(build.hat,&self.parts.hats),(build.face,&self.parts.faces),(build.torso,&self.parts.torsos),(build.legs,&self.parts.legs)] {if index>=parts.len() {return Err("saved driver part index is outside original catalog".into());}}Ok(())
    }
    pub fn row(&self,row:usize)->&[driver_parts::Part] {match row {0=>&self.parts.hats,1=>&self.parts.faces,2=>&self.parts.torsos,_=>&self.parts.legs}}
    pub fn menu_template(&self,build:&Build)->&str {
        match (self.parts.torsos[build.torso].variant,self.parts.legs[build.legs].variant) {(0,0)=>"RR",(0,_)=>"RP",(_,0)=>"HR",(_,_)=>"HP"}
    }
    pub fn original_driver(&self,driver:&lrformats::appearance::Driver)->Result<Build,String> {
        let [hat,face,torso,legs]=driver.parts;let build=Build {hat,face,torso,legs};self.validate(&build)?;Ok(build)
    }
    /// Original full selector body keeps its independent arm/hand/leg joints.
    /// Unlike the static CHESTS thumbnails, no guessed waist translation is
    /// needed: the selected template's own SDB supplies its bind transforms.
    pub fn menu_model(&self,library:&Library,build:&Build)->Result<Model,String> {
        self.validate(build)?;
        let remaps=[("face",format!("{}dflt",self.parts.faces[build.face].name)),("torso",self.parts.torsos[build.torso].name.clone()),("legs",self.parts.legs[build.legs].name.clone())];
        let mut materials=HashMap::new();
        for (key,name) in remaps {let mut material=self.materials.get(&name).ok_or_else(||format!("missing selected body material {name}"))?.clone();material.name=key.into();materials.insert(key.into(),material);}
        let mut model=Model::load_with_materials(library,self.menu_template(build),Some("MENUPART"),&materials).map_err(|e|e.to_string())?;
        // Replace the generic head surface using the menu head and selected
        // menu hat; game combined-head geometry uses a different attachment.
        let mut surfaces=Vec::new();let mut parts=Vec::new();for (surface,part) in model.surfaces.into_iter().zip(model.mesh.parts) {if surface.material.as_deref()!=Some("face") {surfaces.push(surface);parts.push(part);}}
        model.surfaces=surfaces;model.mesh.parts=parts;
        let face=format!("{}dflt",self.parts.faces[build.face].name);
        self.append_part(library,&mut model,&self.menu,"head",Some(("face",&face)),18)?;
        let hat=&self.parts.hats[build.hat].name;if hat!="nohat" {self.append_part(library,&mut model,&self.menu,hat,None,18)?;}Ok(model)
    }
    pub fn model(&self,library:&Library,build:&Build)->Result<Model,String> {
        self.validate(build)?;
        let remaps=[("face",format!("{}dflt",self.parts.faces[build.face].name)),("torso",self.parts.torsos[build.torso].name.clone()),("legs",self.parts.legs[build.legs].name.clone())];
        let mut materials=HashMap::new();for (key,name) in &remaps {let mut material=self.materials.get(name).ok_or_else(||format!("missing selected game material {name}"))?.clone();material.name=(*key).into();materials.insert((*key).into(),material);}
        let mut source=Model::load_with_materials(library,"LEG_BOX",Some("GAMEPART"),&materials).map_err(|e|e.to_string())?;
        let mut surfaces=Vec::new();let mut parts=Vec::new();for (surface,part) in source.surfaces.into_iter().zip(source.mesh.parts) {if surface.material.as_deref()!=Some("face") {surfaces.push(surface);parts.push(part);}}
        source.surfaces=surfaces;source.mesh.parts=parts;
        let head=self.parts.heads[build.hat].as_str();
        let selected=self.game.parts.get(head).ok_or_else(||format!("missing original game head {head}"))?;
        if !selected.surfaces.iter().any(|s|s.material=="face") {self.append_part(library,&mut source,&self.game,"head",Some(("face",&remaps[0].1)),4)?;}
        self.append_part(library,&mut source,&self.game,head,Some(("face",&remaps[0].1)),4)?;
        let menu=crate::skeleton::Skeleton::load(library.find_at("LEG_BOX.SDB","MENUDATA","GAMEPART").ok_or("missing original game body skeleton")?,source.mesh.scale)?.pose(None)?;
        let target=crate::skeleton::Skeleton::load(library.find_in("PELVIS.SDB","COMMON").ok_or("missing game driver skeleton")?,source.mesh.scale)?.pose(None)?;
        let mut model=empty();model.mesh.scale=source.mesh.scale;model.images=source.images;
        for surface in source.surfaces {
            let mut triangles=Vec::new();let mut bindings=Vec::new();
            for (triangle,joints) in surface.triangles.iter().zip(&surface.joints) {
                let mut indices=[0;3];let mut game_joints=[0;3];
                for corner in 0..3 {
                    let joint=joints[corner] as usize;
                    let game_joint=match joint {0..=3=>joint,4=>5,_=>return Err("game body joint outside original hierarchy".into())};
                    let transform=target[game_joint].inverse()*menu[joint];
                    let vertex=&source.mesh.vertices[triangle[corner] as usize];let p=macroquad::prelude::Vec3::from_array(vertex.position)*source.mesh.scale;
                    let position=(transform.transform_point3(p)/model.mesh.scale).to_array();
                    indices[corner]=model.mesh.vertices.len() as u32;game_joints[corner]=game_joint as u16;
                    model.mesh.vertices.push(Vertex {position,..*vertex});
                    model.mesh.normals.push(transform.transform_vector3(macroquad::prelude::Vec3::from_array(source.mesh.normals[triangle[corner] as usize])).normalize_or_zero().to_array());
                }
                triangles.push(indices);bindings.push(game_joints);
            }
            model.mesh.parts.push(Part {texture:0,joint:0,flag:0,vertices:0..model.mesh.vertices.len() as u32,triangles:0..0});
            model.surfaces.push(Surface {triangles,joints:bindings,..surface});
        }
        Ok(model)
    }
    pub fn thumbnail(&self,library:&Library,row:usize,index:usize)->Result<Model,String> {
        let part=self.row(row).get(index).ok_or("driver thumbnail outside catalog")?;let mut model=empty();
        match row {
            0=>{if part.name!="nohat" {self.append_part(library,&mut model,&self.menu,&part.name,None,0)?;}},
            1=>self.append_part(library,&mut model,&self.menu,"head",Some(("face",&format!("{}dflt",part.name))),0)?,
            2=>self.append_gdb(library,&mut model,if part.variant==0 {"CHESTS"} else {"CHEST_HK"},"torso",&part.name,0)?,
            _=>self.append_gdb(library,&mut model,if part.variant==0 {"LEGSS"} else {"LEG-PEGS"},"legs",&part.name,0)?,
        }
        Ok(model)
    }
    fn surface(&self,library:&Library,model:&mut Model,name:&str,triangles:Vec<[u32;3]>,joint:u16)->Result<(),String> {
        let material=self.materials.get(&name.to_ascii_lowercase()).ok_or_else(||format!("missing body material {name}"))?;
        let texture=material.texture.as_ref().map(|t|t.to_ascii_lowercase());
        if let Some(texture)=&texture {if !model.images.contains_key(texture) {
            let bytes=library.find_at(&format!("{texture}.BMP"),"MENUDATA","PARTDB").ok_or_else(||format!("missing driver texture {texture}"))?;
            model.images.insert(texture.clone(),bmp::decode(bytes).map_err(|e|e.to_string())?);
        }}
        model.mesh.parts.push(Part {texture:0,joint,flag:0,vertices:0..model.mesh.vertices.len() as u32,triangles:0..0});
        let joints=vec![[joint;3];triangles.len()];model.surfaces.push(Surface {material:Some(name.into()),color:material.base_color(),texture,triangles,joints,color_key:material.color_key,blend:material.blend});Ok(())
    }
    fn append_gdb(&self,library:&Library,model:&mut Model,name:&str,from:&str,to:&str,joint:u16)->Result<(),String> {
        let mesh=gdb::parse(library.find_at(&format!("{name}.GDB"),"MENUDATA","MENUPART").ok_or("missing original driver mesh")?).map_err(|e|e.to_string())?;
        let draws=mesh.resolved_draws()?;let base=model.mesh.vertices.len() as u32;
        model.mesh.vertices.extend(mesh.vertices.iter().map(|v|Vertex {position:v.position.map(|p|p*mesh.scale/model.mesh.scale),..*v}));model.mesh.normals.extend(mesh.normals);
        for (part,draw) in mesh.parts.iter().zip(draws) {let material=mesh.textures.get(part.texture as usize).ok_or("missing driver mesh material")?;let material=if material==from {to} else {material};
            self.surface(library,model,material,draw.iter().map(|t|t.map(|v|v.0+base)).collect(),joint)?;
        }Ok(())
    }
    fn append_part(&self,library:&Library,model:&mut Model,geometry:&part_geometry::Geometry,name:&str,remap:Option<(&str,&str)>,joint:u16)->Result<(),String> {
        let part=geometry.parts.get(name).ok_or_else(||format!("missing driver geometry {name}"))?;
        for surface in &part.surfaces {
            let mut triangles=Vec::new();
            for t in &surface.triangles {let mut triangle=[0;3];for (target,i) in triangle.iter_mut().zip(t) {let v=&geometry.vertices[*i as usize];*target=model.mesh.vertices.len() as u32;model.mesh.vertices.push(Vertex {position:v.position.map(|x|x*part.scale/model.mesh.scale),uv:v.uv,rgba:[255;4]});model.mesh.normals.push(v.normal);}triangles.push(triangle);}
            let material=remap.filter(|(from,_)|*from==surface.material).map_or(surface.material.as_str(),|(_,to)|to);
            self.surface(library,model,material,triangles,joint)?;
        }Ok(())
    }
}
fn empty()->Model {Model {mesh:Mesh {textures:Vec::new(),scale:1.0/64.0,vertices:Vec::new(),normals:Vec::new(),triangles:Vec::new(),parts:Vec::new()},surfaces:Vec::new(),images:HashMap::new()}}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mixed_original_driver_parts_use_chosen_textures_and_restore_exactly() {
        let library=Library::open(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM")).unwrap();let data=Data::load(&library).unwrap();
        for build in [Build::default(),Build {hat:19,face:13,torso:18,legs:10}] {
            let model=data.model(&library,&build).unwrap();assert_eq!(model.mesh.vertices.len(),model.mesh.normals.len());assert!(model.surfaces.iter().any(|s|s.texture.as_deref()==Some(data.parts.torsos[build.torso].name.as_str())));
            assert_eq!(serde_json::from_slice::<Build>(&serde_json::to_vec(&build).unwrap()).unwrap(),build);
        }
        for row in 0..4 {for index in 0..data.row(row).len() {data.thumbnail(&library,row,index).unwrap();}}
    }
    #[test]
    fn original_preview_driver_textures_select_the_original_body_catalog() {
        let library=Library::open(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM")).unwrap();let data=Data::load(&library).unwrap();
        let drivers=lrformats::appearance::drivers(library.find_in("DRIVERS.DDB","COMMON").unwrap()).unwrap();
        for name in ["GB","VV"] {let driver=drivers.iter().find(|d|d.name==name).unwrap();let build=data.original_driver(driver).unwrap();let model=data.menu_model(&library,&build).unwrap();assert!(model.surfaces.iter().any(|s|s.joints.iter().flatten().any(|j|matches!(j,19..=28))));assert!(model.surfaces.iter().any(|s|s.texture.as_deref()==Some(data.parts.torsos[build.torso].name.as_str())));}
    }
    #[test]
    fn custom_game_driver_keeps_selected_body_assembly_and_independent_arm_bindings() {
        use macroquad::prelude::Vec3;
        let library=Library::open(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM")).unwrap();let data=Data::load(&library).unwrap();let build=Build {hat:15,face:22,torso:22,legs:11};
        let source=Model::load(&library,"LEG_BOX",Some("GAMEPART")).unwrap();let game=data.model(&library,&build).unwrap();
        let menu_pose=crate::skeleton::Skeleton::load(library.find_at("LEG_BOX.SDB","MENUDATA","GAMEPART").unwrap(),source.mesh.scale).unwrap().pose(None).unwrap();
        let game_pose=crate::skeleton::Skeleton::load(library.find_in("PELVIS.SDB","COMMON").unwrap(),game.mesh.scale).unwrap().pose(None).unwrap();
        let mut arms=[false;2];let mut corners=0;
        for (source_surface,game_surface) in source.surfaces.iter().filter(|s|s.material.as_deref()!=Some("face")).zip(&game.surfaces) {for ((s,sj),(g,gj)) in source_surface.triangles.iter().zip(&source_surface.joints).zip(game_surface.triangles.iter().zip(&game_surface.joints)) {for i in 0..3 {
            let expected=menu_pose[sj[i] as usize].transform_point3(Vec3::from_array(source.mesh.vertices[s[i] as usize].position)*source.mesh.scale);
            let actual=game_pose[gj[i] as usize].transform_point3(Vec3::from_array(game.mesh.vertices[g[i] as usize].position)*game.mesh.scale);
            assert!(actual.distance(expected)<0.00001);if gj[i]==2||gj[i]==3 {arms[(gj[i]-2) as usize]=true;}corners+=1;
        }}}
        assert!(corners>100);assert_eq!(arms,[true,true]);
    }
}
