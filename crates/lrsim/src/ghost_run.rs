//! Noncollidable time-race replay with original 250ms interpolation cadence.
//! Native saved encoding is separate from original GHB/LRS.
use serde::{Serialize,Deserialize};
#[derive(Clone,Serialize,Deserialize)]
pub struct Pose {pub position:[f32;3],pub rotation:[f32;4]}
#[derive(Clone,Serialize,Deserialize)]
pub struct Run {pub samples:Vec<Pose>,pub lap_times:Vec<f64>}
impl Run {
    pub fn original(ghost:lrformats::ghost::Ghost)->Self {Self {samples:ghost.samples.into_iter().map(|p|Pose {position:p.position,rotation:p.rotation}).collect(),lap_times:ghost.lap_ms.map(|v|f64::from(v)/1000.0).to_vec()}}
    pub fn validate(&self)->Result<(),String> {
        if self.samples.len()<2||self.lap_times.len()!=3||self.lap_times.iter().any(|v|!v.is_finite()||*v<=0.0)||self.samples.iter().any(|p|p.position.iter().chain(&p.rotation).any(|v|!v.is_finite())||p.rotation.iter().map(|v|v*v).sum::<f32>()<0.001) {return Err("invalid native time-race ghost; save preserved".into());}Ok(())
    }
    pub fn sample(&self,seconds:f64)->Option<Pose> {
        // Original GHB files have a fixed recorder capacity; native saves do
        // not. Bound playback by actual samples rather than dropping slow runs.
        if !seconds.is_finite()||seconds<0.0||seconds>=(self.samples.len().saturating_sub(1) as f64*0.25) {return None;}
        let ms=(seconds*1000.0) as u32;let index=(ms/250) as usize;
        let a=self.samples.get(index)?;let b=self.samples.get(index+1)?;let t=(ms%250) as f32*0.004;
        Some(Pose {position:std::array::from_fn(|i|(b.position[i]-a.position[i])*t+a.position[i]),rotation:crate::route_cursor::lerp_quat(&a.rotation,&b.rotation,t)})
    }
    pub fn total(&self)->f64 {self.lap_times.iter().sum()}
}
pub struct Recorder {samples:Vec<Pose>,previous:Pose,previous_time:f64}
impl Recorder {
    pub fn new(pose:Pose)->Self {Self {samples:vec![pose.clone()],previous:pose,previous_time:0.0}}
    pub fn observe(&mut self,time:f64,pose:Pose) {
        if !time.is_finite()||time<=self.previous_time {return;}
        loop {
            let next=self.samples.len() as f64*0.25;if next>time {break;}
            let t=((next-self.previous_time)/(time-self.previous_time)).clamp(0.0,1.0) as f32;
            self.samples.push(Pose {position:std::array::from_fn(|i|(pose.position[i]-self.previous.position[i])*t+self.previous.position[i]),rotation:crate::route_cursor::lerp_quat(&self.previous.rotation,&pose.rotation,t)});
        }
        self.previous=pose;self.previous_time=time;
    }
    pub fn finish(mut self,lap_times:Vec<f64>)->Result<Run,String> {
        // Keep final interpolation available through the finishing fraction.
        self.samples.push(self.previous);
        let run=Run {samples:self.samples,lap_times};run.validate()?;Ok(run)
    }
}
