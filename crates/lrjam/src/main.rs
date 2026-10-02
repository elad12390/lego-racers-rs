use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::ExitCode;

use lrjam::Jam;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["list", jam] => list(jam),
        ["check", jam] => check(jam),
        ["extract", jam, out] => extract(jam, out),
        ["extract-table",jam,group,table,out]=>extract_table(jam,group,table,out),
        _ => Err("usage: jam <list|check> <LEGO.JAM> | jam extract <LEGO.JAM> <out-dir>".into()),
    }
}

fn extract_table(path:&str,group:&str,name:&str,out:&str)->Result<(),String> {
    let jam=open(path)?;let table=jam.tables.iter().find(|t|t.group.eq_ignore_ascii_case(group)&&t.name.eq_ignore_ascii_case(name)).ok_or("archive table missing")?;
    let root=PathBuf::from(out);fs::create_dir(&root).map_err(|e|e.to_string())?;
    for entry in &table.entries {fs::write(root.join(&entry.name),jam.bytes(entry).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;}
    Ok(())
}

fn open(path: &str) -> Result<Jam, String> {
    Jam::open(path).map_err(|error| format!("{path}: {error}"))
}

fn list(path: &str) -> Result<(), String> {
    let jam = open(path)?;
    for table in &jam.tables {
        println!("[{}/{}] {} files", table.group, table.name, table.entries.len());
        for entry in &table.entries {
            println!("  {:<14} {:>10} {:>9}", entry.name, entry.offset, entry.size);
        }
    }
    Ok(())
}

fn check(path: &str) -> Result<(), String> {
    let jam = open(path)?;
    let mut files = 0usize;
    let mut bytes = 0u64;
    let mut bad = 0usize;
    for table in &jam.tables {
        for entry in &table.entries {
            files += 1;
            bytes += u64::from(entry.size);
            if jam.bytes(entry).is_err() {
                bad += 1;
                eprintln!("out of bounds: {}/{}/{}", table.group, table.name, entry.name);
            }
        }
    }
    println!(
        "{} tables, {files} files, {bytes} bytes indexed of {} ({bad} out of bounds)",
        jam.tables.len(),
        jam.len()
    );
    if bad == 0 { Ok(()) } else { Err(format!("{bad} entries are out of bounds")) }
}

fn extract(path: &str, out: &str) -> Result<(), String> {
    let jam = open(path)?;
    let root = PathBuf::from(out);
    for table in &jam.tables {
        let dir = root.join(&table.group).join(&table.name);
        fs::create_dir_all(&dir).map_err(|error| format!("{}: {error}", dir.display()))?;
        for entry in &table.entries {
            let data = jam.bytes(entry).map_err(|error| format!("{}: {error}", entry.name))?;
            let target = dir.join(&entry.name);
            fs::write(&target, data).map_err(|error| format!("{}: {error}", target.display()))?;
        }
    }
    Ok(())
}
