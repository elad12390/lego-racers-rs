//! Preloaded original MAB per-model material changes, including selected faces.
use std::collections::HashMap;
use macroquad::prelude::*;
use lrformats::{library::Library,model::Model,cinematic::Object,material_animation::Animation,mdb::Material};
pub struct Surface {pub texture:Option<Texture2D>,pub pipeline:macroquad::material::Material,pub color:[u8;4]}
struct Track {animation:Animation,channel:usize,target:String,last:Option<String>,surfaces:HashMap<String,Surface>}
pub struct Player {tracks:Vec<Track>,pub missing_expressions:Vec<String>}
impl Player {
    pub fn load(library:&Library,table:&str,worlds:&[String],model:&Model,object:&Object,materials:&HashMap<String,Material>,driver:Option<&crate::custom_driver::Build>)->Result<Self,String> {
        let parts=driver.map(|_|crate::custom_driver::Data::load(library)).transpose()?;
        let body_materials=if driver.is_some() {lrformats::mdb::parse(library.find_at("BODYPART.MDB","MENUDATA","PARTDB").ok_or("missing selected face materials")?).map_err(|e|e.to_string())?.into_iter().map(|m|(m.name.to_ascii_lowercase(),m)).collect::<HashMap<_,_>>()}else {HashMap::new()};
        let mut tracks=Vec::new();let mut missing_expressions=Vec::new();
        for reference in &object.materials {
            let animation=lrformats::material_animation::load_track(library,table,worlds,reference)?;
            let channel=animation.channels.get(reference.channel).ok_or("CDB channel outside MAB resource")?;
            // MinifigPreviewScreen replaces guy1 and rewrites each expression
            // channel for the selected face (00475fd0), not the replacement
            // model's unrelated source material-table indices.
            let face=driver.zip(parts.as_ref());
            let target=if let Some((driver,parts))=face {format!("{}dflt",parts.parts.faces[driver.face].name)}else {model.mesh.textures.get(reference.slot).ok_or_else(||format!("{table}/{} CDB animated material slot {} outside GDB {:?}",object.name,reference.slot,model.mesh.textures))?.clone()};
            if !model.surfaces.iter().any(|s|s.material.as_deref().is_some_and(|name|name.eq_ignore_ascii_case(&target))) {missing_expressions.push(format!("{table}/{} animated slot {} ({target}) has no rendered surfaces",object.name,reference.slot));}
            let mut surfaces=HashMap::new();
            for key in &animation.keys[channel.start..channel.start+channel.count] {if surfaces.contains_key(&key.name) {continue;}
                let selected=face.map(|(driver,parts)|format!("{}{}",parts.parts.faces[driver.face].name,key.name));
                let mut material=if let Some(selected)=&selected {body_materials.get(selected).or_else(||body_materials.get(&target))}else {materials.get(&key.name.to_ascii_lowercase())}.ok_or_else(||format!("{table}/{} slot {} target {target}: missing animated original material {} (selected {selected:?})",object.name,reference.slot,key.name))?.clone();
                if let Some(selected)=&selected {if !body_materials.contains_key(selected) {
                    if library.find(&format!("{selected}.BMP"),Some("PARTDB")).is_some() {material.texture=Some(selected.clone());}
                    else {missing_expressions.push(format!("{table}/{} selected {selected}.BMP absent; retained original default face",object.name));}
                }}
                let texture=material.texture.as_ref().map(|name| {
                    let bytes=library.find_at(&format!("{name}.BMP"),"MENUDATA",table).or_else(||library.find_at(&format!("{name}.BMP"),"MENUDATA","PARTDB")).or_else(||face.and_then(|_|library.find(&format!("{name}.BMP"),Some("PARTDB"))));
                    let image=if let Some(bytes)=bytes {lrformats::bmp::decode(bytes).map_err(|e|e.to_string())?}else {lrformats::tga::decode(library.find_at(&format!("{name}.TGA"),"MENUDATA",table).ok_or("missing original animated texture")?)?};
                    let mut rgba=image.to_rgba();if let Some(key)=material.color_key {for pixel in rgba.chunks_exact_mut(4) {if pixel[..3]==key {pixel[3]=0;}}}
                    let texture=Texture2D::from_rgba8(image.width,image.height,&rgba);texture.set_filter(FilterMode::Nearest);Ok::<_,String>(texture)
                }).transpose()?;
                let color=if texture.is_some() {[255;4]}else {material.base_color()};let pipeline=crate::scene_material::load_blend(material.blend)?;
                surfaces.insert(key.name.clone(),Surface {texture,pipeline,color});
            }
            tracks.push(Track {animation,channel:reference.channel,target,last:None,surfaces});
        }Ok(Self {tracks,missing_expressions})
    }
    pub fn advance(&mut self,seconds:f32,mut update:impl FnMut(&str,&Surface))->Result<(),String> {
        for track in &mut self.tracks {let name=track.animation.sample(track.channel,seconds)?;if track.last.as_deref()==Some(name) {continue;}let surface=track.surfaces.get(name).ok_or("missing preloaded MAB key")?;update(&track.target,surface);track.last=Some(name.into());}Ok(())
    }
}
