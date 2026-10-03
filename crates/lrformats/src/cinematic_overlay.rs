//! Original CEB text/fade descriptors and named CDB event activation ranges.
use crate::{
  cinematic::Event,
  cinematic_audio::records,
  library::Library,
  named_records,
  string_table::StringTable,
  tok::{self, Node},
};
#[derive(Debug)]
pub struct Text {
  pub name: String,
  pub value: String,
  pub font: String,
  pub x: Option<f32>,
  pub y: Option<f32>,
  pub start: u32,
  pub end: u32,
}
#[derive(Debug)]
pub struct Fade {
  pub color: [u8; 3],
  pub millis: u32,
  pub fade_in: bool,
  pub start: u32,
  pub end: Option<u32>,
}
pub struct Plan {
  pub texts: Vec<Text>,
  pub fades: Vec<Fade>,
}
fn float(fields: &[Node], key: u8) -> Result<Option<f32>, String> {
  match named_records::value(fields, key) {
    Some(Node::Float(v)) if v.is_finite() => Ok(Some(*v)),
    None => Ok(None),
    _ => Err("invalid CEB overlay coordinate".into()),
  }
}
pub fn load(
  library: &Library,
  table: &str,
  cdb: &str,
  events: &[Event],
  host: Option<&str>,
) -> Result<Plan, String> {
  let Some(bytes) = library.find_at(&format!("{cdb}.CEB"), "MENUDATA", table) else {
    return Ok(Plan {
      texts: Vec::new(),
      fades: Vec::new(),
    });
  };
  let nodes = tok::parse(bytes).map_err(|e| e.to_string())?;
  let texts = records(&nodes, 0x3f)?;
  let fades = records(&nodes, 0x60)?;
  let starts = records(&nodes, 0x56)?;
  let stops = records(&nodes, 0x57)?;
  let strings = if texts.is_empty() {
    None
  } else {
    Some(StringTable::parse(
      library
        .find_at("ENGLISH.SRF", "MENUDATA", table)
        .ok_or("missing cinematic English strings")?,
    )?)
  };
  let mut plan = Plan {
    texts: Vec::new(),
    fades: Vec::new(),
  };
  for event in events.iter().filter(|e| e.duration > 0) {
    for (_, binding) in starts
      .iter()
      .filter(|(name, _)| name.eq_ignore_ascii_case(&event.name))
    {
      let name = named_records::string(binding, 0x4e)?;
      if binding.contains(&Node::Keyword(0x3f)) {
        // WINCAR starts all named champion captions; the preview screen
        // enables only its selected champion's line, plus the common one.
        if table.eq_ignore_ascii_case("WINCAR")
          && name.to_ascii_lowercase().starts_with("text")
          && !name.eq_ignore_ascii_case("text1")
          && !host.is_some_and(|h| name.eq_ignore_ascii_case(&format!("text{h}")))
        {
          continue;
        }
        let fields = texts
          .iter()
          .find(|(n, _)| n.eq_ignore_ascii_case(&name))
          .ok_or("missing CEB named text")?
          .1;
        let (bank, index) = fields
          .windows(3)
          .find_map(|v| {
            if let [Node::Keyword(0x40), Node::Int(bank), Node::Int(index)] = v {
              Some((*bank, *index))
            } else {
              None
            }
          })
          .ok_or("missing CEB string reference")?;
        if bank != 0 {
          return Err("unsupported secondary CEB string bank".into());
        }
        let index = usize::try_from(index).map_err(|_| "negative CEB string index")?;
        plan.texts.push(Text {
          name,
          value: strings.as_ref().unwrap().get(index)?.into(),
          font: named_records::string(fields, 0x42)?,
          x: if fields.contains(&Node::Keyword(0x45)) {
            None
          } else {
            float(fields, 0x43)?
          },
          y: if fields.contains(&Node::Keyword(0x46)) {
            None
          } else {
            float(fields, 0x44)?
          },
          start: event.start,
          end: event.start.saturating_add(event.duration),
        });
      } else if binding.contains(&Node::Keyword(0x60)) {
        let fields = fades
          .iter()
          .find(|(n, _)| n.eq_ignore_ascii_case(&name))
          .ok_or("missing CEB named fade")?
          .1;
        let millis = u32::try_from(named_records::integer(fields, 0x61)?)
          .map_err(|_| "negative fade duration")?;
        if millis == 0 {
          return Err("zero CEB fade duration".into());
        }
        let color = if let Some(at) = fields.iter().position(|v| *v == Node::Keyword(0x66)) {
          fields
            .get(at + 1..at + 4)
            .ok_or("incomplete CEB fade color")?
            .iter()
            .map(|v| {
              if let Node::Int(i) = v {
                u8::try_from(*i).map_err(|_| "invalid CEB fade color")
              } else {
                Err("invalid CEB fade color")
              }
            })
            .collect::<Result<Vec<_>, _>>()?
            .try_into()
            .unwrap()
        } else {
          [0; 3]
        };
        let terminates = stops.iter().any(|(event_name, fields)| {
          event_name.eq_ignore_ascii_case(&event.name)
            && fields.contains(&Node::Keyword(0x60))
            && named_records::string(fields, 0x4f).is_ok_and(|n| n.eq_ignore_ascii_case(&name))
        });
        plan.fades.push(Fade {
          color,
          millis,
          fade_in: fields.contains(&Node::Keyword(0x63)),
          start: event.start,
          end: terminates.then_some(event.start.saturating_add(event.duration)),
        });
      }
    }
  }
  Ok(plan)
}
