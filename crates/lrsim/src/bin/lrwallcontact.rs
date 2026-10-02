use std::io::{self,Read};
fn main()->Result<(),Box<dyn std::error::Error>> {
    let mut input=String::new();io::stdin().read_to_string(&mut input)?;
    let cases:Vec<lrsim::wall_contact::Case>=serde_json::from_str(&input)?;
    println!("{}",serde_json::to_string(&cases.iter().map(lrsim::wall_contact::solve).collect::<Vec<_>>())?);Ok(())
}
