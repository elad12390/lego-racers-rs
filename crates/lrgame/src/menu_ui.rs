//! Original bitmap menu art at the original640x480 logical coordinates.
//! Native text rendering is provisional; this does not claim original UI parity.
use lrformats::{bmp,library::Library};
use macroquad::prelude::*;
use std::collections::HashMap;

pub struct MenuUi {textures:HashMap<String,Texture2D>,layouts:HashMap<String,lrformats::menu_layout::Layout>,pub selected:usize,font:crate::bitmap_text::BitmapText,small:crate::bitmap_text::BitmapText,pub strings:lrformats::string_table::StringTable}
impl MenuUi {
    pub fn load(library:&Library)->Result<Self,String> {
        let mut textures=HashMap::new();
        for name in ["backdrp","racers","pirate","castle","space","adventur","islander","magical","jungle","alien","rr","gtrophy","strophy","btrophy","arrow","arrowlu","arrowls","arrowru","arrowrs","chck","txtx","txtarol","txtaror","license","mta","rotatea","upa","downa","cameraa","exita","bricks"] {
            let image=bmp::decode(library.find_in(&format!("{name}.BMP"),"MENUDATA").ok_or_else(||format!("missing menu bitmap {name}"))?).map_err(|e|e.to_string())?;
            let mut bytes=image.to_rgba();
            if name!="backdrp" {for pixel in bytes.chunks_exact_mut(4) {if pixel[..3]==[0,0,0] {pixel[3]=0;}}}
            let texture=Texture2D::from_rgba8(image.width,image.height,&bytes);texture.set_filter(FilterMode::Nearest);textures.insert(name.into(),texture);
        }
        let mut layouts=HashMap::new();
        for name in ["mainmenu","garage","circrace","singrace","editdrvr","drvrlice","carbuild","c_award1","c_award2","c_award3","c_award4"] {
            layouts.insert(name.into(),lrformats::menu_layout::Layout::parse(library.find_at(&format!("{name}.MIB"),"MENUDATA","MENUDATA").ok_or_else(||format!("missing original menu {name}"))?)?);
        }
        Ok(Self {textures,layouts,selected:0,font:crate::bitmap_text::BitmapText::load(library,"fontmenu")?,small:crate::bitmap_text::BitmapText::load(library,"font_ths")?,
            strings:lrformats::string_table::StringTable::parse(library.find_at("MENUTEXT.SRF","MENUDATA","ENGLISH").ok_or("missing original menu text")?)?})
    }
    pub fn scale()->f32 {(screen_width()/640.0).min(screen_height()/480.0)}
    pub fn origin()->Vec2 {let s=Self::scale();vec2((screen_width()-640.0*s)*0.5,(screen_height()-480.0*s)*0.5)}
    pub fn text(&self,text:&str,x:f32,y:f32,size:f32,color:Color) {
        let s=Self::scale();let o=Self::origin();
        if self.small.supports(text) {self.small.draw(text,o.x+x*s,o.y+y*s,size*s,color);}
        else {draw_text(text,o.x+x*s,o.y+y*s,size*s,color);}
    }
    pub fn image(&self,name:&str,x:f32,y:f32,width:f32,height:f32) {
        if let Some(texture)=self.textures.get(name) {
            let s=Self::scale();let o=Self::origin();draw_texture_ex(texture,o.x+x*s,o.y+y*s,WHITE,DrawTextureParams {dest_size:Some(vec2(width*s,height*s)),..Default::default()});
        }
    }
    pub fn original_image(&self,name:&str,x:f32,y:f32) {
        if let Some(t)=self.textures.get(name) {self.image(name,x,y,t.width(),t.height());}
    }
    pub fn centered_text(&self,text:&str,x:f32,y:f32,size:f32,color:Color) {self.text(text,x-self.small.width(text,size)/2.0,y,size,color);}
    pub fn menu_text(&self,text:&str,x:f32,y:f32,size:f32,color:Color) {let s=Self::scale();let o=Self::origin();self.font.draw(text,o.x+x*s,o.y+y*s,size*s,color);}
    pub fn layout_rect(&self,layout:&str,widget:&str)->Result<Rect,String> {let values=self.layouts.get(layout).and_then(|l|l.widgets.get(widget)).and_then(|w|w.rect).ok_or_else(||format!("missing original {layout}/{widget} rectangle"))?;Ok(Rect::new(values[0] as f32,values[1] as f32,values[2] as f32,values[3] as f32))}
    pub fn original_background(&self,title:&str,logo:bool) {
        clear_background(BLACK);self.image("backdrp",0.0,0.0,640.0,480.0);
        if logo {self.original_image("racers",35.0,4.0);}
        else {self.banner(title);}
        self.original_image("arrow",0.0,0.0);
    }
    pub fn banner(&self,title:&str) {let size=34.0;let s=Self::scale();let o=Self::origin();self.font.draw(title,o.x+(620.0-self.font.width(title,size))*s,o.y+58.0*s,size*s,WHITE);}
    /// Original main/garage widgets: arrow gutter, yellow inactive text and
    /// bright selected text. MIB coordinates, not a evenly-spaced modern list.
    pub fn original_buttons(&mut self,layout:&str,buttons:&[(&str,String,bool)])->Option<usize> {
        if buttons.is_empty() {return None;}
        self.selected=self.selected.min(buttons.len().saturating_sub(1));
        for (key,step) in [(KeyCode::Down,1),(KeyCode::Up,buttons.len().saturating_sub(1))] {
            if is_key_pressed(key) {for _ in 0..buttons.len() {self.selected=(self.selected+step)%buttons.len();if buttons[self.selected].2 {break;}}}
        }
        let mouse=(Vec2::from(mouse_position())-Self::origin())/Self::scale();let mut clicked=None;
        for (index,(name,label,enabled)) in buttons.iter().enumerate() {
            let widget=&self.layouts[layout].widgets[*name];let [x,y,_,_]=widget.rect.expect("original menu button rectangle validated by asset tests");
            let rect=Rect::new(x as f32,y as f32,225.0,32.0);
            if *enabled&&rect.contains(mouse)&&(mouse_delta_position().length_squared()>0.0||is_mouse_button_pressed(MouseButton::Left)) {self.selected=index;}
            let selected=index==self.selected&&*enabled;
            let color=if !*enabled {Color::new(0.38,0.38,0.32,1.0)} else if selected {YELLOW} else {Color::new(0.55,0.55,0.12,1.0)};
            self.menu_text(label,x as f32+32.0,y as f32+28.0,26.0,color);
            if *name=="goback" {self.original_image("txtarol",x as f32,y as f32+8.0);}
            if *enabled&&rect.contains(mouse)&&is_mouse_button_pressed(MouseButton::Left) {clicked=Some(index);}
        }
        if (is_key_pressed(KeyCode::Enter)||is_key_pressed(KeyCode::Space))&&buttons.get(self.selected).is_some_and(|b|b.2) {clicked=Some(self.selected);}
        clicked
    }
    pub fn preview_frame(&self,x:f32,y:f32,w:f32,h:f32) {
        let s=Self::scale();let o=Self::origin();
        draw_rectangle(o.x+x*s,o.y+y*s,w*s,h*s,Color::new(0.005,0.005,0.14,1.0));
        draw_rectangle_lines(o.x+x*s,o.y+y*s,w*s,h*s,3.0*s,Color::new(0.12,0.09,0.7,1.0));
    }
    pub fn action(&self,label:&str,x:f32,y:f32,icon:Option<&str>)->bool {
        let mouse=(Vec2::from(mouse_position())-Self::origin())/Self::scale();let hover=Rect::new(x,y-28.0,250.0,34.0).contains(mouse);
        if let Some(icon)=icon {self.original_image(icon,x-32.0,y-28.0);}
        self.menu_text(label,x,y,26.0,if hover {YELLOW} else {Color::new(0.6,0.6,0.15,1.0)});
        hover&&is_mouse_button_pressed(MouseButton::Left)
    }
    pub fn icon_button(&self,name:&str,x:f32,y:f32)->bool {
        self.original_image(name,x,y);
        let mouse=(Vec2::from(mouse_position())-Self::origin())/Self::scale();
        self.textures.get(name).is_some_and(|t|Rect::new(x,y,t.width(),t.height()).contains(mouse)&&is_mouse_button_pressed(MouseButton::Left))
    }
    pub fn background(&self,title:&str) {
        self.original_background(title,false);
    }
    pub fn buttons(&mut self,labels:&[String],x:f32,y:f32,spacing:f32)->Option<usize> {
        self.buttons_mode(labels,x,y,spacing,true)
    }
    pub fn mouse_buttons(&mut self,labels:&[String],x:f32,y:f32,spacing:f32)->Option<usize> {
        self.buttons_mode(labels,x,y,spacing,false)
    }
    fn buttons_mode(&mut self,labels:&[String],x:f32,y:f32,spacing:f32,keyboard:bool)->Option<usize> {
        if labels.is_empty() {return None;}
        self.selected=self.selected.min(labels.len()-1);
        if keyboard&&is_key_pressed(KeyCode::Down) {self.selected=(self.selected+1)%labels.len();}
        if keyboard&&is_key_pressed(KeyCode::Up) {self.selected=(self.selected+labels.len()-1)%labels.len();}
        let s=Self::scale();let mouse=(Vec2::from(mouse_position())-Self::origin())/s;
        let mut clicked=None;
        for (index,label) in labels.iter().enumerate() {
            let top=y+index as f32*spacing;
            if Rect::new(x,top-25.0,235.0,spacing).contains(mouse) && (mouse_delta_position().length_squared()>0.0||is_mouse_button_pressed(MouseButton::Left)) {self.selected=index;}
            self.text(label,x,top,26.0,if index==self.selected {YELLOW} else {WHITE});
            if index==self.selected {self.text(">",x-17.0,top,23.0,YELLOW);}
            if self.selected==index && is_mouse_button_pressed(MouseButton::Left)&&Rect::new(x,top-25.0,235.0,spacing).contains(mouse) {clicked=Some(index);}
        }
        if keyboard&&(is_key_pressed(KeyCode::Enter)||is_key_pressed(KeyCode::Space)) {clicked=Some(self.selected);}
        clicked
    }
}
