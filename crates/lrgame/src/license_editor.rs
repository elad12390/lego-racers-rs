//! Original PC license artwork, native name entry and actual framebuffer snapshot.
use crate::{profile::{Profile,Photo},menu_ui::MenuUi,custom_driver::Data,menu_driver::MenuDriver};
use lrformats::library::Library;
use macroquad::prelude::*;
pub async fn run(library:&Library,profile:&mut Profile,ui:&MenuUi,capture:Option<&std::path::Path>)->Result<bool,String> {
    let data=Data::load(library)?;
    let build=if let Some(build)=&profile.custom_driver {build.clone()} else {
        let catalog=crate::game_catalog::Catalog::load(library)?;let driver=catalog.drivers.iter().find(|d|d.name.eq_ignore_ascii_case(&profile.driver)).ok_or("missing license driver")?;
        data.original_driver(driver)?
    };
    let mut driver=MenuDriver::load(library,&build)?;
    let mut name=profile.name.clone();let mut photo=profile.license_photo.clone();let mut texture=photo.as_ref().map(|p|Texture2D::from_rgba8(p.width,p.height,&p.rgba));let mut frame=0;
    loop {
        if is_quit_requested()||is_key_pressed(KeyCode::Escape) {return Ok(false);}
        while let Some(c)=get_char_pressed() {if (c.is_ascii_alphanumeric()||c==' ')&&name.len()<16 {name.push(c.to_ascii_uppercase());}}
        if is_key_pressed(KeyCode::Backspace) {name.pop();}
        if capture.is_some()&&frame==1 {name="NATIVE RACER".into();}
        ui.original_background("Make License",false);ui.original_image("license",104.0,68.0);
        ui.text(&name,121.0,218.0,24.0,WHITE);
        let rect=Rect::new(375.0,82.0,143.0,139.0);let s=MenuUi::scale();let o=MenuUi::origin();
        if let Some(texture)=&texture {draw_texture_ex(texture,o.x+rect.x*s,o.y+rect.y*s,WHITE,DrawTextureParams {dest_size:Some(vec2(rect.w*s,rect.h*s)),..Default::default()});}
        else {
            driver.advance(if capture.is_some() {1.0/60.0}else {get_frame_time().min(0.1)})?;
            ui.preview_frame(rect.x,rect.y,rect.w,rect.h);
            crate::racer_preview::camera(rect,vec3(4.1,2.75,0.0),vec3(0.0,2.75,0.0));driver.draw_at(Mat4::IDENTITY);set_default_camera();
        }
        let snapshot=ui.action("Snapshot",35.0,365.0,None)||(capture.is_some()&&frame==2)||is_key_pressed(KeyCode::F5);
        let build_car=ui.action("Build Car",35.0,405.0,Some("txtaror"))||is_key_pressed(KeyCode::Enter);
        if ui.action("Build Driver",35.0,445.0,Some("txtarol")) {return Ok(false);}
        if snapshot {let image=crate::capture::screen_data();let width=143u16;let height=139u16;let mut rgba=Vec::with_capacity(width as usize*height as usize*4);
            for y in 0..height {for x in 0..width {let sx=(o.x+(rect.x+x as f32)*s).floor() as usize;let sy=(o.y+(rect.y+y as f32)*s).floor() as usize;let start=((image.height as usize-1-sy)*image.width as usize+sx)*4;rgba.extend_from_slice(image.bytes.get(start..start+4).ok_or("license snapshot outside framebuffer")?);}}
            photo=Some(Photo {width,height,rgba});texture=photo.as_ref().map(|p|Texture2D::from_rgba8(p.width,p.height,&p.rgba));
        }
        if let Some(dir)=capture {if frame==4 {crate::capture::save_frame(&dir.join("license.png"),&crate::capture::screen_data())?;}}
        if build_car||(capture.is_some()&&frame==5) {if name.trim().is_empty() {name="RACER".into();}profile.name=name;profile.license_photo=photo;return Ok(true);}
        next_frame().await;frame+=1;
    }
}
