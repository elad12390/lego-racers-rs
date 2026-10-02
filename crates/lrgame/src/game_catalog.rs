//! Original catalog and human-facing labels; no synthesized track/racer roster.
use lrformats::{appearance::{self,Car,Driver},library::Library,race_catalog::{self,Race,Circuit}};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
pub struct Rules {pub finish_points:[u32;6],pub track_names:BTreeMap<String,String>,pub circuit_names:[String;7]}
pub struct Catalog {pub races:Vec<Race>,pub circuits:Vec<Circuit>,pub cars:Vec<Car>,pub drivers:Vec<Driver>,pub rules:Rules,titles:lrformats::string_table::StringTable}
impl Catalog {
    pub fn load(library:&Library)->Result<Self,String> {
        let read=|file:&str,owner:&str|library.find_in(file,owner).ok_or_else(||format!("missing {owner}/{file}"));
        let mut circuits=race_catalog::circuits(read("LEGORACE.CRB","MENUDATA")?)?;circuits.sort_by_key(|c|c.index);
        let rules:Rules=serde_json::from_str(include_str!("../../../assets/native/rules.json")).map_err(|e|e.to_string())?;
        if rules.finish_points.iter().any(|p|*p==0)||rules.finish_points.windows(2).any(|p|p[0]<=p[1]) {return Err("invalid finish award configuration".into());}
        let titles=lrformats::string_table::StringTable::parse(library.find_at("CIRCUIT.SRF","MENUDATA","ENGLISH").ok_or("missing original circuit text")?)?;
        Ok(Self {races:race_catalog::races(read("LEGORACE.RCB","MENUDATA")?)?,circuits,titles,
            cars:appearance::cars(read("CHAMPS.CCB","COMMON")?)?,drivers:appearance::drivers(read("DRIVERS.DDB","COMMON")?)?,rules})
    }
    pub fn title(&self,race:&Race)->String {
        let key=if race.mirrored {race.name.strip_suffix('2').unwrap_or(&race.name)} else {&race.name};
        let title=self.titles.get(race.text_id as usize).ok().map(str::trim).unwrap_or_else(||self.rules.track_names.get(key).map(String::as_str).unwrap_or(&race.name));
        format!("{title}{}",if race.mirrored {" (Mirrored)"} else {""})
    }
    pub fn circuit_races(&self,circuit:&str)->Vec<usize> {
        let mut races=self.races.iter().enumerate().filter(|(_,r)|r.circuit.as_deref()==Some(circuit)).collect::<Vec<_>>();
        races.sort_by_key(|(_,r)|r.slot);races.into_iter().map(|(i,_)|i).collect()
    }
}
