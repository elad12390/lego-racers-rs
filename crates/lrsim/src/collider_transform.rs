//! Original00410de0 inverse point and00410c80normal rotation, including spills.
use serde::{Deserialize,Serialize};

#[derive(Clone,Copy,Deserialize,Serialize)]
pub struct ColliderTransform {pub origin:[f32;3],pub axes:[[f32;3];3]}
impl ColliderTransform {
    pub fn inverse_vector(&self,vector:[f32;3])->[f32;3] {
        self.axes.map(|axis| {
            let mut projected=axis[0]*vector[0];
            for i in 1..3 {projected=(f64::from(axis[i])*f64::from(vector[i])+f64::from(projected)) as f32;}
            projected
        })
    }
    pub fn inverse_point(&self,point:[f32;3])->[f32;3] {
        self.axes.map(|axis| {
            let mut projected=axis[0]*point[0];
            for i in 1..3 {projected=(f64::from(axis[i])*f64::from(point[i])+f64::from(projected)) as f32;}
            let origin=axis.into_iter().zip(self.origin).map(|(a,p)|f64::from(a)*f64::from(p)).sum::<f64>();
            (f64::from(projected)-origin) as f32
        })
    }

    pub fn rotate(&self,normal:[f32;3])->[f32;3] {
        std::array::from_fn(|i| {
            let mut value=self.axes[0][i]*normal[0];
            for (axis,n) in self.axes[1..].iter().zip(&normal[1..]) {value=(f64::from(axis[i])*f64::from(*n)+f64::from(value)) as f32;}
            value
        })
    }

    pub fn point(&self,point:[f32;3])->[f32;3] {
        let vector=self.rotate(point);std::array::from_fn(|i|vector[i]+self.origin[i])
    }
}
