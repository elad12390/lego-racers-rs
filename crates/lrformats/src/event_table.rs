//! Strict framing for counted EVB/TIB entry lists, including stop-only EVB rows.
use crate::tok::Node;

pub(crate) struct Entry<'a> {
  pub id: Option<i32>,
  pub stop: bool,
  pub fields: &'a [Node],
}
pub(crate) fn entries(nodes: &[Node], key: u8) -> Result<Vec<Entry<'_>>, String> {
  let Some((count, body)) = nodes.windows(3).find_map(|v| match v {
    [Node::Keyword(k), Node::Count(count), Node::Block(body)] if *k == key => Some((*count, body)),
    _ => None,
  }) else { return Ok(Vec::new()); };
  let mut rows = Vec::new();
  let mut at = 0;
  while at < body.len() {
    if body.get(at) != Some(&Node::Keyword(0x27)) { return Err("invalid event table entry".into()); }
    at += 1;
    let id = if let Some(Node::Int(id)) = body.get(at) { at += 1; Some(*id) } else { None };
    let stop = body.get(at) == Some(&Node::Keyword(0x3c));
    if stop { at += 1; }
    let Some(Node::Block(fields)) = body.get(at) else { return Err("missing event entry fields".into()); };
    rows.push(Entry {id, stop, fields});
    at += 1;
  }
  if rows.len() != count as usize { return Err("event table count mismatch".into()); }
  Ok(rows)
}
