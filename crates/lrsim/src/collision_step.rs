//! Chassis interval retry/restore from004478b0and00444ef0.
//! Spatial contact queries and vehicle force/support integration are callers.
use crate::contact::{length,sub,Contacts};

#[derive(Clone,Copy)]
pub struct Hit {pub fraction:f32,pub normal:[f32;3],pub probe:usize}

/// Original picks strictly greatest penetration squared, not earliest fraction.
pub fn sweep(contacts:&Contacts,starts:[[f32;3];4],ends:[[f32;3];4])->Option<Hit> {
    if let Some(selection)=contacts.primary_selection(starts,ends) {
        return selection.probe.map(|probe|Hit {fraction:selection.fraction,normal:selection.normal,probe});
    }
    let mut selected=None;let mut penetration=-f32::MAX;
    for (probe,(start,end)) in starts.into_iter().zip(ends).enumerate() {
        if let Some((fraction,normal))=contacts.sweep(start,end) {
            let depth=length(sub(end,start))*(1.0-fraction);
            let squared=depth*depth;
            if squared>penetration {penetration=squared;selected=Some(Hit {fraction,normal,probe});}
        }
    }
    selected
}

pub fn retry_seconds(trial:f32,fraction:f32)->f32 {
    let ticks=(trial*1000.0*fraction) as u32;
    ticks.saturating_sub(5) as f32/1000.0
}

/// Trial against saved state, shorten by original5ms, then respond and spend
/// the rest of the interval. Callers receive false if a zero-time contact has
/// no velocity response; the safe state is retained rather than looping forever.
/// That safety termination is a modern guard, not original behavior evidence.
pub fn advance<S:Clone>(state:&mut S,dt:f32,
    mut integrate:impl FnMut(&mut S,f32),mut query:impl FnMut(&S,&S)->Option<Hit>,
    respond:impl Fn(&mut S,[f32;3])->bool)->bool {
    let mut remaining=dt;
    let mut trial_interval=dt;
    let mut pending=None;
    while remaining>0.0 {
        let saved=state.clone();
        let mut trial=saved.clone();
        integrate(&mut trial,trial_interval);
        if let Some(hit)=query(&saved,&trial) {
            pending=Some(hit);
            let retry=retry_seconds(trial_interval,hit.fraction);
            if retry>0.0 {trial_interval=retry;continue;}
            // No time elapsed: RestoreState precedes the collision response.
            if !respond(state,hit.normal) {return false;}
            pending=None;
            trial_interval=remaining;
            continue;
        }
        *state=trial;
        remaining=(remaining-trial_interval).max(0.0);
        if let Some(hit)=pending.take() {respond(state,hit.normal);}
        trial_interval=remaining;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handling::wall_response;

    #[test]
    fn retry_reintegrates_from_saved_state_then_spends_remaining_interval() {
        #[derive(Clone)]
        struct Motion {x:f32,velocity:f32}
        let mut motion=Motion {x:7.2,velocity:120.0};
        assert!(advance(&mut motion,0.01,
            |m,t|m.x+=m.velocity*t,
            |a,b|if a.x<8.0 && b.x>=8.0 {Some(Hit {fraction:(8.0-a.x)/(b.x-a.x),normal:[-1.0,0.0,0.0],probe:0})} else {None},
            |m,n| {m.velocity=wall_response([m.velocity,0.0,0.0],n)[0];true}));
        assert!((motion.x-6.96).abs()<0.00001);
        assert!((motion.velocity+40.0).abs()<0.00001);
    }

    #[test]
    fn unresolved_zero_time_contact_keeps_safe_state_and_reports_failure() {
        let mut position=0.0;
        assert!(!advance(&mut position,0.01,|p,_|*p=1.0,
            |_,_|Some(Hit {fraction:0.0,normal:[1.0,0.0,0.0],probe:0}),|_,_|false));
        assert_eq!(position,0.0);
    }
}
