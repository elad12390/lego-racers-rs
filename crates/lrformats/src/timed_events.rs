//! Original TIB active/inactive periods, optional random ranges and initial delay.
use crate::{event_table, named_records, tok::{self, Node}};

#[derive(Clone, Debug)]
pub struct Period {pub milliseconds: u32, pub random: bool}
#[derive(Clone, Debug)]
pub struct Timer {
  pub id: i32,
  pub active: Period,
  pub inactive: Period,
  pub delay_ms: u32,
}
fn period(fields: &[Node], key: u8) -> Result<Period, String> {
  let at = fields.iter().position(|n| *n == Node::Keyword(key)).ok_or("missing TIB period")?;
  let random = fields.get(at + 1) == Some(&Node::Keyword(0x2b));
  let Some(Node::Int(value)) = fields.get(at + 1 + usize::from(random)) else { return Err("invalid TIB period".into()); };
  let milliseconds = u32::try_from(*value).map_err(|_| "negative TIB period")?;
  if random && milliseconds == 0 { return Err("zero random TIB range".into()); }
  Ok(Period {milliseconds, random})
}
pub fn parse(bytes: &[u8]) -> Result<Vec<Timer>, String> {
  let nodes = tok::parse(bytes).map_err(|e| e.to_string())?;
  event_table::entries(&nodes, 0x27)?.into_iter().map(|entry| {
    if entry.id.is_some() || entry.stop { return Err("invalid TIB timer framing".into()); }
    let delay_ms = match named_records::value(entry.fields, 0x2d) {
      None => 0,
      Some(Node::Int(value)) => u32::try_from(*value).map_err(|_| "negative TIB delay")?,
      _ => return Err("invalid TIB delay".into()),
    };
    Ok(Timer {id: named_records::integer(entry.fields, 0x2a)?,
      active: period(entry.fields, 0x28)?, inactive: period(entry.fields, 0x29)?, delay_ms})
  }).collect()
}
