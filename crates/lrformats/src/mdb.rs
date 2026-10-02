use crate::tok::{self, Node, TokError};

const KW_MATERIAL: u8 = 0x27;
const KW_TEXTURE_REF: u8 = 0x2c;

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Material {
    pub name: String,
    /// `(record kind, rgba)` colors, in file order. Kind 0x17 and 0x18 are the two colors every
    /// material carries.
    pub colors: Vec<(u8, [u8; 4])>,
    /// Name of the image (without extension) this material is textured with, if any.
    pub texture: Option<String>,
    /// Bound from the owning TDB RGB key, not ignored MDB flag 0x2b.
    pub color_key:Option<[u8;3]>,
    /// GolDP 100268xx stores these 0..10 blend-factor indices unchanged.
    pub blend:Option<[u8;2]>,
}

impl Material {
    /// The color used to tint or fill: the 0x17 record, else the first color, else white.
    pub fn base_color(&self) -> [u8; 4] {
        self.colors
            .iter()
            .find(|(k, _)| *k == 0x17)
            .or_else(|| self.colors.first())
            .map_or([255; 4], |(_, c)| *c)
    }
}

/// Parses a `.MDB` material database.
pub fn parse(data: &[u8]) -> Result<Vec<Material>, TokError> {
    let nodes = tok::parse(data)?;
    let Some(Node::Block(body)) = nodes.iter().find(|n| matches!(n, Node::Block(_))) else {
        return Ok(Vec::new());
    };
    let mut materials = Vec::new();
    let mut i = 0;
    while i + 2 < body.len() {
        if let (Node::Keyword(KW_MATERIAL), Node::Str(name), Node::Block(fields)) =
            (&body[i], &body[i + 1], &body[i + 2])
        {
            materials.push(material(name, fields));
            i += 3;
        } else {
            i += 1;
        }
    }
    Ok(materials)
}

fn material(name: &str, fields: &[Node]) -> Material {
    let mut out = Material { name: name.to_string(), ..Material::default() };
    out.blend=fields.windows(3).find_map(|v|if let [Node::Keyword(0x38),Node::Keyword(src),Node::Keyword(dst)]=v {if (0x39..=0x43).contains(src)&&(0x39..=0x43).contains(dst) {Some([src-0x39,dst-0x39])}else {None}}else {None});
    let mut previous = None;
    for node in fields {
        match node {
            Node::Record { kind, fields } => {
                if (*kind==0x17||*kind==0x18)&&fields.len()==4 {
                    let color=fields.iter().map(|v|v.as_u32().and_then(|i|u8::try_from(i).ok())).collect::<Option<Vec<_>>>();
                    if let Some(color)=color {out.colors.push((*kind,color.try_into().unwrap()));}
                }
            }
            Node::Str(texture) if previous == Some(&Node::Keyword(KW_TEXTURE_REF)) => {
                out.texture = Some(texture.clone());
            }
            _ => {}
        }
        previous = Some(node);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_colors_and_the_texture_reference() {
        let mut data = vec![0x16, 0x17, 4, 0x0c, 0x0c, 0x0c, 0x0c, 0x05];
        // material "m" { 17 10 20 30 255  2c "tex" }
        data.extend_from_slice(&[0x27, 2, b'm', 0, 5, 0x17, 10, 20, 30, 255, 0x2c, 2, b't', b'e', b'x', 0, 6]);
        data.push(6);
        let materials = parse(&data).unwrap();
        assert_eq!(materials.len(), 1);
        assert_eq!(materials[0].name, "m");
        assert_eq!(materials[0].base_color(), [10, 20, 30, 255]);
        assert_eq!(materials[0].texture.as_deref(), Some("tex"));
    }
}
