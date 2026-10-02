use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use lrformats::{bmp, pcm, wav};

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let result = match args.iter().map(String::as_str).collect::<Vec<_>>().as_slice() {
        ["pcm", input, output] => convert_one(Path::new(input), Path::new(output), write_wav),
        ["bmp", input, output] => convert_one(Path::new(input), Path::new(output), write_png),
        ["pcm-all", input_dir, output_dir] => {
            convert_all(Path::new(input_dir), Path::new(output_dir), "pcm", "wav", write_wav)
        }
        ["bmp-all", input_dir, output_dir] => {
            convert_all(Path::new(input_dir), Path::new(output_dir), "bmp", "png", write_png)
        }
        ["gdb-check", dir] => gdb_check(Path::new(dir)),
        ["render", jam, name, out, rest @ ..] => render_sheet(jam, name, out, rest.first().copied()),
        ["render-many", jam, out, names @ ..] => render_many(jam, out, names),
        ["checkpoints-check", jam] => checkpoints_check(jam),
        ["world-check", jam] => world_check(jam),
        ["world-map", jam, table, out] => world_map(jam, table, out),
        ["tok-dump", file] => tok_dump(Path::new(file)),
        ["gdb-info", files @ ..] => files.iter().try_for_each(|f| gdb_info(Path::new(f))),
        _ => Err("usage: lrconv <pcm|bmp> <in> <out> | <pcm-all|bmp-all> <dir> <out-dir> | gdb-check <dir> | gdb-info <files> | tok-dump <file>".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

type Writer = fn(&[u8], &Path) -> Result<(), String>;

fn write_wav(input: &[u8], output: &Path) -> Result<(), String> {
    let sound = pcm::decode(input).map_err(|e| e.to_string())?;
    fs::write(output, wav::encode_mono_16(sound.sample_rate, &sound.samples)).map_err(|e| e.to_string())
}

fn write_png(input: &[u8], output: &Path) -> Result<(), String> {
    let image = bmp::decode(input).map_err(|e| e.to_string())?;
    let file = fs::File::create(output).map_err(|e| e.to_string())?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), image.width.into(), image.height.into());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(&image.to_rgba()).map_err(|e| e.to_string())
}

fn convert_one(input: &Path, output: &Path, write: Writer) -> Result<(), String> {
    let bytes = fs::read(input).map_err(|e| format!("{}: {e}", input.display()))?;
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    write(&bytes, output).map_err(|e| format!("{}: {e}", input.display()))
}

fn convert_all(
    input_dir: &Path,
    output_dir: &Path,
    from_ext: &str,
    to_ext: &str,
    write: Writer,
) -> Result<(), String> {
    let mut files = Vec::new();
    collect(input_dir, from_ext, &mut files).map_err(|e| format!("{}: {e}", input_dir.display()))?;
    let (mut ok, mut failed) = (0usize, 0usize);
    for file in files {
        let relative = file.strip_prefix(input_dir).unwrap().with_extension(to_ext);
        match convert_one(&file, &output_dir.join(relative), write) {
            Ok(()) => ok += 1,
            Err(message) => {
                failed += 1;
                eprintln!("{message}");
            }
        }
    }
    println!("{ok} converted, {failed} failed");
    if failed == 0 { Ok(()) } else { Err(format!("{failed} files failed")) }
}

fn gdb_info(path: &Path) -> Result<(), String> {
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let name = path.file_name().unwrap().to_string_lossy();
    match lrformats::gdb::parse(&bytes) {
        Err(e) => println!("{name}: parse error: {e}"),
        Ok(mesh) => {
            println!(
                "{name}: {} vertices, {} triangles, scale {}, textures {:?}",
                mesh.vertices.len(),
                mesh.triangles.len(),
                mesh.scale,
                mesh.textures
            );
            let mut lo=[f32::INFINITY;3];let mut hi=[f32::NEG_INFINITY;3];
            for vertex in &mesh.vertices {for axis in 0..3 {let v=vertex.position[axis]*mesh.scale;lo[axis]=lo[axis].min(v);hi[axis]=hi[axis].max(v);}}
            println!("   bounds {lo:?} .. {hi:?}; {} source normals",mesh.normals.len());
            for (n, p) in mesh.parts.iter().enumerate().take(8) {
                let mut lo=[f32::INFINITY;3];let mut hi=[f32::NEG_INFINITY;3];
                for v in &mesh.vertices[p.vertices.start as usize..p.vertices.end as usize] {for axis in 0..3 {lo[axis]=lo[axis].min(v.position[axis]*mesh.scale);hi[axis]=hi[axis].max(v.position[axis]*mesh.scale);}}
                println!("   joint {} bounds {lo:?} .. {hi:?}",p.joint);
                println!(
                    "   part {n}: marker {} flag {} vertices {:?} (n={}) triangles {:?} max index {:?}",
                    p.texture,
                    p.flag,
                    p.vertices,
                    p.vertices.end - p.vertices.start,
                    p.triangles,
                    mesh.max_index(p)
                );
            }
            if mesh.parts.len() > 8 {
                println!("   ... {} parts total", mesh.parts.len());
            }
            if let Err(why) = mesh.validate() {
                println!("   INVALID: {why}");
            }
        }
    }
    Ok(())
}

/// Renders four views of a model into one PNG contact sheet.
fn render_sheet(jam: &str, name: &str, out: &str, table: Option<&str>) -> Result<(), String> {
    use lrformats::render::{render, Camera};
    let library = lrformats::library::Library::open(jam).map_err(|e| format!("{jam}: {e}"))?;
    let model = lrformats::model::Model::load(&library, name, table).map_err(|e| format!("{name}: {e}"))?;
    // Top-down first, then three oblique views from above.
    let views = [(0.0, 90.0), (35.0, 50.0), (155.0, 50.0), (275.0, 50.0)];
    let size = 360usize;
    let mut sheet = vec![0u8; size * 2 * size * 2 * 4];
    for (n, (azimuth_deg, elevation_deg)) in views.into_iter().enumerate() {
        let frame = render(&model, &Camera { azimuth_deg, elevation_deg }, size);
        let (ox, oy) = ((n % 2) * size, (n / 2) * size);
        for y in 0..size {
            let dst = ((oy + y) * size * 2 + ox) * 4;
            sheet[dst..dst + size * 4].copy_from_slice(&frame.rgba[y * size * 4..(y + 1) * size * 4]);
        }
    }
    let file = fs::File::create(out).map_err(|e| format!("{out}: {e}"))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), (size * 2) as u32, (size * 2) as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().and_then(|mut w| w.write_image_data(&sheet)).map_err(|e| e.to_string())?;
    println!(
        "{name}: {} parts, {} surfaces ({} textured), {} images",
        model.mesh.parts.len(),
        model.surfaces.len(),
        model.surfaces.iter().filter(|s| s.texture.is_some()).count(),
        model.images.len()
    );
    Ok(())
}

/// One view per model in a grid (6 columns); models that fail to load leave a dark tile.
fn render_many(jam: &str, out: &str, names: &[&str]) -> Result<(), String> {
    use lrformats::render::{render, Camera};
    let library = lrformats::library::Library::open(jam).map_err(|e| format!("{jam}: {e}"))?;
    let (cols, size) = (6usize, 200usize);
    let rows = names.len().div_ceil(cols);
    let width = cols * size;
    let mut sheet = vec![0u8; width * rows * size * 4];
    for pixel in sheet.chunks_exact_mut(4) {
        pixel.copy_from_slice(&[20, 20, 24, 255]);
    }
    for (n, name) in names.iter().enumerate() {
        match lrformats::model::Model::load(&library, name, None) {
            Ok(model) => {
                let frame = render(&model, &Camera { azimuth_deg: 0.0, elevation_deg: 90.0 }, size);
                let (ox, oy) = ((n % cols) * size, (n / cols) * size);
                for y in 0..size {
                    let dst = ((oy + y) * width + ox) * 4;
                    sheet[dst..dst + size * 4].copy_from_slice(&frame.rgba[y * size * 4..(y + 1) * size * 4]);
                }
            }
            Err(e) => eprintln!("{n:3} {name}: {e}"),
        }
    }
    let file = fs::File::create(out).map_err(|e| format!("{out}: {e}"))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), width as u32, (rows * size) as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().and_then(|mut w| w.write_image_data(&sheet)).map_err(|e| e.to_string())
}

/// Loads every checkpoint table with the ported original algorithm and checks the results:
/// the main route is a closed ring, every record gets a progress value, and progress rises
/// monotonically around the lap.
fn checkpoints_check(jam: &str) -> Result<(), String> {
    use lrformats::checkpoints::CheckpointTable;
    let library = lrformats::library::Library::open(jam).map_err(|e| format!("{jam}: {e}"))?;
    let mut bad = 0usize;
    for table in &library.jam().tables {
        for entry in table.entries.iter().filter(|e| e.name.to_ascii_uppercase().ends_with(".CPB")) {
            let data = library.jam().bytes(entry).map_err(|e| e.to_string())?;
            let loaded = CheckpointTable::load(data, false).map_err(|e| format!("{}: {e}", table.name))?;
            let route = loaded.main_route();
            let unassigned = loaded.records.iter().filter(|r| r.progress < 0.0).count();
            let monotonic = route.windows(2).all(|w| loaded.records[w[0]].progress < loaded.records[w[1]].progress);
            let closed = loaded.records[*route.last().unwrap()].next[0] == 0;
            let branches = loaded.records.iter().map(|r| r.next[1..].iter().filter(|&&n| n != 0xff).count()).sum::<usize>();
            let in_range = loaded.records.iter().all(|r| (0.0..=1.0).contains(&r.progress));
            let ok = unassigned == 0 && monotonic && closed && in_range;
            if !ok {
                bad += 1;
            }
            println!(
                "{:9} {:3} records, main route {:3}, {} fork(s), unassigned {unassigned}, monotonic {monotonic}, closed {closed}, in range {in_range} {}",
                table.name,
                loaded.records.len(),
                route.len(),
                branches,
                if ok { "OK" } else { "<-- CHECK" }
            );
        }
    }
    if bad == 0 { Ok(()) } else { Err(format!("{bad} tables failed the checks")) }
}

/// Parses every gameplay file of every table and reports how many succeed per file type.
fn world_check(jam: &str) -> Result<(), String> {
    use lrformats::world;
    let library = lrformats::library::Library::open(jam).map_err(|e| format!("{jam}: {e}"))?;
    let mut tally: std::collections::BTreeMap<String, (usize, usize, usize)> = Default::default();
    let mut failures = Vec::new();
    for table in &library.jam().tables {
        for entry in &table.entries {
            let ext = entry.name.rsplit('.').next().unwrap_or("").to_ascii_uppercase();
            let Ok(data) = library.jam().bytes(entry) else { continue };
            let (count, result): (usize, Result<usize, String>) = match ext.as_str() {
                "SPB" => (0, world::start_positions(data).map(|v| v.len()).map_err(|e| e.to_string())),
                "CPB" => (0, lrformats::checkpoints::CheckpointTable::load(data, false).map(|t| t.records.len()).map_err(|e| e.to_string())),
                "TRB" => (0, world::triggers(data).map(|v| v.len()).map_err(|e| e.to_string())),
                "PWB" => (0, world::pickups(data).map(|v| v.len()).map_err(|e| e.to_string())),
                "BVB" => (0, world::collision_mesh(data).map(|m| m.triangles.len()).map_err(|e| e.to_string())),
                "WDB" => (0, world::instances(data).map(|v| v.len()).map_err(|e| e.to_string())),
                _ => continue,
            };
            let slot = tally.entry(ext).or_default();
            match result {
                Ok(items) => {
                    slot.0 += 1;
                    slot.2 += items + count;
                }
                Err(why) => {
                    slot.1 += 1;
                    failures.push(format!("{}/{}: {why}", table.name, entry.name));
                }
            }
        }
    }
    for (ext, (ok, bad, items)) in &tally {
        println!(".{ext}: {ok} ok, {bad} failed, {items} items in total");
    }
    for line in failures.iter().take(10) {
        println!("  FAIL {line}");
    }
    if failures.is_empty() { Ok(()) } else { Err(format!("{} files failed", failures.len())) }
}

/// Top-down map of a race: the track mesh with collision, checkpoints, starts, triggers,
/// pickups and placed instances drawn over it.
fn world_map(jam: &str, table: &str, out: &str) -> Result<(), String> {
    use lrformats::render::{render_with, Camera, Projector};
    use lrformats::world;
    let library = lrformats::library::Library::open(jam).map_err(|e| format!("{jam}: {e}"))?;
    let entries: Vec<_> = library
        .jam()
        .tables
        .iter()
        .filter(|t| t.name.eq_ignore_ascii_case(table))
        .flat_map(|t| t.entries.iter())
        .collect();
    let read = |ext: &str| -> Vec<(String, &[u8])> {
        entries
            .iter()
            .filter(|e| e.name.to_ascii_uppercase().ends_with(ext))
            .filter_map(|e| Some((e.name.to_ascii_uppercase(), library.jam().bytes(e).ok()?)))
            .collect()
    };
    let track = read(".WDB")
        .iter()
        .find_map(|(_, data)| world::track_model(data))
        .ok_or("no track model named in any .WDB")?;
    let model = lrformats::model::Model::load(&library, &track, Some(table)).map_err(|e| format!("{track}: {e}"))?;
    let size = 1100usize;
    let projector = Projector::fit(&model, &Camera { azimuth_deg: 0.0, elevation_deg: 90.0 }, size);
    let mut frame = render_with(&model, &projector, size);
    for pixel in frame.rgba.chunks_exact_mut(4) {
        for c in &mut pixel[..3] {
            *c = (u16::from(*c) * 6 / 10) as u8;
        }
    }
    let at = |p: [f32; 3]| {
        let s = projector.project(p);
        [s[0], s[1]]
    };
    let mut counts = Vec::new();
    for (name, data) in read(".BVB") {
        if let Ok(mesh) = world::collision_mesh(data) {
            let color = if name.starts_with("COLLIDE") { [230, 60, 60] } else if name.starts_with("CHCKPT") { [60, 200, 230] } else { [200, 120, 255] };
            for t in &mesh.triangles {
                let v = t.indices.map(|i| at(mesh.vertices[i as usize]));
                frame.line(v[0], v[1], color);
                frame.line(v[1], v[2], color);
                frame.line(v[2], v[0], color);
            }
            counts.push(format!("{name} {} tris", mesh.triangles.len()));
        }
    }
    for (_, data) in read(".CPB") {
        if let Ok(table) = lrformats::checkpoints::CheckpointTable::load(data, false) {
            let route = table.main_route();
            for pair in route.windows(2) {
                frame.line(at(table.records[pair[0]].center), at(table.records[pair[1]].center), [255, 255, 0]);
            }
            for (n, c) in table.records.iter().enumerate() {
                let p = at(c.center);
                let on_route = route.contains(&n);
                frame.dot(p[0], p[1], 3, if on_route { [255, 255, 0] } else { [255, 140, 255] });
                // Gate plane: a segment along the plane across the road.
                let (nx, ny) = (c.plane[0], c.plane[1]);
                let tip = at([c.center[0] - ny * 30.0, c.center[1] + nx * 30.0, c.center[2]]);
                let tail = at([c.center[0] + ny * 30.0, c.center[1] - nx * 30.0, c.center[2]]);
                frame.line(tail, tip, [60, 200, 230]);
                for &fork in &c.next[1..] {
                    if fork != 0xff {
                        frame.line(p, at(table.records[usize::from(fork)].center), [255, 140, 255]);
                    }
                }
            }
            counts.push(format!("{} checkpoints ({} on the main route)", table.records.len(), route.len()));
        }
    }
    for (_, data) in read(".SPB") {
        if let Ok(list) = world::start_positions(data) {
            for s in &list {
                let p = at(s.position);
                frame.dot(p[0], p[1], 4, [60, 255, 60]);
                frame.line(p, at([s.position[0] + s.forward[0] * 40.0, s.position[1] + s.forward[1] * 40.0, s.position[2]]), [60, 255, 60]);
            }
            counts.push(format!("{} starts", list.len()));
        }
    }
    for (_, data) in read(".TRB") {
        if let Ok(list) = world::triggers(data) {
            for t in &list {
                let p = at(t.position);
                frame.dot(p[0], p[1], 3, [255, 140, 0]);
            }
            counts.push(format!("{} triggers", list.len()));
        }
    }
    for (_, data) in read(".PWB") {
        if let Ok(list) = world::pickups(data) {
            for k in &list {
                let p = at(k.position);
                frame.dot(p[0], p[1], 3, if k.kind == 0x2d { [255, 255, 255] } else { [255, 80, 200] });
            }
            counts.push(format!("{} pickups", list.len()));
        }
    }
    for (_, data) in read(".WDB") {
        if let Ok(list) = world::instances(data) {
            for i in &list {
                let p = at(i.position);
                frame.dot(p[0], p[1], 2, [200, 120, 255]);
            }
            counts.push(format!("{} instances", list.len()));
        }
    }
    let file = fs::File::create(out).map_err(|e| format!("{out}: {e}"))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), size as u32, size as u32);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header().and_then(|mut w| w.write_image_data(&frame.rgba)).map_err(|e| e.to_string())?;
    println!("{table} ({track}): {}", counts.join(", "));
    Ok(())
}

/// Prints the parsed token tree symbolically (long lists are truncated).
fn tok_dump(path: &Path) -> Result<(), String> {
    use lrformats::tok::{Node, Value};
    fn value(v: &Value) -> String {
        match v {
            Value::U8(x) => x.to_string(),
            Value::I8(x) => x.to_string(),
            Value::U16(x) => x.to_string(),
            Value::I16(x) => x.to_string(),
            Value::I32(x) => x.to_string(),
            Value::F32(x) => format!("{x:.3}"),
        }
    }
    fn row(r: &[Value]) -> String {
        r.iter().map(value).collect::<Vec<_>>().join(" ")
    }
    fn walk(nodes: &[Node], depth: usize) {
        let pad = "  ".repeat(depth);
        for node in nodes {
            match node {
                Node::Keyword(k) => println!("{pad}kw {k:#04x}"),
                Node::Str(s) => println!("{pad}str {s:?}"),
                Node::Float(f) => println!("{pad}float {f}"),
                Node::Int(i) => println!("{pad}int {i}"),
                Node::Count(n) => println!("{pad}[{n}]"),
                Node::List(items)=> {println!("{pad}[");walk(items,depth+1);println!("{pad}]");},
                Node::Comma | Node::Semi => {}
                Node::Block(body) => {
                    println!("{pad}{{");
                    walk(body, depth + 1);
                    println!("{pad}}}");
                }
                Node::Record { kind, fields } => println!("{pad}record {kind:#04x}: {}", row(fields)),
                Node::Packed { kind, rows } => {
                    println!("{pad}packed kind {kind:#04x} x{}", rows.len());
                    for r in rows.iter().take(6) {
                        println!("{pad}  {}", row(r));
                    }
                    if rows.len() > 6 {
                        println!("{pad}  ...");
                    }
                }
                Node::PackedStrings(list) => println!("{pad}strings {:?}", &list[..list.len().min(6)]),
                Node::StringRecord {kind,fields}=>println!("{pad}string record {kind:02x} {fields:?}"),
            }
        }
    }
    let bytes = fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    walk(&lrformats::tok::parse(&bytes).map_err(|e| e.to_string())?, 0);
    Ok(())
}

fn gdb_check(dir: &Path) -> Result<(), String> {
    let mut files = Vec::new();
    collect(dir, "gdb", &mut files).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut by_reason: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    let (mut ok, mut triangles, mut vertices) = (0usize, 0usize, 0usize);
    for file in &files {
        let name = file.file_name().unwrap().to_string_lossy().into_owned();
        let bytes = fs::read(file).map_err(|e| format!("{}: {e}", file.display()))?;
        match lrformats::gdb::parse(&bytes).map_err(|e| e.to_string()).and_then(|mesh| {
            mesh.validate()?;
            Ok(mesh)
        }) {
            Ok(mesh) => {
                ok += 1;
                triangles += mesh.triangles.len();
                vertices += mesh.vertices.len();
            }
            Err(reason) => by_reason.entry(reason).or_default().push(name),
        }
    }
    println!("{ok}/{} meshes valid ({vertices} vertices, {triangles} triangles)", files.len());
    for (reason, names) in by_reason.iter().take(8) {
        println!("  {:3} x {reason}   e.g. {}", names.len(), names.iter().take(3).cloned().collect::<Vec<_>>().join(", "));
    }
    if ok == files.len() { Ok(()) } else { Err(format!("{} meshes failed", files.len() - ok)) }
}

fn collect(dir: &Path, ext: &str, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(&path, ext, out)?;
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case(ext)) {
            out.push(path);
        }
    }
    Ok(())
}
