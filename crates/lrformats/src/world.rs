//! Readers for the per-race gameplay files: start grid, checkpoints, triggers, pickups,
//! collision meshes and the world instance list. Built on the tokenized container in [`tok`].

use std::fmt;

use crate::tok::{self, Node, TokError, Value};

pub type Vec3 = [f32; 3];

#[derive(Debug)]
pub enum WorldError {
    Tok(TokError),
    Shape(String),
}

impl fmt::Display for WorldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WorldError::Tok(e) => write!(f, "{e}"),
            WorldError::Shape(why) => write!(f, "unexpected layout: {why}"),
        }
    }
}

impl std::error::Error for WorldError {}

impl From<TokError> for WorldError {
    fn from(error: TokError) -> Self {
        WorldError::Tok(error)
    }
}

fn shape<T>(why: &str) -> Result<T, WorldError> {
    Err(WorldError::Shape(why.to_string()))
}

const KW_ENTRY: u8 = 0x27;

/// The `{ ... }` bodies of the top-level statements, keyed by their keyword byte.
fn statements(nodes: &[Node]) -> Vec<(u8, &[Node])> {
    let mut out = Vec::new();
    let mut keyword = None;
    for node in nodes {
        match node {
            Node::Keyword(k) => keyword = Some(*k),
            Node::Block(body) => {
                if let Some(k) = keyword.take() {
                    out.push((k, body.as_slice()));
                }
            }
            _ => {}
        }
    }
    out
}

fn statement<'a>(nodes: &'a [Node], keyword: u8) -> Option<&'a [Node]> {
    statements(nodes).into_iter().find(|(k, _)| *k == keyword).map(|(_, body)| body)
}

/// Entries of the form `27 [label] { fields }` inside a body: the label (if any) and the fields.
fn entries(body: &[Node]) -> Vec<(Option<&Node>, &[Node])> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < body.len() {
        if body[i] == Node::Keyword(KW_ENTRY) {
            let (label, block) = match (body.get(i + 1), body.get(i + 2)) {
                (Some(Node::Block(b)), _) => (None, Some(b)),
                (Some(label), Some(Node::Block(b))) => (Some(label), Some(b)),
                _ => (None, None),
            };
            if let Some(block) = block {
                out.push((label, block.as_slice()));
            }
        }
        i += 1;
    }
    out
}

fn record(fields: &[Node], kind: u8) -> Option<&[Value]> {
    fields.iter().find_map(|n| match n {
        Node::Record { kind: k, fields } if *k == kind => Some(fields.as_slice()),
        _ => None,
    })
}

fn floats<const N: usize>(values: &[Value]) -> Option<[f32; N]> {
    let mut out = [0.0; N];
    for (slot, v) in out.iter_mut().zip(values) {
        *slot = v.as_f32()?;
    }
    (values.len() >= N).then_some(out)
}

/// The node after the first occurrence of `keyword`.
fn after(fields: &[Node], keyword: u8) -> Option<&Node> {
    let at = fields.iter().position(|n| *n == Node::Keyword(keyword))?;
    fields.get(at + 1)
}

fn number(node: Option<&Node>) -> Option<f32> {
    match node? {
        Node::Float(f) => Some(*f),
        Node::Int(i) => Some(*i as f32),
        _ => None,
    }
}

fn packed_floats(node: Option<&Node>) -> Option<Vec<f32>> {
    match node? {
        Node::Packed { rows, .. } => rows.iter().map(|r| r.first()?.as_f32()).collect(),
        _ => None,
    }
}

// ---- start grid (.SPB) -------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct StartPosition {
    pub slot: i32,
    pub position: Vec3,
    pub forward: Vec3,
    pub up: Vec3,
}

pub fn start_positions(data: &[u8]) -> Result<Vec<StartPosition>, WorldError> {
    let nodes = tok::parse(data)?;
    let body = statement(&nodes, KW_ENTRY).ok_or(WorldError::Shape("no start list".into()))?;
    entries(body)
        .into_iter()
        .map(|(label, fields)| {
            let slot = match label {
                Some(Node::Int(slot)) => *slot,
                _ => return shape("start without a slot number"),
            };
            let position = record(fields, 0x17).and_then(floats::<3>);
            let orientation = record(fields, 0x18).and_then(floats::<6>);
            match (position, orientation) {
                (Some(position), Some(o)) => Ok(StartPosition {
                    slot,
                    position,
                    forward: [o[0], o[1], o[2]],
                    up: [o[3], o[4], o[5]],
                }),
                _ => shape("start without position and orientation"),
            }
        })
        .collect()
}

// ---- triggers (.TRB) ---------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Trigger {
    pub position: Vec3,
    pub radius: f32,
    pub id: i32,
    pub model: Option<String>,
}

pub fn triggers(data: &[u8]) -> Result<Vec<Trigger>, WorldError> {
    let nodes = tok::parse(data)?;
    let body = statement(&nodes, KW_ENTRY).ok_or(WorldError::Shape("no trigger list".into()))?;
    entries(body)
        .into_iter()
        .map(|(_, fields)| {
            let position = record(fields, 0x17).and_then(floats::<3>).ok_or(WorldError::Shape("trigger position".into()))?;
            Ok(Trigger {
                position,
                radius: number(after(fields, 0x2a)).unwrap_or(0.0),
                id: number(after(fields, 0x2b)).unwrap_or(0.0) as i32,
                model: match after(fields, 0x2d) {
                    Some(Node::Str(name)) => Some(name.clone()),
                    _ => None,
                },
            })
        })
        .collect()
}

// ---- pickups (.PWB) ----------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Pickup {
    pub position: Vec3,
    /// The keyword byte that follows the position (0x2c / 0x2d select the pickup kind).
    pub kind: u8,
}

pub fn pickups(data: &[u8]) -> Result<Vec<Pickup>, WorldError> {
    let nodes = tok::parse(data)?;
    let body = statement(&nodes, KW_ENTRY).ok_or(WorldError::Shape("no pickup list".into()))?;
    entries(body)
        .into_iter()
        .map(|(_, fields)| {
            let position = record(fields, 0x17).and_then(floats::<3>).ok_or(WorldError::Shape("pickup position".into()))?;
            let kind = fields
                .iter()
                .find_map(|n| match n {
                    Node::Keyword(k) => Some(*k),
                    _ => None,
                })
                .ok_or(WorldError::Shape("pickup kind".into()))?;
            Ok(Pickup { position, kind })
        })
        .collect()
}

// ---- collision mesh (.BVB) ---------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct CollisionTriangle {
    pub indices: [u32; 3],
    /// The fourth value of each triangle record; selects the surface type.
    pub surface: u32,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct CollisionMesh {
    pub names: Vec<String>,
    pub vertices: Vec<Vec3>,
    pub triangles: Vec<CollisionTriangle>,
}

pub fn collision_mesh(data: &[u8]) -> Result<CollisionMesh, WorldError> {
    let nodes = tok::parse(data)?;
    let mut names = Vec::new();
    if let Some(body) = statement(&nodes, KW_ENTRY) {
        for node in body {
            match node {
                Node::Str(s) => names.push(s.clone()),
                Node::PackedStrings(list) => names.extend(list.iter().cloned()),
                _ => {}
            }
        }
    }
    let flat = statement(&nodes, 0x34)
        .and_then(|body| body.iter().find_map(|n| packed_floats(Some(n))))
        .unwrap_or_default();
    if flat.len() % 3 != 0 {
        return shape("vertex floats are not a multiple of 3");
    }
    let vertices: Vec<Vec3> = flat.chunks_exact(3).map(|c| [c[0], c[1], c[2]]).collect();

    let mut triangles = Vec::new();
    for node in statement(&nodes, 0x2d).unwrap_or_default() {
        let rows: Vec<&[Value]> = match node {
            Node::Packed { rows, .. } => rows.iter().map(Vec::as_slice).collect(),
            Node::Record { fields, .. } => vec![fields.as_slice()],
            _ => continue,
        };
        for row in rows {
            let ids: Vec<u32> = row.iter().filter_map(|v| v.as_u32()).collect();
            if ids.len() < 3 {
                return shape("collision triangle with fewer than three indices");
            }
            if ids[..3].iter().any(|&i| i as usize >= vertices.len()) {
                return shape("collision triangle indexes past the vertex list");
            }
            triangles.push(CollisionTriangle {
                indices: [ids[0], ids[1], ids[2]],
                surface: ids.get(3).copied().unwrap_or(0),
            });
        }
    }
    Ok(CollisionMesh { names, vertices, triangles })
}

/// Name of the main track model: the first string of a `.WDB`'s `2d` statement.
pub fn track_model(data: &[u8]) -> Option<String> {
    let nodes = tok::parse(data).ok()?;
    statement(&nodes, 0x2d)?.iter().find_map(|n| match n {
        Node::Str(name) => Some(name.clone()),
        Node::PackedStrings(list) => list.first().cloned(),
        _ => None,
    })
}

// ---- world instances (.WDB) --------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub struct Instance {
    pub model: String,
    pub position: Vec3,
    pub forward: Vec3,
    pub up: Vec3,
}

/// Models placed in the world: the `2e` statement of a `.WDB`.
pub fn instances(data: &[u8]) -> Result<Vec<Instance>, WorldError> {
    instances_from(data, 0x2e, 0x2a)
}

/// Original collision-mesh placements use41 entries and40 model references.
pub fn collision_instances(data: &[u8]) -> Result<Vec<Instance>, WorldError> {
    instances_from(data, 0x41, 0x40)
}

fn instances_from(data: &[u8], entry_keyword: u8, model_keyword: u8) -> Result<Vec<Instance>, WorldError> {
    let nodes = tok::parse(data)?;
    let Some(body) = statement(&nodes, entry_keyword) else { return Ok(Vec::new()) };
    let model_names: Vec<_> = statement(&nodes, model_keyword).into_iter().flatten().flat_map(|n| match n {
        Node::Str(s) => vec![s.clone()],
        Node::PackedStrings(list) => list.clone(),
        _ => Vec::new(),
    }).collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i + 2 < body.len() {
        if let (Node::Keyword(k), Node::Str(label), Node::Block(fields)) = (&body[i], &body[i + 1], &body[i + 2]) {
            if *k != entry_keyword { i += 1; continue; }
            let model = match after(fields, model_keyword) {
                Some(Node::Int(index)) => model_names.get(*index as usize).ok_or_else(|| WorldError::Shape(format!("instance {label}: invalid model reference {index}")))?,
                _ => label,
            };
            let position = record(fields, 0x17).and_then(floats::<3>);
            let orientation = record(fields, 0x18).and_then(floats::<6>);
            if let (Some(position), Some(o)) = (position, orientation) {
                out.push(Instance {
                    model: model.clone(),
                    position,
                    forward: [o[0], o[1], o[2]],
                    up: [o[3], o[4], o[5]],
                });
            }
            i += 3;
        } else {
            i += 1;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn floats_bytes(values: &[f32]) -> Vec<u8> {
        values.iter().flat_map(|v| v.to_le_bytes()).collect()
    }

    fn layouts() -> Vec<u8> {
        let mut data = vec![0x16, 0x17, 3, 3, 3, 3];
        data.extend_from_slice(&[0x16, 0x18, 6, 3, 3, 3, 3, 3, 3]);
        data
    }

    #[test]
    fn reads_start_slots_with_position_and_orientation() {
        let mut data = layouts();
        // 27 [1] { 27 int 7 { 17 pos 18 ori } }
        data.extend_from_slice(&[0x27, 7, 4, 1, 0, 0, 0, 8, 5]);
        data.extend_from_slice(&[0x27, 4, 7, 0, 0, 0, 5, 0x17]);
        data.extend_from_slice(&floats_bytes(&[1.0, 2.0, 3.0]));
        data.push(0x18);
        data.extend_from_slice(&floats_bytes(&[0.0, 1.0, 0.0, 0.0, 0.0, 1.0]));
        data.extend_from_slice(&[6, 6]);
        let starts = start_positions(&data).unwrap();
        assert_eq!(
            starts,
            vec![StartPosition { slot: 7, position: [1.0, 2.0, 3.0], forward: [0.0, 1.0, 0.0], up: [0.0, 0.0, 1.0] }]
        );
    }

    #[test]
    fn reads_triggers_with_radius_id_and_model() {
        let mut data = layouts();
        data.extend_from_slice(&[0x27, 7, 4, 1, 0, 0, 0, 8, 5, 0x27, 5, 0x17]);
        data.extend_from_slice(&floats_bytes(&[9.0, 8.0, 7.0]));
        data.extend_from_slice(&[0x2a, 3]);
        data.extend_from_slice(&floats_bytes(&[12.5]));
        data.extend_from_slice(&[0x2b, 4, 0x09, 0x04, 0, 0]);
        data.extend_from_slice(&[0x2d, 2, b'm', 0, 6, 6]);
        let list = triggers(&data).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!((list[0].radius, list[0].id), (12.5, 1033));
        assert_eq!(list[0].position, [9.0, 8.0, 7.0]);
        assert_eq!(list[0].model.as_deref(), Some("m"));
    }

    #[test]
    fn collision_mesh_rejects_out_of_range_indices() {
        let mut data = vec![0x16, 0x17, 4, 0x0c, 0x0c, 0x0c, 0x0c];
        // vertices: 34 [1] { 14 count=3 kind=3 floats }
        data.extend_from_slice(&[0x34, 7, 4, 1, 0, 0, 0, 8, 5, 0x14, 9, 0, 3]);
        data.extend_from_slice(&floats_bytes(&[0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]));
        data.push(6);
        // one triangle 0 1 2 with surface 40
        let mut ok = data.clone();
        ok.extend_from_slice(&[0x2d, 7, 4, 1, 0, 0, 0, 8, 5, 0x17, 0, 1, 2, 40, 6]);
        assert_eq!(collision_mesh(&ok).unwrap().triangles, vec![CollisionTriangle { indices: [0, 1, 2], surface: 40 }]);
        let mut bad = data;
        bad.extend_from_slice(&[0x2d, 7, 4, 1, 0, 0, 0, 8, 5, 0x17, 0, 1, 9, 40, 6]);
        assert!(collision_mesh(&bad).is_err());
    }
}
