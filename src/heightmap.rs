use serde::{Deserialize, Serialize};

/// A 2D heightmap stored as a flat row-major `Vec<f32>` buffer.
///
/// Covers world space `[0, width * scale) × [0, height * scale)`.
/// `width` and `height` are grid-cell counts; `scale` is world units per cell.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeightMap {
    pub data: Vec<f32>,
    pub width: usize,
    pub height: usize,
    pub scale: f32,
}

impl HeightMap {
    pub fn new(width: usize, height: usize, scale: f32) -> Self {
        assert!(width > 0 && height > 0, "dimensions must be positive");
        assert!(scale > 0.0, "scale must be positive");
        Self {
            data: vec![0.0; width * height],
            width,
            height,
            scale,
        }
    }

    #[inline]
    pub fn get(&self, x: usize, z: usize) -> f32 {
        self.data[z * self.width + x]
    }

    #[inline]
    pub fn set(&mut self, x: usize, z: usize, val: f32) {
        self.data[z * self.width + x] = val;
    }

    #[inline]
    pub fn get_clamped(&self, x: i32, z: i32) -> f32 {
        let cx = x.clamp(0, self.width as i32 - 1) as usize;
        let cz = z.clamp(0, self.height as i32 - 1) as usize;
        self.get(cx, cz)
    }

    /// Sample height at world position using bilinear interpolation.
    /// Clamps to heightmap boundaries.
    pub fn get_height_at(&self, world_x: f32, world_z: f32) -> f32 {
        let gx = world_x / self.scale;
        let gz = world_z / self.scale;

        let x0 = gx.floor() as i32;
        let z0 = gz.floor() as i32;
        let fx = gx - x0 as f32;
        let fz = gz - z0 as f32;

        let h00 = self.get_clamped(x0, z0);
        let h10 = self.get_clamped(x0 + 1, z0);
        let h01 = self.get_clamped(x0, z0 + 1);
        let h11 = self.get_clamped(x0 + 1, z0 + 1);

        let h0 = h00 + (h10 - h00) * fx;
        let h1 = h01 + (h11 - h01) * fx;
        h0 + (h1 - h0) * fz
    }

    /// Compute surface normal at world position using central differences.
    /// Returns a normalized `[x, y, z]` vector where `y` is up.
    pub fn get_normal_at(&self, world_x: f32, world_z: f32) -> [f32; 3] {
        let step = self.scale;
        let hl = self.get_height_at(world_x - step, world_z);
        let hr = self.get_height_at(world_x + step, world_z);
        let hd = self.get_height_at(world_x, world_z - step);
        let hu = self.get_height_at(world_x, world_z + step);

        let dhdx = (hr - hl) / (2.0 * step);
        let dhdz = (hu - hd) / (2.0 * step);

        let nx = -dhdx;
        let ny = 1.0_f32;
        let nz = -dhdz;
        let len = (nx * nx + ny * ny + nz * nz).sqrt();
        [nx / len, ny / len, nz / len]
    }

    /// Normalize all height values to `[0.0, 1.0]`.
    pub fn normalize(&mut self) {
        let min = self.data.iter().cloned().fold(f32::INFINITY, f32::min);
        let max = self.data.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let range = max - min;
        if range > f32::EPSILON {
            for v in &mut self.data {
                *v = (*v - min) / range;
            }
        }
    }

    /// World-space width of the heightmap.
    pub fn world_width(&self) -> f32 {
        self.width as f32 * self.scale
    }

    /// World-space depth of the heightmap.
    pub fn world_depth(&self) -> f32 {
        self.height as f32 * self.scale
    }
}
