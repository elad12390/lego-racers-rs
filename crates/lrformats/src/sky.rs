//! Original SKB named environment palettes. Timed transitions are retained later.
use crate::tok::{self, Node};

#[derive(Debug)]
pub struct Profile {
    pub name: String,
    pub colors: [[u8; 3]; 3],
}

#[derive(Debug)]
pub struct Sky {
    pub default: String,
    pub profiles: Vec<Profile>,
}

pub fn parse(data: &[u8]) -> Result<Sky, String> {
    let nodes = tok::parse(data).map_err(|e| e.to_string())?;
    let default = nodes
        .windows(2)
        .find_map(|pair| match pair {
            [Node::Keyword(0x2d), Node::Str(name)] => Some(name.clone()),
            _ => None,
        })
        .ok_or("SKB lacks default environment")?;
    let body = nodes
        .iter()
        .find_map(|node| {
            if let Node::Block(body) = node {
                Some(body)
            } else {
                None
            }
        })
        .ok_or("SKB lacks profiles")?;
    let mut profiles = Vec::new();
    for triple in body.windows(3) {
        let [Node::Count(_), Node::Str(name), Node::Block(frames)] = triple else {
            continue;
        };
        let fields = frames
            .iter()
            .find_map(|node| {
                if let Node::Block(fields) = node {
                    Some(fields)
                } else {
                    None
                }
            })
            .ok_or("SKB profile lacks palette")?;
        let mut colors = [[0; 3]; 3];
        for (i, kind) in [0x17, 0x18, 0x19].into_iter().enumerate() {
            let values = fields
                .iter()
                .find_map(|n| match n {
                    Node::Record { kind: k, fields } if *k == kind => Some(fields),
                    _ => None,
                })
                .ok_or("SKB profile lacks a color")?;
            if values.len() != 3 {
                return Err("SKB palette color requires3channels".into());
            }
            for (channel, value) in colors[i].iter_mut().zip(values) {
                *channel = value
                    .as_u32()
                    .and_then(|v| u8::try_from(v).ok())
                    .ok_or("SKB invalid color channel")?;
            }
        }
        profiles.push(Profile {
            name: name.clone(),
            colors,
        });
    }
    if !profiles.iter().any(|p| p.name == default) {
        return Err("SKB default profile missing".into());
    }
    Ok(Sky { default, profiles })
}
