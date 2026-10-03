//! Original FDB ordered glyph codes and palette-BMP glyph strips.
use crate::{
  bmp::Image,
  tok::{self, Node, Value},
};
pub struct FontSpec {
  pub name: String,
  pub characters: Vec<char>,
  pub spacing: i32,
  pub color_key: [u8; 3],
}
pub struct Glyph {
  pub character: char,
  pub x: u16,
  pub width: u16,
}

/// GolDP1001e190 counts columns identical to the first column and stores that
/// count as font+0x18, the space advance. Compare the complete column pattern,
/// not a guessed percentage of glyph height or a single background color.
pub fn leading_space_width(image: &Image) -> Result<u16, String> {
  if image.width == 0 || image.height == 0 {
    return Err("empty original font image".into());
  }
  let width = usize::from(image.width);
  for x in 1..width {
    if (0..usize::from(image.height)).any(|y| {
      if image.direct_rgba.is_none() {
        image.indices.get(y * width + x) != image.indices.get(y * width)
      } else {
        image.rgb(y * width + x) != image.rgb(y * width)
      }
    }) {
      return Ok(x as u16);
    }
  }
  Err("original font image has no non-background column".into())
}

#[cfg(test)]
mod metric_tests {
  use super::*;
  #[test]
  fn compares_whole_column_pattern_not_uniform_color_or_height() {
    let image = Image {
      width: 4,
      height: 2,
      palette: vec![[255, 255, 255], [0, 0, 0]],
      indices: vec![0, 0, 0, 1, 1, 1, 0, 1],
      direct_rgba: None,
    };
    assert_eq!(leading_space_width(&image), Ok(2));
  }
  #[test]
  fn rejects_empty_or_pattern_only_strip() {
    let mut image = Image {
      width: 0,
      height: 2,
      palette: vec![[0, 0, 0]],
      indices: vec![],
      direct_rgba: None,
    };
    assert!(leading_space_width(&image).is_err());
    image.width = 2;
    image.indices = vec![0; 4];
    assert!(leading_space_width(&image).is_err());
  }

  #[test]
  fn palette_aliases_do_not_hide_original_pixel_index_changes() {
    let image = Image {
      width: 2,
      height: 1,
      palette: vec![[0, 0, 0], [0, 0, 0]],
      indices: vec![0, 1],
      direct_rgba: None,
    };
    assert_eq!(leading_space_width(&image), Ok(1));
  }

  #[test]
  #[ignore = "requires original LEGO.JAM via LR_TEST_JAM"]
  fn font_ths_original_asset_metrics_match_original_prefixes() {
    let library = crate::library::Library::open(std::env::var("LR_TEST_JAM").unwrap()).unwrap();
    let image = crate::bmp::decode(
      library
        .find_at("FONT_THS.BMP", "MENUDATA", "ENGLISH")
        .unwrap(),
    )
    .unwrap();
    let fonts = parse(
      library
        .find_at("GFONTS.FDB", "MENUDATA", "ENGLISH")
        .unwrap(),
    )
    .unwrap();
    let spec = fonts
      .iter()
      .find(|font| font.name.eq_ignore_ascii_case("font_ths"))
      .unwrap();
    assert_eq!(leading_space_width(&image), Ok(8));
    assert_eq!(image.height, 32);
    assert_eq!(spec.spacing, 0);
    let glyphs = glyphs(spec, &image).unwrap();
    for (label, expected) in [("CONTINUE", 106), ("CANCEL", 80)] {
      let measured = label
        .chars()
        .map(|c| u32::from(glyphs.iter().find(|g| g.character == c).unwrap().width))
        .sum::<u32>();
      assert_eq!(measured, expected);
    }
  }
}

pub fn parse(data: &[u8]) -> Result<Vec<FontSpec>, String> {
  let nodes = tok::parse(data).map_err(|e| e.to_string())?;
  let body = nodes
    .iter()
    .find_map(|n| {
      if let Node::Block(b) = n {
        Some(b)
      } else {
        None
      }
    })
    .ok_or("missing original font table")?;
  let mut fonts = Vec::new();
  for row in body.windows(3) {
    let [Node::Keyword(0x27), Node::Str(name), Node::Block(fields)] = row else {
      continue;
    };
    let list = fields
      .windows(2)
      .find_map(|pair| {
        if let [Node::Keyword(0x2b), Node::List(items)] = pair {
          Some(items)
        } else {
          None
        }
      })
      .ok_or("missing font character map")?;
    let mut characters = Vec::new();
    fn append(node: &Node, characters: &mut Vec<char>) -> Result<(), String> {
      match node {
        Node::Str(s) => characters.extend(s.chars()),
        Node::Int(i) => characters.push(char::from_u32(*i as u32).ok_or("invalid font character")?),
        Node::PackedStrings(strings) => {
          for s in strings {
            characters.extend(s.chars());
          }
        }
        Node::Packed { kind: 4, rows } => {
          for row in rows {
            for v in row {
              if let Value::I32(i) = v {
                characters.push(char::from_u32(*i as u32).ok_or("invalid packed font code")?);
              } else {
                return Err("invalid font map value".into());
              }
            }
          }
        }
        _ => return Err("unsupported original glyph map".into()),
      }
      Ok(())
    }
    for node in list {
      append(node, &mut characters)?;
    }
    let spacing = fields
      .windows(2)
      .find_map(|p| {
        if let [Node::Keyword(0x2c), Node::Int(i)] = p {
          Some(*i)
        } else {
          None
        }
      })
      .unwrap_or(0);
    let color_key = fields
      .windows(4)
      .find_map(|row| {
        if let [Node::Keyword(0x2a), Node::Int(r), Node::Int(g), Node::Int(b)] = row {
          Some(
            [*r, *g, *b]
              .map(u8::try_from)
              .into_iter()
              .collect::<Result<Vec<_>, _>>(),
          )
        } else {
          None
        }
      })
      .ok_or("missing original font color key")?
      .map_err(|_| "invalid original font color key")?;
    fonts.push(FontSpec {
      name: name.clone(),
      characters,
      spacing,
      color_key: color_key.try_into().unwrap(),
    });
  }
  Ok(fonts)
}

pub fn glyphs(spec: &FontSpec, image: &Image) -> Result<Vec<Glyph>, String> {
  let width = image.width as usize;
  let mut ranges = Vec::new();
  let mut begin = None;
  for x in 0..=width {
    let filled = x < width
      && (0..image.height as usize)
        .any(|y| image.palette[image.indices[y * width + x] as usize] != spec.color_key);
    if filled && begin.is_none() {
      begin = Some(x);
    }
    if !filled {
      if let Some(start) = begin.take() {
        ranges.push((start, x - start));
      }
    }
  }
  // These original strips include control/punctuation slots that need the
  // complete original mapping dispatcher. Use only the verified shared
  // A-Z/0-9 prefix for now; never silently shift punctuation into wrong slots.
  let prefix = spec
    .characters
    .iter()
    .take_while(|c| c.is_ascii_alphanumeric())
    .count();
  if prefix < 36 || ranges.len() < prefix {
    return Err(format!("font {} alphabet strip is incomplete", spec.name));
  }
  // FONT_THS is a one-to-one strip including punctuation and control glyphs.
  // Other layouts keep only the verified shared prefix, rather than shifting
  // symbols across the FONTMENU extra strip or opaque font backgrounds.
  let count = if ranges.len() == spec.characters.len() {
    spec.characters.len()
  } else {
    prefix
  };
  Ok(
    spec
      .characters
      .iter()
      .take(count)
      .zip(ranges)
      .map(|(character, (x, width))| Glyph {
        character: *character,
        x: x as u16,
        width: width as u16,
      })
      .collect(),
  )
}
