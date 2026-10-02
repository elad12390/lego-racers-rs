use lrsim::brick_build::{BuilderData,PlacedBrick};
use lrformats::library::Library;
#[test]
fn a_saved_car_can_be_built_reopened_and_edited_using_original_parts() {
    let library=Library::open(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM")).unwrap();
    let data=BuilderData::load(&library).unwrap();let mut build=data.default_build();
    let heights=data.heights(&build).unwrap();let ((x,y),z)=heights.iter().find(|((x,y),z)|*x<9&&*y<5&&**z>0&&**z<16).unwrap();
    let part=PlacedBrick {name:"l300300".into(),color:"red".into(),x:*x,y:*y,z:*z,rotation:0};
    // Find a real attachable footprint, not a mocked placement surface.
    let mut placed=false;
    for x in 0..9 {for y in 0..5 {for z in 0..16 {let candidate=PlacedBrick {x,y,z,..part.clone()};if data.add(&mut build,candidate).is_ok() {placed=true;break;}}if placed {break;}}if placed {break;}}
    assert!(placed);let saved=serde_json::to_vec(&build).unwrap();let restored=serde_json::from_slice(&saved).unwrap();assert_eq!(build,restored);data.validate(&restored).unwrap();
    let duplicate=build.bricks[0].clone();assert!(data.add(&mut build,duplicate).is_err());
    build.bricks.pop();assert!(build.bricks.is_empty());data.validate(&build).unwrap();
}
