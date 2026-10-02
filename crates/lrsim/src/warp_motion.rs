//! Route_MoveToward0041eaf0 and WarpEffect0045d8a0: CPB-center transit.
use lrformats::checkpoints::CheckpointTable;
use crate::contact::{length,sub};

pub fn advance(table:&CheckpointTable,point:&mut [f32;3],mut amount:f32,start:Option<usize>)->Result<(),String> {
    if !amount.is_finite()||amount<0.0 {return Err("invalid warp distance".into());}
    if table.records.is_empty() {return Ok(());}
    let mut at=start.unwrap_or(0);let mut zero_hops=0;
    while amount>0.0 {
        let record=table.records.get(at).ok_or("warp checkpoint link out of range")?;
        let delta=sub(record.center,*point);let distance=length(delta);
        if amount<=distance {for i in 0..3 {point[i]+=delta[i]/distance*amount;}return Ok(());}
        amount-=distance;*point=record.center;at=record.next[0] as usize;
        if distance==0.0 {zero_hops+=1;if zero_hops>table.records.len() {return Err("warp route has zero-length cycle".into());}} else {zero_hops=0;}
    }
    Ok(())
}
