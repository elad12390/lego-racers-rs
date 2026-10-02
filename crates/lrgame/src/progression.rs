//! Native persistence of recovered medal/part gates; original LRS is untouched.
use crate::{profile::Profile,game_catalog::Catalog};
impl Profile {
    pub fn driver_part_allowed(&self,flag:u8,catalog:&Catalog)->bool {
        // Screen_BuildFilteredList_*00483c60..: flags <3 are unconditional;
        // 3..9 address the gold-award circuit mask, 0x80 the twelve-ghost mask.
        match flag {0..=2=>true,3..=9=>catalog.circuits.get((flag-3) as usize).is_some_and(|c|self.circuit_medals.get(&c.name)==Some(&1)),128=>{
            let races=catalog.races.iter().filter(|r|r.circuit.as_deref().is_some_and(|c|matches!(c,"c0"|"c1"|"c2"))).collect::<Vec<_>>();
            races.len()==12&&races.iter().all(|r|self.trial_wins.get(&r.name)==Some(&true))
        },_=>false}
    }
    pub fn record_circuit(&mut self,catalog:&Catalog,index:usize,scores:[u32;6])->Result<u32,String> {
        let circuit=catalog.circuits.get(index).ok_or("circuit award outside original catalog")?;
        let rank=1+scores.iter().skip(1).filter(|v|**v>scores[0]).count() as u32;
        self.circuit_medals.entry(circuit.name.clone()).and_modify(|v|*v=(*v).min(rank)).or_insert(rank);
        // Gold/silver (screens 0x16/0x17) claim the next CRB record; bronze
        // (0x18) records a medal only. Ties count strictly higher scores.
        if rank<=2 {if let Some(next)=&circuit.next {let next=catalog.circuits.iter().find(|c|&c.name==next).ok_or("next CRB circuit not found")?;self.unlocked_circuit=self.unlocked_circuit.max(next.index);}}
        Ok(rank)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn silver_opens_next_circuit_gold_opens_parts_and_ties_share_rank() {
        let lib=lrformats::library::Library::open(std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group/LEGO.JAM")).unwrap();let c=Catalog::load(&lib).unwrap();let mut p=Profile::default();
        assert!(p.driver_part_allowed(1,&c));assert!(p.driver_part_allowed(2,&c));assert!(!p.driver_part_allowed(3,&c));assert!(!p.driver_part_allowed(128,&c));
        assert_eq!(p.record_circuit(&c,0,[90,100,80,70,60,50]).unwrap(),2);assert_eq!(p.unlocked_circuit,1);assert!(!p.driver_part_allowed(3,&c));
        assert_eq!(p.record_circuit(&c,0,[100,100,80,70,60,50]).unwrap(),1);assert!(p.driver_part_allowed(3,&c));assert!(!p.driver_part_allowed(4,&c));
        assert_eq!(p.record_circuit(&c,1,[80,100,90,70,60,50]).unwrap(),3);assert_eq!(p.unlocked_circuit,1);
    }
}
