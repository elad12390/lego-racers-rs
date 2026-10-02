//! Checkpoint-derived live diagnostic standings; full dispatch parity is open.
use crate::checkpoint_contacts::{CheckpointContacts,State};
use serde::Serialize;

pub struct Pose {
    pub position:[f32;3],
    pub probes:[[f32;3];4],
}

#[derive(Serialize)]
pub struct Report {
    pub mode:&'static str,
    pub ranks:Vec<u32>,
    pub states:Vec<State>,
    pub finite_probe_contacts:u32,
}

pub struct RacePositions {
    previous:Vec<[[f32;3];4]>,
    states:Vec<State>,
    ranks:Vec<u32>,
    contacts:u32,
    finish_count:u32,
    dispatched:bool,
}

impl RacePositions {
    pub fn new(poses:&[Pose])->Self {
        Self {previous:poses.iter().map(|p|p.probes).collect(),states:vec![State::default();poses.len()],
            ranks:(1..=poses.len() as u32).collect(),contacts:0,finish_count:0,dispatched:false}
    }

    pub fn advance(&mut self,poses:&[Pose],checkpoints:&CheckpointContacts) {
        assert_eq!(poses.len(),self.states.len());
        for (i,pose) in poses.iter().enumerate() {
            self.contacts+=checkpoints.advance_probes(&mut self.states[i],self.previous[i],pose.probes);
            self.previous[i]=pose.probes;
        }
        self.ranks=crate::standings::ranks(&poses.iter().enumerate().map(|(i,p)|self.states[i].standing(p.position,self.ranks[i])).collect::<Vec<_>>(),&checkpoints.table.records);
    }

    pub fn report(&self)->Report {
        Report {mode:if self.dispatched {"original_chassis_query_owner_checkpoint_ranking_wheel_effect_dispatch_pending"} else {"original_checkpoint_tree_numeric_ranking_whole_world_dispatch_pending"},ranks:self.ranks.clone(),states:self.states.clone(),finite_probe_contacts:self.contacts}
    }

    pub fn state_mut(&mut self,index:usize)->&mut State {&mut self.states[index]}
    pub fn first_two_states_mut(&mut self)->(&mut State,&mut State) {let (left,right)=self.states.split_at_mut(1);(&mut left[0],&mut right[0])}
    pub fn add_contacts(&mut self,count:u32) {self.contacts+=count;}

    /// Rank actual owner-callback state without another post-frame sweep.
    pub fn rank_dispatched(&mut self,poses:&[Pose],checkpoints:&CheckpointContacts) {
        assert_eq!(poses.len(),self.states.len());self.dispatched=true;
        self.ranks=crate::standings::ranks(&poses.iter().enumerate().map(|(i,p)|self.states[i].standing(p.position,self.ranks[i])).collect::<Vec<_>>(),&checkpoints.table.records);
    }

    /// Game_State3/4 assign and freeze finishing positions in racer-table
    /// iteration order for a shared tick. Continued body movement is separate.
    pub fn finish(&mut self,finished:&[bool]) {
        assert_eq!(finished.len(),self.states.len());
        freeze_finishers(&mut self.states,&mut self.ranks,&mut self.finish_count,finished);
    }
}

pub fn freeze_finishers(states:&mut [State],ranks:&mut [u32],finish_count:&mut u32,finished:&[bool]) {
    assert_eq!(states.len(),ranks.len());assert_eq!(states.len(),finished.len());
    for (index,state) in states.iter_mut().enumerate() {
        if finished[index] && state.flags&0x1000==0 {
            state.flags|=0x1000;
            *finish_count+=1;
            ranks[index]=*finish_count;
        }
    }
}

pub fn finished(player:&crate::race::Race,rivals:&[crate::rivals::Rival<'_>])->Vec<bool> {
    std::iter::once(player.finished).chain(rivals.iter().map(|r|r.race.finished)).collect()
}

/// One shared original probe binding for the native window and headless races.
pub fn poses(player:&crate::vehicle::Vehicle,rivals:&[crate::rivals::Rival<'_>])->Vec<Pose> {
    let mut poses=vec![Pose {position:player.position,probes:player.checkpoint_probes()}];
    poses.extend(rivals.iter().map(|rival|Pose {position:rival.motion.position,
        probes:crate::checkpoint_contacts::probes(rival.motion.position,rival.motion.basis,rival.data.chassis.gear_range)}));
    poses
}
