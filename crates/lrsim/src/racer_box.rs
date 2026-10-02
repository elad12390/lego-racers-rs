//! Original00441790:15separating axes, but only6face contact-normal candidates.
//! Contact-point averaging and two-car impulse resolution are separate stages.
use crate::contact::{cross,dot,sub};
use serde::{Deserialize,Serialize};
use std::sync::atomic::{AtomicU64,Ordering};

static EMPTY_MANIFOLDS:AtomicU64=AtomicU64::new(0);
/// Diagnostic safety-deviation count; never counted as a resolved contact.
pub fn rejected_empty_manifolds()->u64 {EMPTY_MANIFOLDS.load(Ordering::Relaxed)}

#[derive(Deserialize,Serialize)]
pub struct Body {pub position:[f32;3],pub basis:[f32;9],pub extents:[f32;3],pub scale:f32}
impl Body {
    pub fn axes(&self)->[[f32;3];3] {
        std::array::from_fn(|i|self.basis[3*i..3*i+3].try_into().unwrap())
    }
    fn half(&self)->[f32;3] {self.extents.map(|v|v*0.5*self.scale)}
}

#[derive(Serialize)]
pub struct Contact {pub penetration:f32,pub normal:[f32;3],pub offsets:[f32;2]}

fn projection(axes:[[f32;3];3],half:[f32;3],axis:[f32;3])->f64 {
    (0..3).map(|i|f64::from((precise_dot(axes[i],axis) as f32).abs())*f64::from(half[i])).sum()
}

fn precise_dot(a:[f32;3],b:[f32;3])->f64 {
    (0..3).map(|i|f64::from(a[i])*f64::from(b[i])).sum()
}

pub fn separating_contact(a:&Body,b:&Body)->Option<Contact> {
    if a.scale==0.0 || b.scale==0.0 {return None;}
    let aa=a.axes();let ba=b.axes();let ah=a.half();let bh=b.half();
    let displacement=sub(b.position,a.position);
    let mut result=Contact {penetration:f32::MAX,normal:[0.0;3],offsets:[0.0;2]};
    // Preserve original strict improvement and asymmetric face-test ordering.
    for (other,index) in [(false,0),(true,0),(false,1),(false,2),(true,1),(true,2)] {
        let (axis,own,radius)=if other {(ba[index],bh[index],projection(aa,ah,ba[index]))}
            else {(aa[index],ah[index],projection(ba,bh,aa[index]))};
        let distance=precise_dot(displacement,axis).abs();let sum=f64::from(own)+radius;
        if sum<distance {return None;}
        let overlap=sum-distance;
        if overlap<f64::from(result.penetration) {
            result=Contact {penetration:overlap as f32,normal:axis,offsets:[own-overlap as f32,(radius-f64::from(overlap as f32)) as f32]};
        }
    }
    for x in aa {
        for y in ba {
            let axis=cross(x,y);
            let radius=projection(aa,ah,axis)+projection(ba,bh,axis);
            // Original skips nearly parallel cross axes based on projected
            // extent sum, not the length of the axis or a generic epsilon.
            if radius.abs()>=0.001 && radius<precise_dot(displacement,axis).abs() {return None;}
        }
    }
    if dot(sub(a.position,b.position),result.normal)<0.0 {result.normal=result.normal.map(|v|-v);}
    Some(result)
}

#[derive(Serialize)]
pub struct BoxContact {pub penetration:f32,pub normal:[f32;3],pub point:[f32;3]}

/// Original00441330averages selected corners of both scaled bodies. It does
/// not use the midpoint of their centers or a single deepest corner.
pub fn collide(a:&Body,b:&Body)->Option<BoxContact> {
    let contact=separating_contact(a,b)?;
    let mut point=[0.0;3];let mut count=0;
    for (index,body) in [a,b].into_iter().enumerate() {
        let axes=body.axes();let half=body.half();
        // Preserve the original corner order because sums round at f32.
        for signs in [[1.0,1.0,1.0],[1.0,1.0,-1.0],[1.0,-1.0,1.0],[1.0,-1.0,-1.0],
                      [-1.0,1.0,1.0],[-1.0,1.0,-1.0],[-1.0,-1.0,1.0],[-1.0,-1.0,-1.0]] {
            // Original keeps X on the x87 stack; Y/Z intermediates spill to
            // binary32. Boundary corner inclusion is sensitive to these spills.
            let corner: [f64;3]=std::array::from_fn(|i| {
                let x=f64::from(axes[0][i])*f64::from(body.extents[0])*0.5*f64::from(body.scale)*f64::from(signs[0]);
                let y=f64::from(axes[1][i])*f64::from(body.extents[1])*0.5*f64::from(body.scale)*f64::from(signs[1]);
                let z=f64::from(axes[2][i]*half[2])*f64::from(signs[2]);
                if i==0 {x+y+z} else {
                    let xy=(x as f32+y as f32) as f32;
                    f64::from((f64::from(xy)+z) as f32)
                }
            });
            let facing=(0..3).map(|i|corner[i]*f64::from(contact.normal[i])).sum::<f64>()*if index==0 {1.0} else {-1.0};
            if facing>=f64::from(contact.offsets[index]) {
                for i in 0..3 {point[i]=(f64::from(body.position[i])+f64::from(point[i])+corner[i]) as f32;}
                count+=1;
            }
        }
    }
    // The original divides by zero here on retained RACEC0R3 tick4145.
    // Reject an empty manifold rather than fabricate a corner or transmit NaN
    // through both cars. This measured safety deviation is not exact parity.
    if count==0 {EMPTY_MANIFOLDS.fetch_add(1,Ordering::Relaxed);return None;}
    Some(BoxContact {penetration:contact.penetration,normal:contact.normal,point:point.map(|v|v/count as f32)})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn two_racer_boxes_report_signed_face_overlap_and_disabled_body_does_not_collide() {
        let a=Body {position:[0.0;3],basis:[1.0,0.0,0.0,0.0,1.0,0.0,0.0,0.0,1.0],extents:[8.0,5.0,6.2],scale:1.0};
        let mut b=Body {position:[7.0,0.0,0.0],basis:a.basis,extents:a.extents,scale:1.0};
        let hit=separating_contact(&a,&b).unwrap();
        assert_eq!(hit.penetration,1.0);assert_eq!(hit.normal,[-1.0,0.0,0.0]);
        assert_eq!(hit.offsets,[3.0,3.0]);
        b.scale=0.0;assert!(separating_contact(&a,&b).is_none());
    }

    #[test]
    fn near_touching_racec0r3_pair_cannot_poison_both_cars_with_an_empty_manifold() {
        let source=include_str!("../tests/fixtures/empty_manifold.json");
        #[derive(Deserialize)]struct Pair {a:Body,b:Body}
        let pairs:Vec<Pair>=serde_json::from_str(source).unwrap();let p=&pairs[0];
        let separation=separating_contact(&p.a,&p.b).unwrap();
        assert!(separation.penetration>0.0);
        let before=rejected_empty_manifolds();
        assert!(collide(&p.a,&p.b).is_none());
        assert!(rejected_empty_manifolds()>before);
    }
}
