//! Original SDB hierarchy and local bind transforms, optionally sampled from ADB.
use lrformats::{animation::Animation,tok::{self,Node}};
use macroquad::prelude::*;

struct Joint {name:String,parent:Option<usize>,position:Vec3,rotation:Quat}
struct JointPose {position:Vec3,rotation:Quat}
pub struct Skeleton {joints:Vec<Joint>,scale:f32}

impl Skeleton {
    pub fn load(data:&[u8],model_scale:f32)->Result<Self,String> {
        let nodes=tok::parse(data).map_err(|e|e.to_string())?;
        let body=nodes.iter().find_map(|n|if let Node::Block(b)=n {Some(b)} else {None}).ok_or("SDB joint list missing")?;
        let mut joints:Vec<Joint>=Vec::new();
        for triple in body.windows(3) {
            let [Node::Keyword(0x27),Node::Str(name),Node::Block(fields)]=triple else {continue;};
            let mut position=Vec3::ZERO;let mut rotation=Quat::IDENTITY;let mut parent=None;
            for node in fields {
                if let Node::Record {kind,fields}=node {
                    let values:Vec<_>=fields.iter().filter_map(|v|v.as_f32()).collect();
                    match (*kind,values.as_slice()) {
                        (0x17,[x,y,z])=>position=vec3(*x,*y,*z)*model_scale,
                        //00449340 writes engine basis columns with the opposite
                        // quaternion convention to glam. Conjugate, do not
                        // animate the wheel axle itself around the car.
                        (0x18,[x,y,z,w])=>rotation=Quat::from_xyzw(*x,*y,*z,*w).normalize().conjugate(),
                        _=>{},
                    }
                }
            }
            for pair in fields.windows(2) {
                if let [Node::Keyword(0x2a),Node::Str(p)]=pair {
                    parent=Some(joints.iter().position(|j|&j.name==p).ok_or_else(||format!("SDB parent{p}must precede{name}"))?);
                }
            }
            if !position.is_finite() || !rotation.is_finite() || joints.iter().any(|j|j.name==*name) {return Err("SDB invalid or duplicate joint".into());}
            joints.push(Joint {name:name.clone(),parent,position,rotation});
        }
        if joints.is_empty() {return Err("SDB empty skeleton".into());}
        Ok(Self {joints,scale:model_scale})
    }

    pub fn pose(&self,animation:Option<(&Animation,&str,f32)>)->Result<Vec<Mat4>,String> {
        self.pose_clip(animation,true)
    }

    pub fn pose_clip(&self,animation:Option<(&Animation,&str,f32)>,looped:bool)->Result<Vec<Mat4>,String> {
        Ok(self.compose(self.sample(animation,looped)?))
    }

    pub fn blend_clips(&self,animation:&Animation,current:(&str,f32),next:(&str,f32),fraction:f32)->Result<Vec<Mat4>,String> {
        let a=self.sample(Some((animation,current.0,current.1)),false)?;
        let b=self.sample(Some((animation,next.0,next.1)),false)?;
        let fraction=fraction.clamp(0.0,1.0);
        let pose=a.into_iter().zip(b).map(|(a,b)| {
            let mut rotation=b.rotation;
            if a.rotation.dot(rotation)<=0.0 {rotation=-rotation;}
            JointPose {position:a.position.lerp(b.position,fraction),rotation:(a.rotation*(1.0-fraction)+rotation*fraction).normalize()}
        }).collect();
        Ok(self.compose(pose))
    }

    fn sample(&self,animation:Option<(&Animation,&str,f32)>,looped:bool)->Result<Vec<JointPose>,String> {
        let mut pose=Vec::new();
        for (index,joint) in self.joints.iter().enumerate() {
            let mut rotation=joint.rotation;
            let mut position=joint.position;
            if let Some((animation,clip,time))=animation {
                if let Some((a,b,fraction))=animation.sample_rotation(clip,index,time,looped)? {
                    let a=Quat::from_array(a);let mut b=Quat::from_array(b);
                    if a.dot(b)<=0.0 {b=-b;}
                    rotation=(a*(1.0-fraction)+b*fraction).normalize().conjugate();
                }
                if let Some(p)=animation.sample_position(clip,index,time,looped)? {
                    position=Vec3::from_array(p)*self.scale;
                }
            }
            pose.push(JointPose {rotation,position});
        }
        Ok(pose)
    }

    fn compose(&self,pose:Vec<JointPose>)->Vec<Mat4> {
        let mut matrices=Vec::new();
        for (joint,pose) in self.joints.iter().zip(pose) {
            let local=Mat4::from_rotation_translation(pose.rotation,pose.position);
            let parent=joint.parent.map_or(Mat4::IDENTITY,|index|matrices[index]);
            matrices.push(parent*local);
        }
        matrices
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lrformats::{animation,library::Library,model::Model};
    use std::path::PathBuf;

    #[test]
    fn original_wheel_keys_rotate_geometry_without_moving_axles_or_parent() {
        let library=Library::open(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM")).unwrap();
        let model=Model::load(&library,"bkjmw",Some("COMMON")).unwrap();
        let skeleton=Skeleton::load(library.find_in("BKJMW.SDB","COMMON").unwrap(),model.mesh.scale).unwrap();
        let animation=animation::parse(library.find_in("BKJMW.ADB","COMMON").unwrap()).unwrap();
        let bind=skeleton.pose(None).unwrap();
        let start=skeleton.pose(Some((&animation,"frwrd",0.0))).unwrap();
        let moving=skeleton.pose(Some((&animation,"frwrd",12.5))).unwrap();
        let looped=skeleton.pose(Some((&animation,"frwrd",35.0))).unwrap();
        assert_eq!(bind.len(),3);
        assert_eq!(moving[0],bind[0]);
        for i in 1..3 {
            let axle=bind[i].transform_point3(Vec3::ZERO);
            assert!((axle-moving[i].transform_point3(Vec3::ZERO)).length()<0.00001);
            let test_vertex=vec3(0.8,0.0,0.0);
            assert!((moving[i].transform_point3(test_vertex)-start[i].transform_point3(test_vertex)).length()>0.5);
            assert!((looped[i].transform_point3(test_vertex)-start[i].transform_point3(test_vertex)).length()<0.00001);
        }
        // Wheel geometry's local Z is its axle. All original spin keys must
        // preserve that axis; checking translation alone misses wrong handedness.
        for clip in ["frwrd","rvrse"] {
            for tick in 0..34 {
                let pose=skeleton.pose(Some((&animation,clip,tick as f32))).unwrap();
                for joint in 1..3 {
                    let axle=pose[joint].transform_vector3(Vec3::Z);
                    assert!((axle-vec3(0.0,-1.0,0.0)).length()<0.00001,"{clip} tick {tick}: {axle:?}");
                }
            }
        }
    }

    #[test]
    fn original_driver_default_keys_and_all_clips_sample() {
        let library=Library::open(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM")).unwrap();
        let model=Model::load(&library,"bkpelvis",Some("COMMON")).unwrap();
        let skeleton=Skeleton::load(library.find_in("PELVIS.SDB","COMMON").unwrap(),model.mesh.scale).unwrap();
        let animation=animation::parse(library.find_in("PELVIS.ADB","COMMON").unwrap()).unwrap();
        assert_eq!(animation.clips[9].name,"default");
        let seated=skeleton.pose_clip(Some((&animation,"default",0.0)),false).unwrap();
        assert_eq!(seated.len(),6);
        let right=skeleton.pose_clip(Some((&animation,"steer-r",0.0)),false).unwrap();
        let left=skeleton.pose_clip(Some((&animation,"steer-l",0.0)),false).unwrap();
        for (fraction,expected) in [(0.0,&right),(1.0,&left)] {
            let pose=skeleton.blend_clips(&animation,("steer-r",0.0),("steer-l",0.0),fraction).unwrap();
            for (actual,expected) in pose.iter().zip(expected) {
                assert!(actual.to_cols_array().into_iter().zip(expected.to_cols_array()).all(|(a,b)|(a-b).abs()<0.00001));
            }
        }
        assert_ne!(right,left);
        let middle=skeleton.blend_clips(&animation,("steer-r",0.0),("steer-l",0.0),0.5).unwrap();
        assert!(middle.iter().all(|m|m.is_finite()));
        // Original default keys remove the bind torso's small roll. The arms
        // themselves are identity in this clip; don't invent a large gesture.
        assert!((seated[2].transform_vector3(Vec3::Y)-Vec3::Y).length()<0.00001);
        for clip in &animation.clips {
            for time in [0.0,0.5,f32::from(clip.duration),100.0] {
                let pose=skeleton.pose_clip(Some((&animation,&clip.name,time)),false).unwrap();
                assert!(pose.iter().all(|matrix|matrix.is_finite()),"{}at{time}",clip.name);
            }
        }
    }
}
