//! 2D 라이트맵 및 정점 색상(Vertex Color) 라이팅 모듈.
//!
//! # 배경
//!
//! 메이플스토리의 라이트맵은 DXBC 셰이더나 복잡한 WGSL 없이 `BasicEffect`와
//! 정점 색상(Vertex Color RGBA)의 선형 감쇠 및 메시 합성으로 구현된다 (`LightRenderer.cs:18`).
//! 따라서 추가 셰이더 비용 없이 기존 `SpriteVertex`의 `color` 필드를 통해 효율적으로 렌더링된다.

use crate::batch::{BlendMode, SpriteBatch, SpriteVertex};
use crate::GpuKey;

/// 점광원(Point Light) 정의.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightSource {
    /// 월드 좌표상 위치 (x, y).
    pub pos: [f32; 2],
    /// 내부 반경 (이 반경 이내에서는 100% 밝기 유지).
    pub inner_radius: f32,
    /// 외부 반경 (이 반경 이후에는 감쇠되어 0이 됨).
    pub outer_radius: f32,
    /// 빛의 색상과 세기 [r, g, b, a].
    pub color: [f32; 4],
}

impl LightSource {
    pub fn new(pos: [f32; 2], inner_radius: f32, outer_radius: f32, color: [f32; 4]) -> Self {
        Self {
            pos,
            inner_radius,
            outer_radius,
            color,
        }
    }

    /// 특정 월드 위치에서의 광원 감쇠율 (0.0 ~ 1.0).
    pub fn intensity_at(&self, point: [f32; 2]) -> f32 {
        let dx = point[0] - self.pos[0];
        let dy = point[1] - self.pos[1];
        let dist = (dx * dx + dy * dy).sqrt();

        if self.outer_radius <= self.inner_radius {
            return if dist <= self.inner_radius { 1.0 } else { 0.0 };
        }

        if dist <= self.inner_radius {
            1.0
        } else if dist >= self.outer_radius {
            0.0
        } else {
            let t = (dist - self.inner_radius) / (self.outer_radius - self.inner_radius);
            // 제곱 감쇠로 자연스러운 falloff 구현
            let k = 1.0 - t;
            k * k
        }
    }

    /// 기준 정점 색상에 라이트 감쇠 및 틴트를 곱셈 합성.
    pub fn apply_to_color(&self, point: [f32; 2], base_color: [f32; 4]) -> [f32; 4] {
        let factor = self.intensity_at(point);
        [
            base_color[0] * (1.0 + self.color[0] * factor).clamp(0.0, 1.0),
            base_color[1] * (1.0 + self.color[1] * factor).clamp(0.0, 1.0),
            base_color[2] * (1.0 + self.color[2] * factor).clamp(0.0, 1.0),
            base_color[3],
        ]
    }
}

/// 단일/다중 광원을 포함하는 2D 라이트맵 컨텍스트.
#[derive(Debug, Default, Clone)]
pub struct LightMap {
    pub ambient_color: [f32; 4],
    pub lights: Vec<LightSource>,
}

impl LightMap {
    pub fn new(ambient_color: [f32; 4]) -> Self {
        Self {
            ambient_color,
            lights: Vec::new(),
        }
    }

    pub fn add_light(&mut self, light: LightSource) {
        self.lights.push(light);
    }

    /// 주어진 월드 지점의 최종 라이팅 합성 색상 계산.
    pub fn evaluate_color(&self, point: [f32; 2], base_color: [f32; 4]) -> [f32; 4] {
        let mut r = base_color[0] * self.ambient_color[0];
        let mut g = base_color[1] * self.ambient_color[1];
        let mut b = base_color[2] * self.ambient_color[2];

        for light in &self.lights {
            let intensity = light.intensity_at(point);
            r += base_color[0] * light.color[0] * intensity;
            g += base_color[1] * light.color[1] * intensity;
            b += base_color[2] * light.color[2] * intensity;
        }

        [
            r.clamp(0.0, 1.0),
            g.clamp(0.0, 1.0),
            b.clamp(0.0, 1.0),
            base_color[3],
        ]
    }

    /// 광원을 중심으로 하는 2D 부채꼴/원형 라이트 정점 메시를 빌드하여 배치에 추가.
    ///
    /// 중앙 정점은 광원 색상, 둘레 정점은 알파가 0인 투명 색상으로 그라데이션.
    pub fn append_light_disk(
        &self,
        batch: &mut SpriteBatch,
        white_texture: GpuKey,
        light: &LightSource,
        segments: u32,
        z: f32,
    ) {
        if segments < 3 {
            return;
        }

        let mut verts = Vec::with_capacity((segments + 1) as usize);
        verts.push(SpriteVertex::new(light.pos, z, [0.5, 0.5], light.color));

        let edge_color = [light.color[0], light.color[1], light.color[2], 0.0];
        let angle_step = std::f32::consts::TAU / segments as f32;

        for i in 0..segments {
            let theta = i as f32 * angle_step;
            let px = light.pos[0] + light.outer_radius * theta.cos();
            let py = light.pos[1] + light.outer_radius * theta.sin();
            verts.push(SpriteVertex::new([px, py], z, [0.5, 0.5], edge_color));
        }

        let mut local_indices = Vec::with_capacity((segments * 3) as usize);
        for i in 0..segments {
            let current = 1 + i;
            let next = 1 + ((i + 1) % segments);
            local_indices.extend_from_slice(&[0, current, next]);
        }

        batch.push_triangles(white_texture, BlendMode::Additive, &verts, &local_indices);
    }
}

/// 2D 부채꼴/원형 스포트라이트 (`Light2D`).
///
/// 각도는 원본과 같이 위쪽이 0도, 오른쪽이 90도, 아래가 180도다.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpotLight {
    pub pos: [f32; 2],
    pub inner_radius: f32,
    pub outer_radius: f32,
    pub inner_angle: f32,
    pub outer_angle: f32,
    /// 중심 색상 [r, g, b, a].
    pub color: [f32; 4],
}

impl SpotLight {
    /// 원본 `LightRenderer.DrawSpotLight` 와 동일한 36분할 메시를 배치에 추가한다.
    pub fn append(&self, batch: &mut SpriteBatch, white_texture: GpuKey, z: f32) {
        const ARC_POINTS: usize = 36;
        let mut inner_angle = self.inner_angle;
        let outer_angle = self.outer_angle;
        let is_circle = (inner_angle - outer_angle).abs() < f32::EPSILON;
        if inner_angle <= outer_angle {
            inner_angle += 360.0;
        }
        let rad_start = outer_angle.to_radians();
        let rad_end = inner_angle.to_radians();
        let inc = (rad_end - rad_start) / ARC_POINTS as f32;

        let radius_steps: Vec<f32> = if self.inner_radius <= 0.0 {
            vec![self.outer_radius]
        } else {
            vec![self.inner_radius, self.outer_radius]
        };
        let edge_color = [self.color[0], self.color[1], self.color[2], 0.0];
        let color_steps: Vec<[f32; 4]> = if self.inner_radius <= 0.0 {
            vec![edge_color]
        } else {
            vec![self.color, edge_color]
        };

        let ring_len = ARC_POINTS + usize::from(!is_circle);
        let mut verts = Vec::with_capacity(1 + ring_len * radius_steps.len());
        verts.push(SpriteVertex::new(self.pos, z, [0.5, 0.5], self.color));
        for (s, radius) in radius_steps.iter().enumerate() {
            let color = color_steps[s];
            for i in 0..ring_len {
                let angle = if i == ARC_POINTS {
                    rad_end
                } else {
                    rad_start + inc * i as f32
                };
                let px = self.pos[0] + angle.sin() * radius;
                let py = self.pos[1] - angle.cos() * radius;
                verts.push(SpriteVertex::new([px, py], z, [0.5, 0.5], color));
            }
        }

        let mut indices = Vec::with_capacity(ARC_POINTS * (radius_steps.len() * 2 - 1) * 3);
        let base = |s: usize, i: usize| -> u32 { (1 + s * ring_len + (i % ring_len)) as u32 };
        for i in 0..ARC_POINTS {
            let a = base(0, i);
            let b = base(0, i + 1);
            if radius_steps.len() == 1 {
                indices.extend_from_slice(&[0, a, b]);
            } else {
                indices.extend_from_slice(&[0, a, b]);
                let c = base(1, i);
                let d = base(1, i + 1);
                indices.extend_from_slice(&[a, c, d, a, d, b]);
            }
        }

        batch.push_triangles(white_texture, BlendMode::Additive, &verts, &indices);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_light_falloff_calculation() {
        let light = LightSource::new([100.0, 100.0], 20.0, 100.0, [1.0, 1.0, 1.0, 1.0]);

        // 내부 반경 안
        assert_eq!(light.intensity_at([100.0, 110.0]), 1.0);

        // 외부 반경 밖
        assert_eq!(light.intensity_at([100.0, 250.0]), 0.0);

        // 중간 지점 (선형 중간 t=0.5 -> k^2 = 0.25)
        let mid = light.intensity_at([100.0, 160.0]);
        assert!((mid - 0.25).abs() < 1e-4);
    }

    #[test]
    fn test_lightmap_evaluation() {
        let mut lightmap = LightMap::new([0.2, 0.2, 0.2, 1.0]); // 어두운 앰비언트
        lightmap.add_light(LightSource::new(
            [0.0, 0.0],
            10.0,
            50.0,
            [0.8, 0.8, 0.8, 1.0],
        ));

        // 빛 바로 중심에서는 앰비언트(0.2) + 광원(0.8) = 1.0
        let evaluated = lightmap.evaluate_color([0.0, 0.0], [1.0, 1.0, 1.0, 1.0]);
        assert!((evaluated[0] - 1.0).abs() < 1e-4);
        assert!((evaluated[1] - 1.0).abs() < 1e-4);
        assert!((evaluated[2] - 1.0).abs() < 1e-4);

        // 빛 영향권 밖에서는 앰비언트(0.2)만 남음
        let evaluated_far = lightmap.evaluate_color([100.0, 100.0], [1.0, 1.0, 1.0, 1.0]);
        assert!((evaluated_far[0] - 0.2).abs() < 1e-4);
    }

    #[test]
    fn test_spot_light_mesh_matches_original_counts() {
        let spot = SpotLight {
            pos: [10.0, 20.0],
            inner_radius: 30.0,
            outer_radius: 100.0,
            inner_angle: 0.0,
            outer_angle: 90.0,
            color: [1.0, 0.9, 0.5, 0.8],
        };
        let mut batch = SpriteBatch::new();
        spot.append(&mut batch, GpuKey(9), 5.0);
        batch.finish();
        // inner+outer ring, 37 points each (36 + closing point) + center.
        assert_eq!(batch.vertices.len(), 1 + 37 * 2);
        // 36 fan triangles + 36 quads (72 triangles) = 108 triangles.
        assert_eq!(batch.indices.len(), 108 * 3);
        assert_eq!(batch.commands[0].blend_mode, BlendMode::Additive);
        assert_eq!(batch.commands[0].texture_key, GpuKey(9));
    }

    #[test]
    fn test_circle_spot_light_has_no_duplicate_closing_point() {
        let spot = SpotLight {
            pos: [0.0, 0.0],
            inner_radius: 0.0,
            outer_radius: 50.0,
            inner_angle: 30.0,
            outer_angle: 30.0,
            color: [1.0, 1.0, 1.0, 1.0],
        };
        let mut batch = SpriteBatch::new();
        spot.append(&mut batch, GpuKey(9), 0.0);
        batch.finish();
        assert_eq!(batch.vertices.len(), 1 + 36);
        assert_eq!(batch.indices.len(), 36 * 3);
    }

    #[test]
    fn test_append_light_disk_records_draw_command() {
        let lightmap = LightMap::new([0.2, 0.2, 0.2, 1.0]);
        let light = LightSource::new([50.0, 50.0], 10.0, 40.0, [1.0, 0.9, 0.5, 1.0]);
        let mut batch = SpriteBatch::new();
        lightmap.append_light_disk(&mut batch, GpuKey(7), &light, 8, 0.0);
        batch.finish();

        assert_eq!(batch.vertices.len(), 9); // 중심 1 + 둘레 8
        assert_eq!(batch.indices.len(), 24); // 삼각형 8 * 3
        assert_eq!(batch.commands.len(), 1);
        assert_eq!(batch.commands[0].texture_key, GpuKey(7));
        assert_eq!(batch.commands[0].blend_mode, BlendMode::Additive);
        assert_eq!(batch.commands[0].index_start, 0);
        assert_eq!(batch.commands[0].index_count, 24);
    }
}
