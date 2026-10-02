//! Native GPU draws using the game's original English bitmap fonts.
use lrformats::{library::Library,bitmap_font::{self,Glyph},bmp};
use macroquad::prelude::*;
use std::collections::HashMap;
pub struct BitmapText {texture:Texture2D,glyphs:HashMap<char,Glyph>,height:f32,spacing:f32}
impl BitmapText {
    pub fn load(library:&Library,name:&str)->Result<Self,String> {
        let fonts=bitmap_font::parse(library.find_at("GFONTS.FDB","MENUDATA","ENGLISH").ok_or("missing original menu fonts")?)?;
        let spec=fonts.iter().find(|f|f.name.eq_ignore_ascii_case(name)).ok_or("original font not in FDB")?;
        let image=bmp::decode(library.find_at(&format!("{}.BMP",spec.name),"MENUDATA","ENGLISH").ok_or("missing original font image")?).map_err(|e|e.to_string())?;
        let glyphs=bitmap_font::glyphs(spec,&image)?.into_iter().map(|g|(g.character,g)).collect();
        let mut bytes=image.to_rgba();for pixel in bytes.chunks_exact_mut(4) {if pixel[..3]==[0,0,0] {pixel[3]=0;}}
        let texture=Texture2D::from_rgba8(image.width,image.height,&bytes);texture.set_filter(FilterMode::Nearest);
        Ok(Self {texture,glyphs,height:image.height as f32,spacing:spec.spacing as f32})
    }
    pub fn supports(&self,text:&str)->bool {text.chars().all(|c|c==' '||self.glyphs.contains_key(&c.to_ascii_uppercase()))}
    pub fn native_height(&self)->f32 {self.height}
    pub fn width(&self,text:&str,size:f32)->f32 {
        text.chars().map(|c|if c==' ' {size*0.45} else {self.glyphs.get(&c.to_ascii_uppercase()).map_or(0.0,|g|(g.width as f32+self.spacing)*size/self.height)}).sum()
    }
    pub fn draw(&self,text:&str,mut x:f32,y:f32,size:f32,color:Color) {
        let scale=size/self.height;
        for character in text.chars().map(|c|c.to_ascii_uppercase()) {
            if character==' ' {x+=size*0.45;continue;}
            if let Some(glyph)=self.glyphs.get(&character).or_else(||self.glyphs.get(&'?')) {
                draw_texture_ex(&self.texture,x,y-size,color,DrawTextureParams {source:Some(Rect::new(glyph.x as f32,0.0,glyph.width as f32,self.height)),dest_size:Some(vec2(glyph.width as f32*scale,size)),..Default::default()});
                x+=(glyph.width as f32+self.spacing)*scale;
            }
        }
    }
}
