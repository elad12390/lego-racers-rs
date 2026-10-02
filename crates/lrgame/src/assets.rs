use lrformats::library::Library;
use lrformats::model::Model;
use lrformats::world;

use crate::options::Options;

pub fn load_track(options: &Options) -> Result<Model, String> {
    let library = Library::open(&options.jam)
        .map_err(|error| format!("{}: {error}", options.jam.display()))?;
    let table = library
        .jam()
        .tables
        .iter()
        .find(|table| table.name.eq_ignore_ascii_case(&options.table))
        .ok_or_else(|| format!("race table {} is absent from original JAM", options.table))?;
    let (name, world_data) = table
        .entries
        .iter()
        .filter(|entry| entry.name.to_ascii_uppercase().ends_with(".WDB"))
        .find_map(|entry| {
            let data = library.jam().bytes(entry).ok()?;
            Some((world::track_model(data)?, data))
        })
        .ok_or_else(|| format!("race table {} has no original track model", options.table))?;
    let bindings = lrformats::scene::SceneBindings::load(&library, &options.table, world_data)?;
    let model =
        Model::load_with_materials(&library, &name, Some(&options.table), &bindings.materials)
            .map_err(|error| error.to_string())?;
    if model.mesh.vertices.is_empty()
        || model
            .surfaces
            .iter()
            .all(|surface| surface.triangles.is_empty())
    {
        return Err(format!(
            "original track model {name} has no drawable geometry"
        ));
    }
    if !model.mesh.scale.is_finite()
        || model
            .mesh
            .vertices
            .iter()
            .flat_map(|vertex| vertex.position)
            .any(|value| !value.is_finite())
    {
        return Err(format!(
            "original track model {name} contains nonfinite geometry"
        ));
    }
    println!(
        "{}: original track={name}, vertices={}, triangles={}, textures={}",
        options.table,
        model.mesh.vertices.len(),
        model
            .surfaces
            .iter()
            .map(|surface| surface.triangles.len())
            .sum::<usize>(),
        model.images.len()
    );
    Ok(model)
}
