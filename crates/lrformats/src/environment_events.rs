//! EVB 0x42 named sky actions, distinct start and stop variants of the same id.
use crate::{event_table, named_records, tok::{self, Node}};

#[derive(Debug)]
pub struct Action {
  pub id: i32,
  pub stop: bool,
  pub name: String,
  pub transition_ms: u32,
  pub flags: u8,
}
pub fn parse(bytes: &[u8]) -> Result<Vec<Action>, String> {
  let nodes = tok::parse(bytes).map_err(|e| e.to_string())?;
  event_table::entries(&nodes, 0x42)?.into_iter().map(|entry| {
    let name = match named_records::value(entry.fields, 0x43) {
      Some(Node::Str(name)) => name.clone(), _ => return Err("invalid named sky event".into()),
    };
    let transition_ms = u32::try_from(named_records::integer(entry.fields, 0x44)?)
      .map_err(|_| "negative named sky transition")?;
    let flags = entry.fields.iter().fold(0, |flags, node| flags | match node {
      Node::Keyword(0x45) => 2, Node::Keyword(0x46) => 1,
      Node::Keyword(0x47) => 8, Node::Keyword(0x48) => 4, _ => 0,
    });
    Ok(Action {id: entry.id.ok_or("missing named sky event id")?, stop: entry.stop,
      name, transition_ms, flags})
  }).collect()
}
