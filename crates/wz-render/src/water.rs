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

/// `WaterFrontUniforms` 파라미터.
#[derive(Debug, Clone)]
pub struct WaterFrontParams {
    pub vp: [f32; 16],
    pub vp_inv: [f32; 16],
    pub resolution_time: [f32; 4],
    pub value: [f32; 2],
    pub cgrad: [f32; 2],
    pub octave: [f32; 2],
    pub strength: f32,
    pub edge: f32,
    pub godray_min_y: f32,
    pub godray_max_y: f32,
    pub godray_move: f32,
    pub godray_bright: f32,
    pub godray_color: [f32; 3],
    pub godray_uv_rot: f32,
    pub godray_uv_scale: [f32; 2],
    pub aberration: f32,
    pub dist_strength: f32,
    pub diffuse: [f32; 3],
    pub screen_min_y: f32,
    pub screen_max_y: f32,
    pub screen_color: [f32; 3],
    pub min_y: f32,
    pub max_y: f32,
    pub distance_factor1: f32,
    pub distance_factor2: f32,
}

impl Default for WaterFrontParams {
    fn default() -> Self {
        Self {
            vp: glam::Mat4::IDENTITY.to_cols_array(),
            vp_inv: glam::Mat4::IDENTITY.to_cols_array(),
            resolution_time: [800.0, 600.0, 0.0, 0.0],
            value: [0.0, 0.04],
            cgrad: [1.0, 1.0],
            octave: [1.0, 1.0],
            strength: 1.0,
            edge: 0.1,
            godray_min_y: -2000.0,
            godray_max_y: 4000.0,
            godray_move: 0.0,
            godray_bright: 0.5,
            godray_color: [1.0, 1.0, 1.0],
            godray_uv_rot: 0.0,
            godray_uv_scale: [1.0, 1.0],
            aberration: 0.005,
            dist_strength: 0.001,
            diffuse: [0.30, 0.62, 0.90],
            screen_min_y: -2000.0,
            screen_max_y: 4000.0,
            screen_color: [0.2, 0.4, 0.6],
            min_y: -2000.0,
            max_y: 2000.0,
            distance_factor1: 1.0,
            distance_factor2: 1.0,
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
        put(144, self.value[0]);
        put(148, self.value[1]);
        put(152, self.cgrad[0]);
        put(156, self.cgrad[1]);
        put(160, self.octave[0]);
        put(164, self.octave[1]);
        put(168, self.strength);
        put(172, self.edge);
        put(176, self.godray_min_y);
        put(180, self.godray_max_y);
        put(184, self.godray_move);
        put(188, self.godray_bright);
        put(192, self.godray_color[0]);
        put(196, self.godray_color[1]);
        put(200, self.godray_color[2]);
        put(204, self.godray_uv_rot);
        put(208, self.godray_uv_scale[0]);
        put(212, self.godray_uv_scale[1]);
        put(216, self.aberration);
        put(220, self.dist_strength);
        put(224, self.diffuse[0]);
        put(228, self.diffuse[1]);
        put(232, self.diffuse[2]);
        put(236, self.screen_min_y);
        put(240, self.screen_max_y);
        put(244, self.screen_color[0]);
        put(248, self.screen_color[1]);
        put(252, self.screen_color[2]);
        put(256, 0.0);
        put(260, 0.0);
        put(264, self.distance_factor1);
        put(268, 0.0);
        put(272, self.min_y);
        put(276, self.max_y);
        put(280, 0.0);
        put(284, 0.0);
        put(288, self.distance_factor2);
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
