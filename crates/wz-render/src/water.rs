//! waterBack / waterFront GPU 유니폼 레이아웃.
//!
//! WGSL `WaterBackUniforms` (176 B) / `WaterFrontUniforms` (352 B) 와
//! 바이트 단위로 일치하도록 오프셋을 직접 기록한다.

/// `WaterBackUniforms` 파라미터.
#[derive(Debug, Clone)]
pub struct WaterBackParams {
    pub min_y: f32,
    pub max_y: f32,
    /// `(color[3], level)` 10세트.
    pub bands: [[f32; 4]; 10],
}

impl Default for WaterBackParams {
    fn default() -> Self {
        let colors = [
            [0.02, 0.10, 0.25, 0.00],
            [0.03, 0.16, 0.34, 0.10],
            [0.05, 0.22, 0.43, 0.20],
            [0.07, 0.29, 0.52, 0.30],
            [0.10, 0.36, 0.61, 0.40],
            [0.13, 0.44, 0.70, 0.50],
            [0.17, 0.52, 0.78, 0.60],
            [0.22, 0.60, 0.86, 0.70],
            [0.28, 0.68, 0.92, 0.80],
            [0.35, 0.76, 0.98, 0.90],
        ];
        Self {
            min_y: -2000.0,
            max_y: 2000.0,
            bands: colors,
        }
    }
}

impl WaterBackParams {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; 176];
        out[0..4].copy_from_slice(&self.min_y.to_le_bytes());
        out[4..8].copy_from_slice(&self.max_y.to_le_bytes());
        for (i, band) in self.bands.iter().enumerate() {
            let off = 16 + i * 16;
            for (j, value) in band.iter().enumerate() {
                out[off + j * 4..off + j * 4 + 4].copy_from_slice(&value.to_le_bytes());
            }
        }
        out
    }
}

/// `WaterFrontUniforms` 파라미터 (필요한 값만 노출).
#[derive(Debug, Clone)]
pub struct WaterFrontParams {
    pub vp: [f32; 16],
    pub vp_inv: [f32; 16],
    pub resolution_time: [f32; 4],
    pub diffuse: [f32; 3],
    pub min_y: f32,
    pub max_y: f32,
    pub strength: f32,
    pub edge: f32,
    pub aberration: f32,
    pub dist_strength: f32,
}

impl Default for WaterFrontParams {
    fn default() -> Self {
        Self {
            vp: glam::Mat4::IDENTITY.to_cols_array(),
            vp_inv: glam::Mat4::IDENTITY.to_cols_array(),
            resolution_time: [800.0, 600.0, 0.0, 0.0],
            diffuse: [0.30, 0.62, 0.90],
            min_y: -2000.0,
            max_y: 2000.0,
            strength: 1.0,
            edge: 0.1,
            aberration: 0.005,
            dist_strength: 0.001,
        }
    }
}

impl WaterFrontParams {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; 352];
        let mut put = |off: usize, v: f32| {
            out[off..off + 4].copy_from_slice(&v.to_le_bytes());
        };
        for (i, v) in self.vp.iter().enumerate() {
            put(i * 4, *v);
        }
        for (i, v) in self.vp_inv.iter().enumerate() {
            put(64 + i * 4, *v);
        }
        for (i, v) in self.resolution_time.iter().enumerate() {
            put(128 + i * 4, *v);
        }
        // cb1 starts at 144.
        put(144, 0.0); // value.x
        put(148, 0.0); // value.y
        put(152, 1.0); // cgrad.x
        put(156, 1.0); // cgrad.y
        put(160, 1.0); // octave.x
        put(164, 1.0); // octave.y
        put(168, self.strength);
        put(172, self.edge);
        put(176, -2000.0); // godray_min_y
        put(180, 4000.0); // godray_max_y
        put(184, 0.0); // godray_move
        put(188, 0.5); // godray_bright
        put(192, 1.0); // godray_color.r
        put(196, 1.0); // godray_color.g
        put(200, 1.0); // godray_color.b
        put(204, 0.0); // godray_uv_rot
        put(208, 1.0); // godray_uv_scale.x
        put(212, 1.0); // godray_uv_scale.y
        put(216, self.aberration);
        put(220, self.dist_strength);
        put(224, self.diffuse[0]);
        put(228, self.diffuse[1]);
        put(232, self.diffuse[2]);
        put(236, -2000.0); // screen_min_y
        put(240, 4000.0); // screen_max_y
        put(244, 0.2); // screen_color.r
        put(248, 0.4); // screen_color.g
        put(252, 0.6); // screen_color.b
                       // cb2 starts at 256.
        put(256, 0.0); // player_pos.x
        put(260, 0.0);
        put(264, 1.0); // distance_factor1
        put(268, 0.0);
        put(272, self.min_y);
        put(276, self.max_y);
        put(280, 0.0); // dist_center_pos.x
        put(284, 0.0);
        put(288, 1.0); // distance_factor2
        put(292, 0.0);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_sizes_match_wgsl() {
        assert_eq!(WaterBackParams::default().to_bytes().len(), 176);
        assert_eq!(WaterFrontParams::default().to_bytes().len(), 352);
    }
}
