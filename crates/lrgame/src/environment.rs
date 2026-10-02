//! Original named sky colors on a camera-centered dome.
//! This is not yet the original sky projection or environment-zone transition.
use lrformats::{library::Library, sky};
use macroquad::prelude::*;

pub struct Environment {
    mesh: Mesh,
}

impl Environment {
    pub fn load(library: &Library, table: &str, radius: f32) -> Result<Self, String> {
        let owner = library
            .jam()
            .tables
            .iter()
            .find(|t| t.name.eq_ignore_ascii_case(table))
            .ok_or("missing sky table")?;
        let entry = owner
            .entries
            .iter()
            .find(|e| e.name.to_ascii_uppercase().ends_with(".SKB"))
            .ok_or("original sky file missing")?;
        let sky = sky::parse(library.jam().bytes(entry).map_err(|e| e.to_string())?)?;
        let profile = sky
            .profiles
            .iter()
            .find(|p| p.name == sky.default)
            .ok_or("default sky missing")?;
        let color_at = |elevation: f32| {
            let t = elevation.max(0.0) * 2.0;
            let index = if t < 1.0 { 0 } else { 1 };
            let blend = if index == 0 { t } else { t - 1.0 };
            let c = std::array::from_fn::<_, 3, _>(|i| {
                f32::from(profile.colors[index][i]) * (1.0 - blend)
                    + f32::from(profile.colors[index + 1][i]) * blend
            });
            Color::new(c[0] / 255.0, c[1] / 255.0, c[2] / 255.0, 1.0)
        };
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        const ROWS: usize = 16;
        const COLS: usize = 32;
        for row in 0..=ROWS {
            let latitude =
                -std::f32::consts::FRAC_PI_2 + std::f32::consts::PI * row as f32 / ROWS as f32;
            for col in 0..=COLS {
                let longitude = std::f32::consts::TAU * col as f32 / COLS as f32;
                let point = vec3(
                    latitude.cos() * longitude.cos(),
                    latitude.sin(),
                    latitude.cos() * longitude.sin(),
                ) * radius;
                vertices.push(Vertex::new(
                    point.x,
                    point.y,
                    point.z,
                    0.0,
                    0.0,
                    color_at(latitude.sin()),
                ));
            }
        }
        for row in 0..ROWS {
            for col in 0..COLS {
                let a = (row * (COLS + 1) + col) as u16;
                let b = a + (COLS + 1) as u16;
                indices.extend([a, b, a + 1, a + 1, b, b + 1]);
            }
        }
        Ok(Self {
            mesh: Mesh {
                vertices,
                indices,
                texture: None,
            },
        })
    }

    pub fn draw(&self, eye: Vec3) {
        unsafe {
            macroquad::window::get_internal_gl()
                .quad_gl
                .push_model_matrix(Mat4::from_translation(eye));
        }
        draw_mesh(&self.mesh);
        unsafe {
            macroquad::window::get_internal_gl()
                .quad_gl
                .pop_model_matrix();
        }
    }
}
