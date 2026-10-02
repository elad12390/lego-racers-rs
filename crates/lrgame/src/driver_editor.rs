//! Native PC build-driver screen using the original part catalog and meshes.
use crate::{custom_driver::{Build,Data},menu_ui::MenuUi,gpu::TrackGpu,menu_driver::MenuDriver};
use lrformats::library::Library;
use macroquad::prelude::*;

pub async fn run(library:&Library,profile:&crate::profile::Profile,catalog:&crate::game_catalog::Catalog,ui:&MenuUi,capture:Option<&std::path::Path>)->Result<Option<Build>,String> {
    let data=Data::load(library)?;let mut build=profile.custom_driver.clone().unwrap_or_default();data.validate(&build)?;
    let mut row=1usize;let mut rotation=0.0f32;let mut frame=0;let mut rendered=None;let mut previous=None;
    let choices=(0..4).map(|row|data.row(row).iter().enumerate().filter(|(i,part)|profile.driver_part_allowed(part.unlock,catalog)||*i==*slot(&mut build,row)).map(|(i,_)|i).collect::<Vec<_>>()).collect::<Vec<_>>();
    let stage_world=library.find_at("CBSET.WDB","MENUDATA","CB_SET").ok_or("missing original driver stage")?;
    let camera=crate::menu_camera::MenuCamera::load(stage_world)?;
    let stage_origin=crate::gpu::world_position(lrformats::world::instances(stage_world).map_err(|e|e.to_string())?.first().ok_or("driver stage has no original stand")?.position,1.0);
    // The original stand's top is below the minifigure's pelvis origin. Only
    // recenter X/Y; raising the stand to the pelvis hides the entire lower legs.
    let stage_offset=vec3(stage_origin.x,0.0,stage_origin.z);
    let bindings=lrformats::scene::SceneBindings::load(library,"CB_SET",stage_world)?;
    let stand=TrackGpu::upload(&lrformats::model::Model::load_with_materials(library,"STAND",Some("CB_SET"),&bindings.materials).map_err(|e|e.to_string())?)?;
    loop {
        if is_quit_requested()||is_key_pressed(KeyCode::Escape) {return Ok(None);}
        if is_key_pressed(KeyCode::Up) {row=(row+3)%4;}if is_key_pressed(KeyCode::Down) {row=(row+1)%4;}
        let mouse=(Vec2::from(mouse_position())-MenuUi::origin())/MenuUi::scale();
        for r in 0..4 {
            let y=42.0+r as f32*78.0;
            let direction=if (r==row&&is_key_pressed(KeyCode::Left))||(is_mouse_button_pressed(MouseButton::Left)&&Rect::new(15.0,y,32.0,40.0).contains(mouse)) {-1}
                else if (r==row&&is_key_pressed(KeyCode::Right))||(is_mouse_button_pressed(MouseButton::Left)&&Rect::new(248.0,y,32.0,40.0).contains(mouse)) {1} else {0};
            if direction!=0 {row=r;let index=slot(&mut build,r);let current=choices[r].iter().position(|v|v==index).unwrap_or(0);*index=choices[r][(current as isize+direction).rem_euclid(choices[r].len() as isize) as usize];}
        }
        if capture.is_some()&&frame==3 {build=Build {hat:15,face:22,torso:22,legs:11};} // actual unlocked hair / face / torso / red legs
        if previous.as_ref()!=Some(&build) {
            let character=MenuDriver::load(library,&build)?;
            let mut thumbnails=Vec::new();
            for r in 0..4 {let current=*slot(&mut build,r);let position=choices[r].iter().position(|v|*v==current).unwrap_or(0);
                for offset in [-1,0,1] {let i=choices[r][(position as isize+offset).rem_euclid(choices[r].len() as isize) as usize];let model=data.thumbnail(library,r,i)?;thumbnails.push((r,offset,(!model.mesh.vertices.is_empty()).then(||TrackGpu::upload(&model)).transpose()?));}
            }
            rendered=Some((character,thumbnails));previous=Some(build.clone());
        }
        ui.original_background("Build Driver",false);ui.preview_frame(306.0,80.0,306.0,355.0);
        for r in 0..4 {let y=42.0+r as f32*78.0;ui.original_image(if r==row {"arrowls"} else {"arrowlu"},15.0,y);ui.original_image(if r==row {"arrowrs"} else {"arrowru"},248.0,y);}
        if is_mouse_button_down(MouseButton::Left)&&Rect::new(306.0,80.0,306.0,355.0).contains(mouse) {rotation+=mouse_delta_position().x/MenuUi::scale()*0.01;}
        if is_key_down(KeyCode::A) {rotation-=get_frame_time();}if is_key_down(KeyCode::D) {rotation+=get_frame_time();}
        if let Some((character,thumbnails))=&mut rendered {
            character.advance(if capture.is_some() {1.0/60.0}else {get_frame_time().min(0.1)})?;
            for (r,offset,gpu) in thumbnails {if let Some(gpu)=gpu {
                let x=125.0+*offset as f32*70.0;let y=38.0+*r as f32*78.0;
                let target=if *r==3 {vec3(0.0,-0.45,0.0)} else {vec3(0.0,0.7,0.0)};
                crate::racer_preview::camera(Rect::new(x, y,60.0,60.0),target+vec3(4.2,0.8,1.5),target);gpu.draw();set_default_camera();
            }}
            camera.draw(Rect::new(309.0,83.0,300.0,349.0),stage_offset,0.0);
            stand.draw_at(Mat4::from_translation(vec3(0.0,stage_origin.y,0.0)));character.draw_at(Mat4::from_rotation_y(rotation));set_default_camera();
        }
        if ui.action("Mix",35.0,366.0,None) {for r in 0..4 {let count=choices[r].len();*slot(&mut build,r)=choices[r][rand::gen_range(0,count)];}}
        let done=ui.action("Make License",35.0,406.0,Some("txtaror"))||is_key_pressed(KeyCode::Enter);
        if ui.action("Cancel",35.0,446.0,Some("txtx")) {return Ok(None);}
        if let Some(dir)=capture {if frame==2||frame==5 {crate::capture::save_frame(&dir.join(if frame==2 {"driver-default.png"} else {"driver-mixed.png"}),&crate::capture::screen_data())?;}if frame==6 {return Ok(Some(build));}}
        if done {return Ok(Some(build));}next_frame().await;frame+=1;
    }
}
fn slot(build:&mut Build,row:usize)->&mut usize {match row {0=>&mut build.hat,1=>&mut build.face,2=>&mut build.torso,_=>&mut build.legs}}
