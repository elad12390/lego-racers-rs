//! Real native handling implementation probe for unchanged-x86 comparison.
use std::io::{self, Read};
use std::process::ExitCode;

use lrsim::handling::{normal_force, player_controls, ControlCase, ForceCase};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    if std::env::args().nth(1).as_deref() == Some("--support-torque") {
        let cases:Vec<lrsim::handling::SupportTorqueCase>=serde_json::from_str(&input)?;
        let results:Vec<_>=cases.into_iter().map(lrsim::handling::support_torque).collect();
        println!("{}",serde_json::to_string(&results)?);
    } else if std::env::args().nth(1).as_deref() == Some("--landing") {
        let cases:Vec<lrsim::handling::LandingCase>=serde_json::from_str(&input)?;
        let results:Vec<_>=cases.into_iter().map(lrsim::handling::landing_response).collect();
        println!("{}",serde_json::to_string(&results)?);
    } else if std::env::args().nth(1).as_deref() == Some("--ground") {
        let normals:Vec<[f32;3]>=serde_json::from_str(&input)?;
        let results:Vec<_>=normals.into_iter().map(lrsim::handling::ground_acceleration).collect();
        println!("{}",serde_json::to_string(&results)?);
    } else if std::env::args().nth(1).as_deref() == Some("--surfaces") {
        let flags: Vec<u32> = serde_json::from_str(&input)?;
        let results: Vec<_> = flags
            .into_iter()
            .map(|f| u32::from(lrformats::collision_materials::blocks_chassis(f)))
            .collect();
        println!("{}", serde_json::to_string(&results)?);
    } else if std::env::args().nth(1).as_deref() == Some("--controls") {
        let cases: Vec<ControlCase> = serde_json::from_str(&input)?;
        let results: Vec<_> = cases.iter().map(player_controls).collect();
        println!("{}", serde_json::to_string(&results)?);
    } else {
        let cases: Vec<ForceCase> = serde_json::from_str(&input)?;
        let results: Vec<_> = cases.iter().map(normal_force).collect();
        println!("{}", serde_json::to_string(&results)?);
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
