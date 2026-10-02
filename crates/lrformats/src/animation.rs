//! Original ADB key tables, six-integer channels and named clip metadata.
use crate::tok::{self,Node,Value};

pub struct Animation {
    pub positions:Vec<[f32;3]>,
    pub rotations:Vec<[f32;4]>,
    pub times:Vec<u16>,
    pub channels:Vec<Channel>,
    pub clips:Vec<Clip>,
}

pub struct Channel {pub rotation_start:usize,pub time_start:usize,pub rotation_count:usize,pub position_start:usize,pub position_time_start:usize,pub position_count:usize}
pub struct Clip {pub name:String,pub channel_start:usize,pub duration:u16,pub loop_duration:u16,pub frames_per_second:f32}

pub fn parse(data:&[u8])->Result<Animation,String> {
    let nodes=tok::parse(data).map_err(|e|e.to_string())?;
    let track=block(&nodes,0x27).ok_or("ADB key tables missing")?;
    let values=rows(block(track,0x28).ok_or("ADB position table missing")?).into_iter().flatten()
        .map(|v|v.as_f32().ok_or_else(||"ADB invalid position value".to_string())).collect::<Result<Vec<_>,_>>()?;
    if values.len()%3!=0 || values.iter().any(|v|!v.is_finite()) {return Err("ADB incomplete/nonfinite position".into());}
    let positions=values.chunks_exact(3).map(|v|[v[0],v[1],v[2]]).collect::<Vec<_>>();
    let rotations=rows(block(track,0x29).ok_or("ADB rotation table missing")?).into_iter().map(|row| {
        if row.len()!=4 {return Err("ADB quaternion requires4fields".into());}
        Ok(std::array::from_fn(|i|row[i].as_f32().unwrap_or(f32::NAN)))
    }).collect::<Result<Vec<_>,String>>()?;
    if rotations.iter().flatten().any(|v|!v.is_finite()) {return Err("ADB nonfinite quaternion".into());}
    let times=rows(block(track,0x2a).ok_or("ADB time table missing")?).into_iter().flatten().map(|v|
        v.as_u32().and_then(|v|u16::try_from(v).ok()).ok_or_else(||"ADB invalid time".to_string()))
        .collect::<Result<Vec<_>,_>>()?;
    let values=rows(block(&nodes,0x2b).ok_or("ADB channels missing")?).into_iter().flatten().map(|v|
        v.as_u32().map(|v|v as usize).ok_or_else(||"ADB negative channel index".to_string()))
        .collect::<Result<Vec<_>,_>>()?;
    if values.len()%6!=0 {return Err("ADB incomplete six-field channel".into());}
    let channels=values.chunks_exact(6).map(|row|Channel {rotation_start:row[0],time_start:row[1],rotation_count:row[2],position_start:row[3],position_time_start:row[4],position_count:row[5]}).collect::<Vec<_>>();
    for channel in &channels {
        let count=channel.rotation_count;
        if channel.rotation_start.checked_add(count).is_none_or(|end|end>rotations.len()) || channel.time_start.checked_add(count).is_none_or(|end|end>times.len()) {return Err("ADB rotation channel out of range".into());}
        if times[channel.time_start..channel.time_start+count].windows(2).any(|t|t[1]<=t[0]) {return Err("ADB rotation times must increase".into());}
        let count=channel.position_count;
        if channel.position_start.checked_add(count).is_none_or(|end|end>positions.len()) || channel.position_time_start.checked_add(count).is_none_or(|end|end>times.len()) {return Err("ADB position channel out of range".into());}
        if times[channel.position_time_start..channel.position_time_start+count].windows(2).any(|t|t[1]<=t[0]) {return Err("ADB position times must increase".into());}
    }
    let mut clips=Vec::new();
    for triple in block(&nodes,0x2c).ok_or("ADB clips missing")?.windows(3) {
        let [Node::Keyword(0x2c),Node::Str(name),Node::Block(fields)]=triple else {continue;};
        let integer=|keyword|fields.windows(2).find_map(|pair|match pair {[Node::Keyword(k),Node::Int(v)] if *k==keyword=>usize::try_from(*v).ok(),_=>None});
        let channel_start=integer(0x2b).ok_or("ADB clip channel base missing")?;
        let duration=integer(0x2d).and_then(|v|u16::try_from(v).ok()).ok_or("ADB clip duration missing")?;
        let loop_duration=integer(0x2e).and_then(|v|u16::try_from(v).ok()).ok_or("ADB loop duration missing")?;
        let frames_per_second=integer(0x2f).ok_or("ADB frame rate missing")? as f32;
        if channel_start>=channels.len() || frames_per_second<=0.0 {return Err("ADB invalid clip range/frame rate".into());}
        clips.push(Clip {name:name.clone(),channel_start,duration,loop_duration,frames_per_second});
    }
    Ok(Animation {positions,rotations,times,channels,clips})
}

fn block(nodes:&[Node],keyword:u8)->Option<&[Node]> {
    let at=nodes.iter().position(|n|*n==Node::Keyword(keyword))?;
    nodes[at+1..].iter().find_map(|n|if let Node::Block(body)=n {Some(body.as_slice())} else {None})
}

fn rows(nodes:&[Node])->Vec<Vec<Value>> {
    nodes.iter().flat_map(|n|match n {
        Node::Packed {rows,..}=>rows.clone(),
        Node::Record {fields,..}=>vec![fields.clone()],
        Node::Int(v)=>vec![vec![Value::I32(*v)]],
        Node::Float(v)=>vec![vec![Value::F32(*v)]],
        _=>Vec::new(),
    }).collect()
}

impl Animation {
    /// Key interval for a looped rotation channel. Sampling/interpolation is
    /// left to the native math library; original values are never synthesized.
    pub fn rotation_keys(&self,clip:&str,joint:usize,time:f32)->Result<Option<([f32;4],[f32;4],f32)>,String> {
        self.sample_rotation(clip,joint,time,true)
    }

    pub fn sample_rotation(&self,clip:&str,joint:usize,time:f32,looped:bool)->Result<Option<([f32;4],[f32;4],f32)>,String> {
        let clip=self.clips.iter().find(|c|c.name.eq_ignore_ascii_case(clip)).ok_or("ADB named clip missing")?;
        let channel=self.channels.get(clip.channel_start+joint).ok_or("ADB joint channel missing")?;
        let count=channel.rotation_count;
        if count==0 {return Ok(None);}
        let quats=&self.rotations[channel.rotation_start..channel.rotation_start+count];
        if count==1 {return Ok(Some((quats[0],quats[0],0.0)));}
        let times=&self.times[channel.time_start..channel.time_start+count];
        let duration=if looped {clip.loop_duration} else {clip.duration};
        let (a,b,fraction)=interval(times,duration,time,looped);
        Ok(Some((quats[a],quats[b],fraction)))
    }

    pub fn sample_position(&self,clip:&str,joint:usize,time:f32,looped:bool)->Result<Option<[f32;3]>,String> {
        let clip=self.clips.iter().find(|c|c.name.eq_ignore_ascii_case(clip)).ok_or("ADB named clip missing")?;
        let channel=self.channels.get(clip.channel_start+joint).ok_or("ADB joint channel missing")?;
        let count=channel.position_count;
        if count==0 {return Ok(None);}
        let positions=&self.positions[channel.position_start..channel.position_start+count];
        let times=&self.times[channel.position_time_start..channel.position_time_start+count];
        let duration=if looped {clip.loop_duration} else {clip.duration};
        let (a,b,f)=interval(times,duration,time,looped);
        Ok(Some(std::array::from_fn(|i|positions[a][i]+(positions[b][i]-positions[a][i])*f)))
    }
}

fn interval(times:&[u16],duration:u16,time:f32,looped:bool)->(usize,usize,f32) {
    if times.len()==1 {return (0,0,0.0);}
    let last=times.len()-1;
    let looped=looped && duration>0;
    let time=if looped {time.rem_euclid(f32::from(duration))} else {time};
    if !looped {
        if time<=f32::from(times[0]) {return (0,0,0.0);}
        if time>=f32::from(times[last]) {return (last,last,0.0);}
    }
    let next=times.iter().position(|t|time<f32::from(*t)).unwrap_or(times.len());
    let (a,b,start,end)=if next==0 {
        (last,0,f32::from(times[last])-f32::from(duration),f32::from(times[0]))
    } else if next==times.len() {
        (last,0,f32::from(times[last]),f32::from(times[0])+f32::from(duration))
    } else {(next-1,next,f32::from(times[next-1]),f32::from(times[next]))};
    (a,b,if end==start {0.0} else {(time-start)/(end-start)})
}
