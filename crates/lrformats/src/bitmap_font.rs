//! Original FDB ordered glyph codes and palette-BMP glyph strips.
use crate::{tok::{self,Node,Value},bmp::Image};
pub struct FontSpec {pub name:String,pub characters:Vec<char>,pub spacing:i32}
pub struct Glyph {pub character:char,pub x:u16,pub width:u16}

pub fn parse(data:&[u8])->Result<Vec<FontSpec>,String> {
    let nodes=tok::parse(data).map_err(|e|e.to_string())?;
    let body=nodes.iter().find_map(|n|if let Node::Block(b)=n {Some(b)} else {None}).ok_or("missing original font table")?;
    let mut fonts=Vec::new();
    for row in body.windows(3) {
        let [Node::Keyword(0x27),Node::Str(name),Node::Block(fields)]=row else {continue;};
        let list=fields.windows(2).find_map(|pair|if let [Node::Keyword(0x2b),Node::List(items)]=pair {Some(items)} else {None}).ok_or("missing font character map")?;
        let mut characters=Vec::new();
        fn append(node:&Node,characters:&mut Vec<char>)->Result<(),String> {
            match node {
                Node::Str(s)=>characters.extend(s.chars()),
                Node::Int(i)=>characters.push(char::from_u32(*i as u32).ok_or("invalid font character")?),
                Node::PackedStrings(strings)=>for s in strings {characters.extend(s.chars());},
                Node::Packed {kind:4,rows}=>for row in rows {for v in row {if let Value::I32(i)=v {characters.push(char::from_u32(*i as u32).ok_or("invalid packed font code")?);} else {return Err("invalid font map value".into());}}},
                _=>return Err("unsupported original glyph map".into()),
            }
            Ok(())
        }
        for node in list {append(node,&mut characters)?;}
        let spacing=fields.windows(2).find_map(|p|if let [Node::Keyword(0x2c),Node::Int(i)]=p {Some(*i)} else {None}).unwrap_or(0);
        fonts.push(FontSpec {name:name.clone(),characters,spacing});
    }
    Ok(fonts)
}

pub fn glyphs(spec:&FontSpec,image:&Image)->Result<Vec<Glyph>,String> {
    let width=image.width as usize;let mut ranges=Vec::new();let mut begin=None;
    for x in 0..=width {
        let filled=x<width&&(0..image.height as usize).any(|y|image.palette[image.indices[y*width+x] as usize]!=[0,0,0]);
        if filled&&begin.is_none() {begin=Some(x);}
        if !filled {if let Some(start)=begin.take() {ranges.push((start,x-start));}}
    }
    // These original strips include control/punctuation slots that need the
    // complete original mapping dispatcher. Use only the verified shared
    // A-Z/0-9 prefix for now; never silently shift punctuation into wrong slots.
    let prefix=spec.characters.iter().take_while(|c|c.is_ascii_alphanumeric()).count();
    if prefix<36||ranges.len()<prefix {return Err(format!("font {} alphabet strip is incomplete",spec.name));}
    // FONT_THS is a one-to-one strip including punctuation and control glyphs.
    // Other layouts keep only the verified shared prefix, rather than shifting
    // symbols across the FONTMENU extra strip or opaque font backgrounds.
    let count=if ranges.len()==spec.characters.len() {spec.characters.len()}else {prefix};
    Ok(spec.characters.iter().take(count).zip(ranges).map(|(character,(x,width))|Glyph {character:*character,x:x as u16,width:width as u16}).collect())
}
