//! Original ordinary on-route movement, displacement and speed recoil.
//! Contact-free Step and ordinary StepRoute/ApplyImpulse branches are calibrated;
//! secondary collider retry, boost/roll, locks and recovery remain incomplete.
use lrformats::route::RouteRecord;
use crate::route_cursor::RouteCursor;

/// Original00449340normalizes only while generating the column-major matrix.
pub fn quaternion_basis([x,y,z,w]:[f32;4])->[f32;9] {
    let scale=2.0/(x*x+y*y+z*z+w*w);
    let yy=y*scale;let zz=z*scale;let wx=w*scale*x;let xx=scale*x*x;
    [1.0-(zz*z+y*yy),yy*x-w*zz,zz*x+w*yy,
     yy*x+w*zz,1.0-(zz*z+xx),y*zz-wx,
     zz*x-w*yy,y*zz+wx,1.0-(y*yy+xx)]
}

#[derive(Clone)]
pub struct ContactFreeRoute<'a> {
    pub cursor:RouteCursor<'a>,
    pub position:[f32;3],
    pub basis:[f32;9],
    /// Original Step copies the render basis into physics_transform. StepRoute
    /// changes the render basis/COM but leaves this collision/inertia basis at
    /// the most recent Step until the next copy.
    pub collision_basis:[f32;9],
    /// World units per second (original body stores units per millisecond).
    pub velocity:[f32;3],
    pub multiplier:f32,
    pub slide_offset:f32,
    query_probes:Option<[[f32;3];4]>,
}

impl<'a> ContactFreeRoute<'a> {
    /// CarBody::UpdateRouteFollow0042a100: compare the current left axis to
    /// the forward axis 500 original milliseconds ahead, then clamp gain 8.
    /// Sampling a clone must not advance the live route/lap clock.
    pub fn steering(&self)->f32 {
        let current=quaternion_basis(self.cursor.rotation);let mut ahead=self.cursor.clone();ahead.advance(500.0);let basis=quaternion_basis(ahead.rotation);
        (crate::contact::dot(current[3..6].try_into().unwrap(),basis[..3].try_into().unwrap())*8.0).clamp(-1.0,1.0)
    }
    pub fn at_start(record:&'a RouteRecord)->Self {
        Self {cursor:RouteCursor::start_at_beginning(record),position:record.start_position,
              basis:quaternion_basis(record.start_rotation),collision_basis:quaternion_basis(record.start_rotation),velocity:[0.0;3],multiplier:1.0,slide_offset:0.0,query_probes:None}
    }

    /// Caller supplies the original integer-millisecond frame interval.
    pub fn advance(&mut self,elapsed:u32) {
        if elapsed==0 {return;}
        let ms=elapsed as f32;let speed=self.cursor.speed;
        self.cursor.speed=if speed<self.multiplier {
            (speed+0.00048828125*ms).min(self.multiplier)
        } else {(speed-0.002*ms).max(self.multiplier)};
        let old=self.position;
        self.cursor.advance(ms);
        if self.slide_offset>0.0 {self.slide_offset=(self.slide_offset-0.001953125*ms).max(0.0);}
        else {self.slide_offset=(self.slide_offset+0.001953125*ms).min(0.0);}
        self.basis=quaternion_basis(self.cursor.rotation);
        self.sync_position();
        self.collision_basis=self.basis;
        self.velocity=std::array::from_fn(|i|(self.position[i]-old[i])/ms*1000.0);
    }

    /// Ordinary00429d40calls004478b0with primary disabled. Owner callbacks
    /// stay outside the saved motion so RestoreState cannot undo CPB progress.
    pub fn advance_queried(&mut self,elapsed:u32,gear_range:[f32;2],
        mut query:impl FnMut([[f32;3];4],[[f32;3];4])->crate::chassis_dispatch::Selection)->bool {
        if elapsed==0 {return false;}
        let probes=|motion:&Self|crate::checkpoint_contacts::probes(motion.position,motion.basis,gear_range);
        // StepRoute displacement changes shape/cursor, not retained chassis
        // query endpoints. They are rebuilt by004478b0only on the next Step.
        let starts=self.query_probes.unwrap_or_else(||probes(self));
        self.query_probes=Some(starts);
        let saved=self.clone();
        self.advance(elapsed);
        let ends=probes(self);let selected=query(starts,ends);
        if selected.improvement_count==0 {self.query_probes=Some(ends);return false;}
        let retry=((f64::from(elapsed)*f64::from(selected.fraction)) as u32).saturating_sub(5);
        *self=saved;self.advance(retry);
        self.cursor.speed=-0.1;
        true
    }

    fn sync_position(&mut self) {
        self.position=std::array::from_fn(|i|self.cursor.position[i]+self.basis[3+i]*self.slide_offset);
    }

    /// Original00429680on-route penetration displacement, ordinary no-roll mode.
    pub fn displace(&mut self,displacement:[f32;3]) {
        let residual=self.cursor.displace(displacement);
        self.basis=quaternion_basis(self.cursor.rotation);
        self.slide_offset=crate::contact::dot(residual,self.basis[3..6].try_into().unwrap())
            .clamp(-self.cursor.width_a,self.cursor.width_b);
        self.sync_position();
    }

    /// Ordinary unlocked00429770response; amount uses original units, not
    /// a mass-scaled world velocity. Spin-hold and effect branches stay separate.
    pub fn impulse(&mut self,normal:[f32;3],mut amount:f32) {
        let mut side=crate::contact::dot(self.basis[..3].try_into().unwrap(),normal);
        if amount<0.0 {side=-side;amount=-amount;}
        amount=amount.min(240.0);
        self.cursor.speed=if side<0.0 {
            (self.cursor.speed-amount/240.0-(side+1.0)*0.25).max(-0.5)
        } else {(self.cursor.speed+amount/240.0+(1.0-side)*0.05).min(2.75)};
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn original_quaternion_basis_preserves_original_heading_sign() {
        let q=[0.0,0.0,-0.6,0.8];
        let a=quaternion_basis(q);let b=quaternion_basis(q.map(|v|v*0.4));
        assert!((a[0]-0.28).abs()<0.00001);
        assert!((a[1]-0.96).abs()<0.00001);
        assert!(a.into_iter().zip(b).all(|(x,y)|(x-y).abs()<0.00001));
    }
}
