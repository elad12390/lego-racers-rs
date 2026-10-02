use lrformats::world::{CollisionMesh, Vec3};

const CELL: f32 = 16.0;
/// Triangles steeper than this (|normal.z|) are walls, not drivable ground.
const MIN_GROUND_NORMAL_Z: f32 = 0.45;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundHit {
    pub height: f32,
    /// Unit surface normal, pointing up.
    pub normal: Vec3,
    pub plane_offset:f32,
    pub surface: u32,
    pub triangle: usize,
}

/// Height lookup over a collision mesh using a uniform grid on the XY plane.
pub struct Ground {
    tree:std::sync::Arc<lrformats::collision_tree::CollisionTree>,
    supporting_surfaces:Vec<bool>,
    min: [f32; 2],
    cols: usize,
    rows: usize,
    cells: Vec<Vec<u32>>,
}

impl Ground {
    pub fn new(mesh: CollisionMesh) -> Self {
        Self::with_tree(std::sync::Arc::new(lrformats::collision_tree::CollisionTree {mesh,planes:Vec::new(),depth:0}),Vec::new())
    }

    pub fn with_tree(tree:std::sync::Arc<lrformats::collision_tree::CollisionTree>,supporting_surfaces:Vec<bool>)->Self {
        let mesh=&tree.mesh;
        let (mut min, mut max) = ([f32::MAX; 2], [f32::MIN; 2]);
        for v in &mesh.vertices {
            for k in 0..2 {
                min[k] = min[k].min(v[k]);
                max[k] = max[k].max(v[k]);
            }
        }
        if mesh.vertices.is_empty() {
            return Ground {
                tree,supporting_surfaces,
                min: [0.0; 2],
                cols: 0,
                rows: 0,
                cells: Vec::new(),
            };
        }
        let cols = (((max[0] - min[0]) / CELL).ceil() as usize).max(1);
        let rows = (((max[1] - min[1]) / CELL).ceil() as usize).max(1);
        let mut cells = vec![Vec::new(); cols * rows];
        for (n, tri) in mesh.triangles.iter().enumerate() {
            let v = tri.indices.map(|i| mesh.vertices[i as usize]);
            let lo = [
                v.iter().map(|p| p[0]).fold(f32::MAX, f32::min),
                v.iter().map(|p| p[1]).fold(f32::MAX, f32::min),
            ];
            let hi = [
                v.iter().map(|p| p[0]).fold(f32::MIN, f32::max),
                v.iter().map(|p| p[1]).fold(f32::MIN, f32::max),
            ];
            let (c0, c1) = (
                cell_index(lo[0], min[0], cols),
                cell_index(hi[0], min[0], cols),
            );
            let (r0, r1) = (
                cell_index(lo[1], min[1], rows),
                cell_index(hi[1], min[1], rows),
            );
            for r in r0..=r1 {
                for c in c0..=c1 {
                    cells[r * cols + c].push(n as u32);
                }
            }
        }
        Ground {
            tree,supporting_surfaces,
            min,
            cols,
            rows,
            cells,
        }
    }

    pub fn triangle_count(&self) -> usize {
        self.tree.mesh.triangles.len()
    }

    /// Bounded downward wheel ray. Interpolate the segment crossing rather than
    /// substituting an unbounded height query (important near stacked decks).
    pub fn trace_vertical(&self, x: f32, y: f32, start: f32, end: f32) -> Option<GroundHit> {
        self.trace_vertical_cached(x, y, start, end, &[])
    }

    /// The original wheel contact cache reuses a previously hit triangle before
    /// traversing the plane tree. Keep that selection and interpolation order.
    pub fn trace_vertical_cached(
        &self,
        x: f32,
        y: f32,
        start: f32,
        end: f32,
        cached: &[GroundHit],
    ) -> Option<GroundHit> {
        if self.cols == 0 || start <= end {
            return None;
        }
        if !self.tree.planes.is_empty() {
            for hit in cached {
                if let Some(contact)=self.trace_triangle(hit.triangle,x,y,start,end,true,Some(*hit)) {return Some(contact);}
            }
            let hit=crate::mesh_query::trace(&self.tree,[x,y,start],[x,y,end])?;
            if !self.supporting_surfaces.get(hit.surface as usize).copied().unwrap_or(true) {return None;}
            return Some(GroundHit {height:hit.point[2],normal:hit.normal,plane_offset:hit.plane_offset,surface:hit.surface,triangle:hit.triangle});
        }
        let c = ((x - self.min[0]) / CELL).floor();
        let r = ((y - self.min[1]) / CELL).floor();
        if c < 0.0 || r < 0.0 || c as usize >= self.cols || r as usize >= self.rows {
            return None;
        }
        let mut best: Option<GroundHit> = None;
        for hit in cached {
            if let Some(contact) = self.trace_triangle(hit.triangle, x, y, start, end, true,Some(*hit)) {
                return Some(contact);
            }
        }
        for &index in &self.cells[r as usize * self.cols + c as usize] {
            if let Some(contact) = self.trace_triangle(index as usize, x, y, start, end, false,None) {
                if best.is_none_or(|hit| contact.height > hit.height) {
                    best = Some(contact);
                }
            }
        }
        best
    }

    fn trace_triangle(
        &self,
        index: usize,
        x: f32,
        y: f32,
        start: f32,
        end: f32,
        cached: bool,
        stored:Option<GroundHit>,
    ) -> Option<GroundHit> {
        let triangle = &self.tree.mesh.triangles[index];
        if !self.supporting_surfaces.get(triangle.surface as usize).copied().unwrap_or(true) {return None;}
        let [a, b, c] = triangle.indices.map(|i| self.tree.mesh.vertices[i as usize]);
        if self.tree.planes.is_empty() {interpolate_height(a,b,c,x,y)?;}
        let normal = stored.map_or_else(||normal(a,b,c),|hit|hit.normal);
        let plane_offset=stored.map_or_else(||offset(normal,a),|hit|hit.plane_offset);
        if self.tree.planes.is_empty() && normal[2] < MIN_GROUND_NORMAL_Z {
            return None;
        }
        let distance = |z: f32| {
            if cached {
                //00448ae0 uses the triangle plane equation already retained in
                // the wheel cache. Do not subtract a distant vertex first here.
                (f64::from(normal[2])*f64::from(z)+f64::from(normal[1])*f64::from(y))
                    +f64::from(normal[0])*f64::from(x)+f64::from(plane_offset)
            } else {
                //00403fa0 traverses the mesh tree using endpoint minus the
                // first triangle vertex, with binary32 subtraction spills.
                let delta=[x-a[0],y-a[1],z-a[2]];
                f64::from(normal.into_iter().zip(delta).map(|(n,v)|f64::from(n)*f64::from(v)).sum::<f64>() as f32)
            }
        };
        let from = distance(start);
        let to = distance(end);
        // Cached00448b80 classifies zero as the positive halfspace. Initial
        // mesh00403fa0 classifies zero as the negative halfspace instead.
        if if cached {(from>=0.0)==(to>=0.0)} else {from<=0.0 || to>0.0} {
            return None;
        }
        let fraction = from / (from - to);
        let height = if cached {
            //00448b80 retains BOTH distances and the ratio; Z interpolation
            // keeps its product until the final addition/FSTP. X/Y products
            // spill separately, but are zero for these vertical cached rays.
            (f64::from(start) + f64::from(end - start) * fraction) as f32
        } else {
            start + (end - start) * fraction as f32
        };
        if !self.tree.planes.is_empty() && !crate::mesh_query::contains([x,y,height],[a,b,c],normal) {return None;}
        Some(GroundHit {
            height,
            normal,
            plane_offset,
            surface: triangle.surface,
            triangle: index,
        })
    }

    pub(crate) fn trace_cached_hit(&self,hit:GroundHit,x:f32,y:f32,start:f32,end:f32)->Option<GroundHit> {
        if start<=end {return None;}
        self.trace_triangle(hit.triangle,x,y,start,end,true,Some(hit))
    }

    /// Every triangle covering (x, y), steep or not, highest first. For diagnostics.
    pub fn all_at(&self, x: f32, y: f32) -> Vec<GroundHit> {
        if self.cols == 0 {
            return Vec::new();
        }
        let c = ((x - self.min[0]) / CELL).floor();
        let r = ((y - self.min[1]) / CELL).floor();
        if c < 0.0 || r < 0.0 || c as usize >= self.cols || r as usize >= self.rows {
            return Vec::new();
        }
        let mut hits: Vec<GroundHit> = self.cells[r as usize * self.cols + c as usize]
            .iter()
            .filter_map(|&n| {
                let tri = &self.tree.mesh.triangles[n as usize];
                if !self.supporting_surfaces.get(tri.surface as usize).copied().unwrap_or(true) {return None;}
                let [a, b, c] = tri.indices.map(|i| self.tree.mesh.vertices[i as usize]);
                let height = interpolate_height(a, b, c, x, y)?;
                Some(GroundHit {
                    height,
                    normal: normal(a, b, c),
                    plane_offset:offset(normal(a,b,c),a),
                    surface: tri.surface,
                    triangle: n as usize,
                })
            })
            .collect();
        hits.sort_by(|a, b| b.height.total_cmp(&a.height));
        hits
    }

    /// The drivable surface at (x, y): the highest ground triangle not above `ceiling`.
    pub fn at(&self, x: f32, y: f32, ceiling: f32) -> Option<GroundHit> {
        if self.cols == 0 {
            return None;
        }
        let c = ((x - self.min[0]) / CELL).floor();
        let r = ((y - self.min[1]) / CELL).floor();
        if c < 0.0 || r < 0.0 || c as usize >= self.cols || r as usize >= self.rows {
            return None;
        }
        let mut best: Option<GroundHit> = None;
        for &n in &self.cells[r as usize * self.cols + c as usize] {
            let tri = &self.tree.mesh.triangles[n as usize];
            if !self.supporting_surfaces.get(tri.surface as usize).copied().unwrap_or(true) {continue;}
            let [a, b, c] = tri.indices.map(|i| self.tree.mesh.vertices[i as usize]);
            let Some(height) = interpolate_height(a, b, c, x, y) else {
                continue;
            };
            if height > ceiling {
                continue;
            }
            let normal = normal(a, b, c);
            if normal[2] < MIN_GROUND_NORMAL_Z {
                continue;
            }
            if best.map_or(true, |hit| height > hit.height) {
                best = Some(GroundHit {
                    height,
                    normal,
                    plane_offset:offset(normal,a),
                    surface: tri.surface,
                    triangle: n as usize,
                });
            }
        }
        best
    }
}

fn cell_index(value: f32, min: f32, count: usize) -> usize {
    (((value - min) / CELL).floor().max(0.0) as usize).min(count - 1)
}

fn offset(normal:Vec3,point:Vec3)->f32 {
    -(normal.into_iter().zip(point).map(|(n,v)|f64::from(n)*f64::from(v)).sum::<f64>()) as f32
}

fn interpolate_height(a: Vec3, b: Vec3, c: Vec3, x: f32, y: f32) -> Option<f32> {
    let det = (b[1] - c[1]) * (a[0] - c[0]) + (c[0] - b[0]) * (a[1] - c[1]);
    if det.abs() < 1e-9 {
        return None;
    }
    let l1 = ((b[1] - c[1]) * (x - c[0]) + (c[0] - b[0]) * (y - c[1])) / det;
    let l2 = ((c[1] - a[1]) * (x - c[0]) + (a[0] - c[0]) * (y - c[1])) / det;
    let l3 = 1.0 - l1 - l2;
    const EDGE: f32 = -1e-4;
    (l1 >= EDGE && l2 >= EDGE && l3 >= EDGE).then(|| l1 * a[2] + l2 * b[2] + l3 * c[2])
}

/// Upward-facing unit normal (flipped if the triangle winds the other way).
fn normal(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let mut n = crate::contact::cross(u,v);
    if n[2] < 0.0 {
        n = [-n[0], -n[1], -n[2]];
    }
    crate::contact::normalized(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lrformats::world::CollisionTriangle;

    fn slab(top: f32) -> CollisionMesh {
        CollisionMesh {
            names: vec![],
            vertices: vec![
                [0.0, 0.0, top],
                [10.0, 0.0, top],
                [0.0, 10.0, top],
                [10.0, 10.0, top],
            ],
            triangles: vec![
                CollisionTriangle {
                    indices: [0, 1, 2],
                    surface: 7,
                },
                CollisionTriangle {
                    indices: [1, 3, 2],
                    surface: 7,
                },
            ],
        }
    }

    #[test]
    fn finds_height_and_surface_inside_the_mesh_only() {
        let ground = Ground::new(slab(3.0));
        let hit = ground.at(2.0, 2.0, 100.0).unwrap();
        assert_eq!((hit.height, hit.surface), (3.0, 7));
        assert!(hit.normal[2] > 0.99);
        assert!(ground.at(20.0, 2.0, 100.0).is_none());
    }

    #[test]
    fn ignores_surfaces_above_the_ceiling_and_picks_the_highest_below_it() {
        let mut mesh = slab(0.0);
        let upper = slab(8.0);
        mesh.vertices.extend(upper.vertices);
        mesh.triangles
            .extend(upper.triangles.iter().map(|t| CollisionTriangle {
                indices: t.indices.map(|i| i + 4),
                surface: 9,
            }));
        let ground = Ground::new(mesh);
        assert_eq!(ground.at(2.0, 2.0, 100.0).unwrap().height, 8.0);
        assert_eq!(ground.at(2.0, 2.0, 5.0).unwrap().height, 0.0);
    }

    #[test]
    fn interpolates_slopes_and_skips_walls() {
        let mesh = CollisionMesh {
            names: vec![],
            vertices: vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 10.0]],
            triangles: vec![CollisionTriangle {
                indices: [0, 1, 2],
                surface: 0,
            }],
        };
        let ground = Ground::new(mesh);
        let hit = ground.at(1.0, 4.0, 100.0).unwrap();
        assert!((hit.height - 4.0).abs() < 1e-4);
        let wall = CollisionMesh {
            names: vec![],
            vertices: vec![[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 0.1, 10.0]],
            triangles: vec![CollisionTriangle {
                indices: [0, 1, 2],
                surface: 0,
            }],
        };
        assert!(Ground::new(wall).at(1.0, 0.05, 100.0).is_none());
    }
}
