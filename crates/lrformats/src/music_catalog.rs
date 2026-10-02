//! Race-local LEGOMSC counted list; indices are used by Game music states.
pub fn parse(bytes:&[u8])->Result<Vec<String>,String> {
    let text=std::str::from_utf8(bytes).map_err(|_|"non-UTF8 music catalog")?;
    let mut lines=text.lines().map(str::trim).filter(|line|!line.is_empty());
    let count:usize=lines.next().ok_or("missing music count")?.parse().map_err(|_|"invalid music count")?;
    let names:Vec<_>=lines.map(str::to_owned).collect();
    if count!=names.len() || count<4 {return Err("music count mismatch or missing state cues".into());}
    if names.iter().any(|name|name.contains(['/', '\\']) || !name.to_ascii_lowercase().ends_with(".tun")) {return Err("invalid tune filename".into());}
    Ok(names)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::library::Library;
    #[test]
    fn every_race_list_resolves_four_state_cues_and_all_original_external_tunes() {
        let directory=std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extracted/Program_Files_Group");
        let library=Library::open(directory.join("LEGO.JAM")).unwrap();
        let paths:std::collections::HashMap<_,_>=std::fs::read_dir(&directory).unwrap().map(|e| {
            let e=e.unwrap();(e.file_name().to_string_lossy().to_ascii_lowercase(),e.path())
        }).collect();
        let mut count=0;
        for owner in library.jam().tables.iter().filter(|t|t.name.starts_with("RACEC")) {
            let names=parse(library.find_in("LEGOMSC",&owner.name).unwrap()).unwrap();
            assert_eq!(&names[..4],&["start.tun",names[1].as_str(),"lose.tun","win.tun"]);
            for name in names {let bytes=std::fs::read(&paths[&name.to_ascii_lowercase()]).unwrap();assert_eq!(&bytes[..4],b"ALP ");}
            count+=1;
        }
        assert_eq!(count,13);
    }
    #[test]
    fn bad_counts_and_nonlocal_filenames_are_rejected() {
        assert!(parse(b"4\nstart.tun\nrace.tun\nlose.tun\n").is_err());
        assert!(parse(b"4\n../start.tun\nrace.tun\nlose.tun\nwin.tun\n").is_err());
    }
}
