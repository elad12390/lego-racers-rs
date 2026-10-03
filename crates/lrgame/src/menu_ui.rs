//! Original bitmap menu art at the original640x480 logical coordinates.
//! Native text rendering is provisional; this does not claim original UI parity.
use crate::platform::prelude::*;
use lrformats::{bmp, library::Library};
use std::collections::HashMap;
#[path = "menu_ui_dialog.rs"]
mod dialog;

pub struct MenuUi {
  pub driver_scroll_ms: u32,
  pub audio: Option<crate::menu_audio::MenuAudio>,
  textures: HashMap<String, Texture2D>,
  layouts: HashMap<String, lrformats::menu_layout::Layout>,
  pub selected: usize,
  font: crate::bitmap_text::BitmapText,
  small: crate::bitmap_text::BitmapText,
  license_name: crate::bitmap_text::BitmapText,
  button_styles: std::collections::BTreeMap<String, lrformats::menu_button::Style>,
  frame_styles: std::collections::BTreeMap<String, lrformats::menu_frame::Frame>,
  pub strings: lrformats::string_table::StringTable,
}
impl MenuUi {
  pub fn navigation_key(&self, key: KeyCode) -> bool {
    let pressed = is_key_pressed(key);
    if pressed {
      if let Some(audio) = &self.audio {
        audio.moved();
      }
    }
    pressed
  }
  pub fn activation_key(&self, key: KeyCode) -> bool {
    let pressed = is_key_pressed(key);
    if pressed {
      if let Some(audio) = &self.audio {
        audio.activated();
      }
    }
    pressed
  }
  pub fn cancellation_key(&self, key: KeyCode) -> bool {
    let pressed = is_key_pressed(key);
    if pressed {
      if let Some(audio) = &self.audio {
        audio.cancelled();
      }
    }
    pressed
  }
  pub fn load(library: &Library) -> Result<Self, String> {
    let mut textures = HashMap::new();
    for name in [
      "backdrp", "racers", "pirate", "castle", "space", "adventur", "islander", "magical",
      "jungle", "alien", "rr", "gtrophy", "strophy", "btrophy", "arrow", "arrowlu", "arrowls",
      "arrowru", "arrowrs", "chck", "txtx", "txtarol", "txtaror", "license", "mta", "rotatea",
      "upa", "downa", "cameraa", "exita", "bricks", "clear32", "lul", "ltb", "lur", "lrb", "lbr",
      "lbb", "lbl", "llb", "tul", "tt", "tur", "tr", "tbr", "tb", "tbl", "tl",
    ] {
      let image = bmp::decode(
        library
          .find_in(&format!("{name}.BMP"), "MENUDATA")
          .ok_or_else(|| format!("missing menu bitmap {name}"))?,
      )
      .map_err(|e| e.to_string())?;
      let mut bytes = image.to_rgba();
      if name != "backdrp" {
        for pixel in bytes.chunks_exact_mut(4) {
          if pixel[..3] == [0, 0, 0] {
            pixel[3] = 0;
          }
        }
      }
      let texture = Texture2D::from_rgba8(image.width, image.height, &bytes);
      texture.set_filter(FilterMode::Nearest);
      textures.insert(name.into(), texture);
    }
    let mut layouts = HashMap::new();
    for name in [
      "mainmenu", "garage", "circrace", "singrace", "editdrvr", "drvrlice", "carbuild", "c_award1",
      "c_award2", "c_award3", "c_award4", "dialog",
    ] {
      layouts.insert(
        name.into(),
        lrformats::menu_layout::Layout::parse(
          library
            .find_at(&format!("{name}.MIB"), "MENUDATA", "MENUDATA")
            .ok_or_else(|| format!("missing original menu {name}"))?,
        )?,
      );
    }
    Ok(Self {
      driver_scroll_ms: lrformats::menu_selector::scroll_duration(
        library
          .find_at("GSTYLES.MSB", "MENUDATA", "MENUDATA")
          .ok_or("missing original selector styles")?,
        "picker",
      )?,
      audio: None,
      textures,
      layouts,
      selected: 0,
      font: crate::bitmap_text::BitmapText::load(library, "fontmenu")?,
      small: crate::bitmap_text::BitmapText::load(library, "font_ths")?,
      license_name: crate::bitmap_text::BitmapText::load(
        library,
        &lrsim::brick_build::Rules::load()?.license_name_font,
      )?,
      button_styles: lrformats::menu_button::parse(
        library
          .find_at("GSTYLES.MSB", "MENUDATA", "MENUDATA")
          .ok_or("missing original button styles")?,
      )?,
      frame_styles: lrformats::menu_frame::parse_styles(
        library
          .find_at("GSTYLES.MSB", "MENUDATA", "MENUDATA")
          .ok_or("missing original frame styles")?,
      )?,
      strings: lrformats::string_table::StringTable::parse(
        library
          .find_at("MENUTEXT.SRF", "MENUDATA", "ENGLISH")
          .ok_or("missing original menu text")?,
      )?,
    })
  }
  pub fn scale() -> f32 {
    (screen_width() / 640.0).min(screen_height() / 480.0)
  }
  pub fn origin() -> Vec2 {
    let s = Self::scale();
    vec2(
      (screen_width() - 640.0 * s) * 0.5,
      (screen_height() - 480.0 * s) * 0.5,
    )
  }
  /// Editor-only Rect_ContainsRelative00472c40 includes both upper edges.
  /// Keep the shared half-open rectangle and unrelated menu hit tests unchanged.
  pub fn editor_contains(rect: Rect, point: Vec2) -> bool {
    point.x >= rect.x
      && point.y >= rect.y
      && point.x <= rect.x + rect.w
      && point.y <= rect.y + rect.h
  }
  pub fn text(&self, text: &str, x: f32, y: f32, size: f32, color: Color) {
    let s = Self::scale();
    let o = Self::origin();
    if self.small.supports(text) {
      self
        .small
        .draw(text, o.x + x * s, o.y + y * s, size * s, color);
    } else {
      draw_text(text, o.x + x * s, o.y + y * s, size * s, color);
    }
  }
  pub fn image(&self, name: &str, x: f32, y: f32, width: f32, height: f32) {
    if let Some(texture) = self.textures.get(name) {
      let s = Self::scale();
      let o = Self::origin();
      draw_texture_ex(
        texture,
        o.x + x * s,
        o.y + y * s,
        WHITE,
        DrawTextureParams {
          dest_size: Some(vec2(width * s, height * s)),
          ..Default::default()
        },
      );
    }
  }
  pub fn original_image(&self, name: &str, x: f32, y: f32) {
    if let Some(t) = self.textures.get(name) {
      self.image(name, x, y, t.width(), t.height());
    }
  }
  pub fn centered_text(&self, text: &str, x: f32, y: f32, size: f32, color: Color) {
    self.text(text, x - self.small.width(text, size) / 2.0, y, size, color);
  }
  /// Keep long original track/champion names inside their allocated columns.
  pub fn fitted_text(&self, text: &str, x: f32, y: f32, size: f32, width: f32, color: Color) {
    let measured = self.small.width(text, size);
    let size = if measured > width {
      size * width / measured
    } else {
      size
    };
    self.text(text, x, y, size, color);
  }
  pub fn menu_text(&self, text: &str, x: f32, y: f32, size: f32, color: Color) {
    let s = Self::scale();
    let o = Self::origin();
    self
      .font
      .draw(text, o.x + x * s, o.y + y * s, size * s, color);
  }
  pub fn layout_rect(&self, layout: &str, widget: &str) -> Result<Rect, String> {
    let values = self
      .layouts
      .get(layout)
      .and_then(|l| l.widgets.get(widget))
      .and_then(|w| w.rect)
      .ok_or_else(|| format!("missing original {layout}/{widget} rectangle"))?;
    Ok(Rect::new(
      values[0] as f32,
      values[1] as f32,
      values[2] as f32,
      values[3] as f32,
    ))
  }
  pub fn license_camera(
    &self,
    library: &Library,
  ) -> Result<crate::menu_camera::MenuCamera, String> {
    let scene = self.layouts["drvrlice"].widgets["platform"]
      .scene
      .as_deref()
      .ok_or("missing original license scene")?;
    crate::menu_camera::MenuCamera::load(
      library
        .find_at(&format!("{scene}.WDB"), "MENUDATA", "MENUDATA")
        .ok_or("missing original license scene camera")?,
    )
  }
  /// UiLayoutFrame0046ee40 insets DRVRLICE's platform by its CLEAR32 edges.
  pub fn license_viewport(&self) -> Result<Rect, String> {
    let card = self.layout_rect("drvrlice", "license")?;
    let platform = self.layout_rect("drvrlice", "platform")?;
    let frame = self.layouts["drvrlice"].frame_for("platform")?;
    let edge = self
      .textures
      .get(&frame.images[0])
      .ok_or("missing license frame edge")?;
    let width = platform.w - platform.x - 2.0 * edge.width();
    let height = platform.h - platform.y - 2.0 * edge.height();
    if width <= 0.0 || height <= 0.0 {
      return Err("invalid original license viewport".into());
    }
    Ok(Rect::new(
      card.x + platform.x + edge.width(),
      card.y + platform.y + edge.height(),
      width,
      height,
    ))
  }
  /// The original frame panel submits an opaque quad; CLEAR32 edge art is keyed
  /// entirely transparent. Keep the card's authored rim, not a fabricated line.
  pub fn license_preview_background(&self) -> Result<(), String> {
    let frame = self.layouts["drvrlice"].frame_for("platform")?;
    if !frame.shown {
      return Ok(());
    }
    if frame.images.iter().any(|name| name != "clear32") {
      return Err("unsupported original license frame edges".into());
    }
    let rect = self.license_viewport()?;
    let scale = Self::scale();
    let origin = Self::origin();
    let [r, g, b, a] = frame.panel_color;
    draw_rectangle(
      origin.x + rect.x * scale,
      origin.y + rect.y * scale,
      rect.w * scale,
      rect.h * scale,
      Color::from_rgba(r, g, b, a),
    );
    Ok(())
  }
  /// DRVRLICE's ftext bounds are parent-relative left/top/right/bottom insets.
  pub fn license_name(&self, name: &str) -> Result<(), String> {
    let card = self.layout_rect("drvrlice", "license")?;
    let text = self.layout_rect("drvrlice", "ftext")?;
    let height = self.license_name.native_height();
    let x = card.x + text.x;
    let baseline = card.y + text.y + (text.h - text.y + height) * 0.5;
    let scale = Self::scale();
    let origin = Self::origin();
    self.license_name.draw(
      name,
      origin.x + x * scale,
      origin.y + baseline * scale,
      height * scale,
      WHITE,
    );
    Ok(())
  }
  pub fn license_name_bounds(&self) -> Result<Rect, String> {
    let card = self.layout_rect("drvrlice", "license")?;
    let text = self.layout_rect("drvrlice", "ftext")?;
    Ok(Rect::new(
      card.x + text.x,
      card.y + text.y,
      text.w - text.x,
      text.h - text.y,
    ))
  }
  /// Original firstbox -> roundbox style and UiLayoutFrame0046ee40 layout.
  pub fn license_name_frame(&self) -> Result<(), String> {
    let card = self.layout_rect("drvrlice", "license")?;
    let inset = self.layout_rect("drvrlice", "firstbox")?;
    let rect = Rect::new(
      card.x + inset.x,
      card.y + inset.y,
      inset.w - inset.x,
      inset.h - inset.y,
    );
    let frame = self
      .frame_styles
      .get("roundbox")
      .ok_or("missing original name frame")?;
    self.draw_original_frame(rect, frame)
  }
  pub fn driver_preview_frame(&self) -> Result<(), String> {
    let inset = self.layout_rect("editdrvr", "platform")?;
    self.draw_original_frame(
      Rect::new(inset.x, inset.y, inset.w - inset.x, inset.h - inset.y),
      self.layouts["editdrvr"].frame_for("platform")?,
    )
  }
  pub fn driver_viewport(&self) -> Result<Rect, String> {
    let inset = self.layout_rect("editdrvr", "platform")?;
    let frame = self.layouts["editdrvr"].frame_for("platform")?;
    let top_left = self
      .textures
      .get(&frame.images[0])
      .ok_or("missing driver frame corner")?;
    let top_right = self
      .textures
      .get(&frame.images[2])
      .ok_or("missing driver frame corner")?;
    let bottom_left = self
      .textures
      .get(&frame.images[6])
      .ok_or("missing driver frame corner")?;
    Ok(Rect::new(
      inset.x + top_left.width(),
      inset.y + top_left.height(),
      inset.w - inset.x - top_left.width() - top_right.width(),
      inset.h - inset.y - top_left.height() - bottom_left.height(),
    ))
  }
  /// EDITDRVR framed-row children: the left/right controls use local anchors.
  /// Original SetBounds + SumOrigins00472e90 proves their screen-space bounds.
  /// Panel bounds use authored left/top/right/bottom coordinates, not sizes.
  pub fn driver_row_bounds(&self, row: usize) -> Result<Rect, String> {
    let name = ["hatsel", "facesel", "torsosel", "legsel"]
      .get(row)
      .ok_or("invalid driver row")?;
    let rect = self.layout_rect("editdrvr", name)?;
    Ok(Rect::new(rect.x, rect.y, rect.w - rect.x, rect.h - rect.y))
  }

  pub fn driver_row_arrow_bounds(&self, row: usize, right: bool) -> Result<Rect, String> {
    let row_name = ["hatsel", "facesel", "torsosel", "legsel"]
      .get(row)
      .ok_or("invalid driver row")?;
    let parent = self.layout_rect("editdrvr", row_name)?;
    let child = self.layout_rect("editdrvr", if right { "right" } else { "left" })?;
    let image = self
      .textures
      .get(if right { "arrowru" } else { "arrowlu" })
      .ok_or("missing original driver row arrow")?;
    Ok(Rect::new(
      parent.x + child.x,
      parent.y + child.y,
      image.width(),
      image.height(),
    ))
  }
  fn draw_original_frame(
    &self,
    rect: Rect,
    frame: &lrformats::menu_frame::Frame,
  ) -> Result<(), String> {
    if !frame.shown {
      return Ok(());
    }
    let images = frame
      .images
      .iter()
      .map(|name| {
        self
          .textures
          .get(name)
          .ok_or("missing original frame bitmap")
      })
      .collect::<Result<Vec<_>, _>>()?;
    let left = images[0].width();
    let top = images[0].height();
    let right = rect.w - images[2].width();
    let bottom = rect.h - images[4].height();
    let pieces = [
      Rect::new(0.0, 0.0, left, top),
      Rect::new(left, 0.0, right - left, images[1].height()),
      Rect::new(right, 0.0, images[2].width(), top),
      Rect::new(
        rect.w - images[3].width(),
        top,
        images[3].width(),
        bottom - top,
      ),
      Rect::new(
        rect.w - images[4].width(),
        bottom,
        images[4].width(),
        images[4].height(),
      ),
      Rect::new(
        images[6].width(),
        rect.h - images[5].height(),
        rect.w - images[4].width() - images[6].width(),
        images[5].height(),
      ),
      Rect::new(
        0.0,
        rect.h - images[6].height(),
        images[6].width(),
        images[6].height(),
      ),
      Rect::new(
        0.0,
        top,
        images[7].width(),
        rect.h - images[6].height() - top,
      ),
    ];
    let scale = Self::scale();
    let origin = Self::origin() + vec2(rect.x, rect.y) * scale;
    let color = |[r, g, b, a]: [u8; 4]| Color::from_rgba(r, g, b, a);
    draw_rectangle(
      origin.x + left * scale,
      origin.y + top * scale,
      (right - left) * scale,
      (rect.h - images[6].height() - top) * scale,
      color(frame.panel_color),
    );
    for (image, piece) in images.iter().zip(pieces) {
      draw_texture_ex(
        image,
        origin.x + piece.x * scale,
        origin.y + piece.y * scale,
        color(frame.edge_color),
        DrawTextureParams {
          dest_size: Some(vec2(piece.w, piece.h) * scale),
          ..Default::default()
        },
      );
    }
    Ok(())
  }
  /// UiTextFieldLaidOut00471930/00471a30: maximum allowed-glyph width,
  /// four pixels tall, at measured text end and field bottom minus six.
  pub fn license_caret_bounds(&self, name: &str) -> Result<Rect, String> {
    let rect = self.license_name_bounds()?;
    let height = self.license_name.native_height();
    // CameraManAnimation passes string ID 1 to CreateControl_WithValue_FromNames.
    let width = self
      .strings
      .get(1)?
      .chars()
      .map(|c| self.license_name.width(&c.to_string(), height))
      .fold(0.0_f32, f32::max);
    Ok(Rect::new(
      rect.x + self.license_name.width(name, height),
      rect.y + rect.h - 6.0,
      width,
      4.0,
    ))
  }
  pub fn license_caret(&self, name: &str) -> Result<(), String> {
    let rect = self.license_caret_bounds(name)?;
    let scale = Self::scale();
    let origin = Self::origin();
    draw_rectangle(
      origin.x + rect.x * scale,
      origin.y + rect.y * scale,
      rect.w * scale,
      rect.h * scale,
      WHITE,
    );
    Ok(())
  }
  pub fn license_name_character(&self, c: char) -> Result<bool, String> {
    Ok(self.strings.get(1)?.contains(c.to_ascii_uppercase()))
  }
  pub fn original_background(&self, title: &str, logo: bool) {
    clear_background(BLACK);
    self.image("backdrp", 0.0, 0.0, 640.0, 480.0);
    if logo {
      self.original_image("racers", 35.0, 4.0);
    } else {
      self.banner(title);
    }
    self.original_image("arrow", 0.0, 0.0);
  }
  pub fn banner(&self, title: &str) {
    let size = 34.0;
    let s = Self::scale();
    let o = Self::origin();
    self.font.draw(
      title,
      o.x + (620.0 - self.font.width(title, size)) * s,
      o.y + 58.0 * s,
      size * s,
      WHITE,
    );
  }
  /// Original main/garage widgets: arrow gutter, yellow inactive text and
  /// bright selected text. MIB coordinates, not a evenly-spaced modern list.
  pub fn original_buttons(
    &mut self,
    layout: &str,
    buttons: &[(&str, String, bool)],
  ) -> Option<usize> {
    if buttons.is_empty() {
      return None;
    }
    self.selected = self.selected.min(buttons.len().saturating_sub(1));
    let previous = self.selected;
    for (key, step) in [
      (KeyCode::Down, 1),
      (KeyCode::Up, buttons.len().saturating_sub(1)),
    ] {
      if is_key_pressed(key) {
        for _ in 0..buttons.len() {
          self.selected = (self.selected + step) % buttons.len();
          if buttons[self.selected].2 {
            break;
          }
        }
      }
    }
    let mouse = (Vec2::from(mouse_position()) - Self::origin()) / Self::scale();
    let mut clicked = None;
    for (index, (name, label, enabled)) in buttons.iter().enumerate() {
      let widget = &self.layouts[layout].widgets[*name];
      let [x, y, _, _] = widget
        .rect
        .expect("original menu button rectangle validated by asset tests");
      let rect = Rect::new(x as f32, y as f32, 225.0, 32.0);
      if *enabled
        && rect.contains(mouse)
        && (mouse_delta_position().length_squared() > 0.0
          || is_mouse_button_pressed(MouseButton::Left))
      {
        self.selected = index;
      }
      let selected = index == self.selected && *enabled;
      let color = if !*enabled {
        Color::new(0.38, 0.38, 0.32, 1.0)
      } else if selected {
        YELLOW
      } else {
        Color::new(0.55, 0.55, 0.12, 1.0)
      };
      self.menu_text(label, x as f32 + 32.0, y as f32 + 28.0, 26.0, color);
      if *name == "goback" {
        self.original_image("txtarol", x as f32, y as f32 + 8.0);
      }
      if *enabled && rect.contains(mouse) && is_mouse_button_pressed(MouseButton::Left) {
        clicked = Some(index);
      }
    }
    if (is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space))
      && buttons.get(self.selected).is_some_and(|b| b.2)
    {
      clicked = Some(self.selected);
    }
    if let Some(audio) = &self.audio {
      if self.selected != previous {
        audio.moved();
      }
      if clicked.is_some() {
        audio.activated();
      }
    }
    clicked
  }
  pub fn preview_frame(&self, x: f32, y: f32, w: f32, h: f32) {
    let s = Self::scale();
    let o = Self::origin();
    draw_rectangle(
      o.x + x * s,
      o.y + y * s,
      w * s,
      h * s,
      Color::new(0.005, 0.005, 0.14, 1.0),
    );
    draw_rectangle_lines(
      o.x + x * s,
      o.y + y * s,
      w * s,
      h * s,
      3.0 * s,
      Color::new(0.12, 0.09, 0.7, 1.0),
    );
  }
  /// License controls keep the original MIB anchors, SRF labels and MSB tints.
  /// Native shortcuts/click timing are retained; this is not the full original
  /// UiInteractiveControl focus/animation state machine.
  pub fn license_action_bounds(
    &self,
    widget: &str,
    label: usize,
    style: &str,
  ) -> Result<Rect, String> {
    self.named_action_bounds("drvrlice", widget, label, style)
  }
  pub fn driver_action_bounds(
    &self,
    widget: &str,
    label: usize,
    style: &str,
  ) -> Result<Rect, String> {
    self.named_action_bounds("editdrvr", widget, label, style)
  }
  fn named_action_bounds(
    &self,
    layout: &str,
    widget: &str,
    label: usize,
    style: &str,
  ) -> Result<Rect, String> {
    let rect = self.layout_rect(layout, widget)?;
    self.action_bounds_at(vec2(rect.x, rect.y), label, style)
  }
  pub(crate) fn action_bounds_at(
    &self,
    position: Vec2,
    label: usize,
    style: &str,
  ) -> Result<Rect, String> {
    let style = self
      .button_styles
      .get(style)
      .ok_or("missing original button style")?;
    let icon = self
      .textures
      .get(&style.images[2])
      .ok_or("missing button bitmap")?;
    Ok(Rect::new(
      position.x,
      position.y,
      icon.width()
        + self
          .small
          .width(self.strings.get(label)?, self.small.native_height()),
      icon.height(),
    ))
  }
  pub fn license_action(
    &self,
    widget: &str,
    label: usize,
    style: &str,
    highlighted: bool,
    pointer: crate::menu_pointer::Frame,
  ) -> Result<bool, String> {
    self.named_action("drvrlice", widget, label, style, Some(highlighted), pointer)
  }
  pub fn driver_action(
    &self,
    widget: &str,
    label: usize,
    style: &str,
    pointer: crate::menu_pointer::Frame,
    highlighted: bool,
  ) -> Result<bool, String> {
    self.named_action("editdrvr", widget, label, style, Some(highlighted), pointer)
  }
  pub fn driver_selector(
    &self,
    row: usize,
  ) -> Result<(&lrformats::menu_selector::Selector, Rect), String> {
    let layout = self
      .layouts
      .get("editdrvr")
      .ok_or("missing original driver layout")?;
    let widget = layout
      .widgets
      .get(if row == 1 { "headbox" } else { "partbox" })
      .ok_or("missing original driver selector")?;
    let selector = widget
      .selector
      .as_ref()
      .ok_or("missing original driver slot data")?;
    let r = widget
      .rect
      .ok_or("missing original driver selector viewport")?;
    let parent = self.layout_rect(
      "editdrvr",
      ["hatsel", "facesel", "torsosel", "legsel"]
        .get(row)
        .ok_or("invalid driver row")?,
    )?;
    Ok((
      selector,
      Rect::new(
        parent.x + r[0] as f32,
        parent.y + r[1] as f32,
        (r[2] - r[0]) as f32,
        (r[3] - r[1]) as f32,
      ),
    ))
  }
  fn named_action(
    &self,
    layout: &str,
    widget: &str,
    label: usize,
    style: &str,
    highlighted: Option<bool>,
    pointer: crate::menu_pointer::Frame,
  ) -> Result<bool, String> {
    let rect = self.layout_rect(layout, widget)?;
    self.named_action_at(widget, label, style, rect, highlighted, pointer)
  }
  pub(crate) fn named_action_at(
    &self,
    widget: &str,
    label: usize,
    style: &str,
    rect: Rect,
    highlighted: Option<bool>,
    pointer: crate::menu_pointer::Frame,
  ) -> Result<bool, String> {
    let style = self
      .button_styles
      .get(style)
      .ok_or("missing original button style")?;
    let label = self.strings.get(label)?;
    let icon = self
      .textures
      .get(&style.images[2])
      .ok_or("missing button bitmap")?;
    let height = icon.height();
    let font_height = self.small.native_height();
    let text_x = rect.x + icon.width();
    let mouse = (Vec2::from(mouse_position()) - Self::origin()) / Self::scale();
    let hover = Self::editor_contains(
      Rect::new(
        rect.x,
        rect.y,
        icon.width() + self.small.width(label, font_height),
        height,
      ),
      mouse,
    );
    let clicked = pointer.committed == Some(widget);
    // StartMove_FromFlags00472080: shown=2, highlighted=4, activated=5.
    let state = if pointer.active == Some(widget) {
      5
    } else if pointer.active.is_none() && highlighted.unwrap_or(hover) {
      4
    } else {
      2
    };
    if style.fonts[state] != "font_ths" {
      return Err("unsupported original button font".into());
    }
    let scale = Self::scale();
    let origin = Self::origin();
    let color = |[r, g, b, a]: [u8; 4]| Color::from_rgba(r, g, b, a);
    draw_texture_ex(
      icon,
      origin.x + rect.x * scale,
      origin.y + rect.y * scale,
      color(style.image_colors[state]),
      DrawTextureParams {
        dest_size: Some(vec2(icon.width() * scale, height * scale)),
        ..Default::default()
      },
    );
    self.small.draw(
      label,
      origin.x + text_x * scale,
      origin.y + (rect.y + (height + font_height) * 0.5) * scale,
      font_height * scale,
      color(style.text_colors[state]),
    );
    if pointer.pressed == Some(widget) {
      if let Some(audio) = &self.audio {
        audio.activated();
      }
    }
    Ok(clicked)
  }
  pub fn action(&self, label: &str, x: f32, y: f32, icon: Option<&str>) -> bool {
    let mouse = (Vec2::from(mouse_position()) - Self::origin()) / Self::scale();
    let hover = Rect::new(x, y - 28.0, 250.0, 34.0).contains(mouse);
    if let Some(icon) = icon {
      self.original_image(icon, x - 32.0, y - 28.0);
    }
    self.menu_text(
      label,
      x,
      y,
      26.0,
      if hover {
        YELLOW
      } else {
        Color::new(0.6, 0.6, 0.15, 1.0)
      },
    );
    let clicked = hover && is_mouse_button_pressed(MouseButton::Left);
    if clicked {
      if let Some(audio) = &self.audio {
        audio.activated();
      }
    }
    clicked
  }
  pub fn icon_button(&self, name: &str, x: f32, y: f32) -> bool {
    self.original_image(name, x, y);
    let mouse = (Vec2::from(mouse_position()) - Self::origin()) / Self::scale();
    let clicked = self.textures.get(name).is_some_and(|t| {
      Rect::new(x, y, t.width(), t.height()).contains(mouse)
        && is_mouse_button_pressed(MouseButton::Left)
    });
    if clicked {
      if let Some(audio) = &self.audio {
        audio.activated();
      }
    }
    clicked
  }
  pub fn background(&self, title: &str) {
    self.original_background(title, false);
  }
  pub fn buttons(&mut self, labels: &[String], x: f32, y: f32, spacing: f32) -> Option<usize> {
    self.buttons_mode(labels, x, y, spacing, true)
  }
  pub fn mouse_buttons(
    &mut self,
    labels: &[String],
    x: f32,
    y: f32,
    spacing: f32,
  ) -> Option<usize> {
    self.buttons_mode(labels, x, y, spacing, false)
  }
  fn buttons_mode(
    &mut self,
    labels: &[String],
    x: f32,
    y: f32,
    spacing: f32,
    keyboard: bool,
  ) -> Option<usize> {
    if labels.is_empty() {
      return None;
    }
    self.selected = self.selected.min(labels.len() - 1);
    let previous = self.selected;
    if keyboard && is_key_pressed(KeyCode::Down) {
      self.selected = (self.selected + 1) % labels.len();
    }
    if keyboard && is_key_pressed(KeyCode::Up) {
      self.selected = (self.selected + labels.len() - 1) % labels.len();
    }
    let s = Self::scale();
    let mouse = (Vec2::from(mouse_position()) - Self::origin()) / s;
    let mut clicked = None;
    for (index, label) in labels.iter().enumerate() {
      let top = y + index as f32 * spacing;
      if Rect::new(x, top - 25.0, 235.0, spacing).contains(mouse)
        && (mouse_delta_position().length_squared() > 0.0
          || is_mouse_button_pressed(MouseButton::Left))
      {
        self.selected = index;
      }
      self.text(
        label,
        x,
        top,
        26.0,
        if index == self.selected {
          YELLOW
        } else {
          WHITE
        },
      );
      if index == self.selected {
        self.text(">", x - 17.0, top, 23.0, YELLOW);
      }
      if self.selected == index
        && is_mouse_button_pressed(MouseButton::Left)
        && Rect::new(x, top - 25.0, 235.0, spacing).contains(mouse)
      {
        clicked = Some(index);
      }
    }
    if keyboard && (is_key_pressed(KeyCode::Enter) || is_key_pressed(KeyCode::Space)) {
      clicked = Some(self.selected);
    }
    if let Some(audio) = &self.audio {
      if self.selected != previous {
        audio.moved();
      }
      if clicked.is_some() {
        audio.activated();
      }
    }
    clicked
  }
}

#[cfg(test)]
mod editor_hit_tests {
  use super::*;
  #[test]
  fn original_editor_upper_edges_are_inclusive_without_changing_shared_rect() {
    let rect = Rect::new(120.0, 197.0, 268.0, 34.0);
    for point in [
      vec2(120.0, 197.0),
      vec2(388.0, 231.0),
      vec2(388.0, 197.0),
      vec2(120.0, 231.0),
    ] {
      assert!(MenuUi::editor_contains(rect, point));
    }
    assert!(!rect.contains(vec2(388.0, 231.0)));
    for point in [
      vec2(119.0, 197.0),
      vec2(120.0, 196.0),
      vec2(389.0, 231.0),
      vec2(388.0, 232.0),
      vec2(f32::NAN, 200.0),
    ] {
      assert!(!MenuUi::editor_contains(rect, point));
    }
  }
}
