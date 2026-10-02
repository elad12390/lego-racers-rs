use std::path::PathBuf;
use lrformats::{animation,library::Library};

#[test]
fn original_wheel_forward_reverse_keys_and_clip_ranges_are_preserved() {
    let library=Library::open(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM")).unwrap();
    let animation=animation::parse(library.find_in("BKJMW.ADB","COMMON").unwrap()).unwrap();
    assert_eq!(animation.rotations.len(),14);
    assert_eq!(animation.times,vec![0,5,10,15,20,25,30]);
    assert_eq!(animation.channels.len(),6);
    assert_eq!((animation.clips[0].name.as_str(),animation.clips[0].channel_start,animation.clips[0].duration),("frwrd",0,34));
    assert_eq!((animation.clips[1].name.as_str(),animation.clips[1].channel_start),("rvrse",3));
    assert!(animation.rotation_keys("frwrd",0,0.0).unwrap().is_none());
    let (first,_,fraction)=animation.rotation_keys("frwrd",1,0.0).unwrap().unwrap();
    assert_eq!(fraction,0.0);
    assert_eq!(first,animation.rotations[0]);
    let (reverse,_,_)=animation.rotation_keys("rvrse",1,0.0).unwrap().unwrap();
    assert_eq!(reverse,animation.rotations[7]);
    let (a,b,f)=animation.rotation_keys("frwrd",2,12.5).unwrap().unwrap();
    assert_eq!((a,b,f),(animation.rotations[2],animation.rotations[3],0.5));
    assert_eq!((animation.clips[0].loop_duration,animation.clips[1].loop_duration),(35,31));
    assert_ne!(animation.rotation_keys("frwrd",1,34.0).unwrap(),animation.rotation_keys("frwrd",1,0.0).unwrap());
    assert_eq!(animation.rotation_keys("frwrd",1,35.0).unwrap(),animation.rotation_keys("frwrd",1,0.0).unwrap());
}

#[test]
fn original_driver_static_and_moving_clips_keep_all_six_joint_channels() {
    let library=Library::open(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM")).unwrap();
    let animation=animation::parse(library.find_in("PELVIS.ADB","COMMON").unwrap()).unwrap();
    assert_eq!(animation.positions.len(),11);
    assert_eq!(animation.rotations.len(),206);
    assert_eq!(animation.channels.len(),108);
    assert_eq!(animation.clips.len(),18);
    for clip in &animation.clips {
        assert_eq!(clip.frames_per_second,30.0);
        for joint in 0..6 {
            for time in [0.0,0.5,f32::from(clip.duration),100.0] {
                let keys=animation.sample_rotation(&clip.name,joint,time,false).unwrap();
                let pos=animation.sample_position(&clip.name,joint,time,false).unwrap();
                assert_eq!(keys.is_some(),animation.channels[clip.channel_start+joint].rotation_count>0);
                if let Some(keys)=keys {assert!(keys.0.into_iter().all(f32::is_finite));}
                if let Some(pos)=pos {assert!(pos.into_iter().all(f32::is_finite));}
            }
        }
    }
    let clip=animation.clips.iter().find(|c|c.name=="steer-r").unwrap();
    assert_eq!(clip.duration,0);
    assert_eq!(animation.sample_rotation("steer-r",1,0.0,false).unwrap(),animation.sample_rotation("steer-r",1,100.0,false).unwrap());
    assert!(animation.sample_rotation("nonexistent",0,0.0,false).is_err());
}
