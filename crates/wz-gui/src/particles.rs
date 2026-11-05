//! 맵 파티클 렌더링 (`ParticleSystem.cs`, `ParticleEmitter.cs`).
//!
//! 원본은 매 프레임 파티클을 생성/적분하지만, 여기서는 같은 결과를 내는
//! **정적 샘플링**을 쓴다. 각 슬롯은 `life` 주기로 순환하고, 시간 오프셋을
//! 균등 배치해 총 개수가 일정하게 유지된다. 렌더 결과는 시간에 대해
//! 결정적이므로 테스트/스크린샷 비교가 가능하다.

use wz_db::particle::ParticleDesc;

/// `Effect/particle.img/<name>` 설명과 실제 텍스처를 묶은 렌더 레이어.
#[derive(Debug, Clone)]
pub struct MapParticleLayer {
    pub desc: ParticleDesc,
    pub texture: (u32, u32, Vec<u8>),
    pub origin: (i32, i32),
    pub points: Vec<(i32, i32)>,
    pub z: i32,
    pub rx: i32,
    pub ry: i32,
}

/// 렌더 준비가 끝난 파티클 한 개.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParticleSprite {
    pub center: [f32; 2],
    pub half: [f32; 2],
    pub rotation: f32,
    pub color: [f32; 4],
}

fn hash_u32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x7feb_352d);
    x ^= x >> 15;
    x = x.wrapping_mul(0x846c_a68b);
    x ^= x >> 16;
    x
}

/// [0, 1) 결정적 난수.
fn rand01(seed: u32) -> f32 {
    (hash_u32(seed) & 0xFFFF) as f32 / 65536.0
}

fn rand_var(seed: u32, base: f32, var: f32) -> f32 {
    if var == 0.0 {
        return base;
    }
    base + var * (rand01(seed) * 2.0 - 1.0)
}

fn argb(packed: u32) -> [f32; 4] {
    let b = (packed & 0xFF) as f32 / 255.0;
    let g = ((packed >> 8) & 0xFF) as f32 / 255.0;
    let r = ((packed >> 16) & 0xFF) as f32 / 255.0;
    let a = ((packed >> 24) & 0xFF) as f32 / 255.0;
    [r, g, b, a]
}

fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

fn lerp_color(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    [
        lerp(a[0], b[0], t),
        lerp(a[1], b[1], t),
        lerp(a[2], b[2], t),
        lerp(a[3], b[3], t),
    ]
}

fn alpha_from_points(desc: &ParticleDesc, start_alpha: f32, end_alpha: f32, t: f32) -> f32 {
    let mut points: Vec<(f32, f32)> = Vec::new();
    if desc.middle_point0 > 0 {
        points.push((
            desc.middle_point0 as f32 / 100.0,
            desc.middle_point_alpha0 as f32 / 255.0,
        ));
    }
    if desc.middle_point1 > 0 {
        points.push((
            desc.middle_point1 as f32 / 100.0,
            desc.middle_point_alpha1 as f32 / 255.0,
        ));
    }
    if points.is_empty() {
        return lerp(start_alpha, end_alpha, t);
    }
    let mut prev_t = 0.0;
    let mut prev_a = start_alpha;
    for (pt, pa) in &points {
        if t < *pt {
            if (*pt - prev_t).abs() < f32::EPSILON {
                return *pa;
            }
            return lerp(prev_a, *pa, (t - prev_t) / (*pt - prev_t));
        }
        prev_t = *pt;
        prev_a = *pa;
    }
    if (1.0 - prev_t).abs() < f32::EPSILON {
        return end_alpha;
    }
    lerp(prev_a, end_alpha, (t - prev_t) / (1.0 - prev_t))
}

/// 레이어 하나를 현재 시각 기준으로 샘플링한다.
///
/// `camera_center` 는 원본 `GetMeshParticle` 의 `Camera.Center * (100+rx)/100`
/// 위치 보정에 쓰인다.
pub fn sample_layer(
    layer: &MapParticleLayer,
    time: f64,
    camera_center: [f32; 2],
) -> Vec<ParticleSprite> {
    let desc = &layer.desc;
    let total = desc.total_particle.clamp(0, 200) as u32;
    if total == 0 || layer.points.is_empty() {
        return Vec::new();
    }
    let life = desc.life.max(0.05);
    let life_var = desc.life_var.max(0.0);
    let base_pos = [
        camera_center[0] * (100.0 + layer.rx as f32) / 100.0,
        camera_center[1] * (100.0 + layer.ry as f32) / 100.0,
    ];
    let tex_len = ((layer.texture.0 * layer.texture.0 + layer.texture.1 * layer.texture.1) as f32)
        .sqrt()
        / std::f32::consts::SQRT_2;
    let tex_len = tex_len.max(1.0);
    let gravity = desc.gravity.clone().unwrap_or_default();

    let mut out = Vec::with_capacity(layer.points.len() * total as usize);
    for (point_index, (px, py)) in layer.points.iter().enumerate() {
        for i in 0..total {
            let seed = (point_index as u32)
                .wrapping_mul(0x9E37_79B9)
                .wrapping_add(i.wrapping_mul(0x85EB_CA6B))
                .wrapping_add(0x27D4_EB2F);
            let individual_life = (life + life_var * (rand01(seed) * 2.0 - 1.0)).max(0.05);
            let phase = (i as f32 / total as f32) * individual_life;
            let elapsed = time as f32;
            if desc.duration > 0.0 && elapsed > desc.duration {
                continue;
            }
            let age = (elapsed + phase) % individual_life;
            let t = (age / individual_life).clamp(0.0, 1.0);

            let speed = rand_var(seed ^ 0xA511_E9B3, gravity.speed, gravity.speed_var);
            let angle = rand_var(seed ^ 0x1B56_C4E9, desc.angle, desc.angle_var);
            let rad = angle.to_radians();
            let start = [
                *px as f32 + rand_var(seed ^ 0x7F4A_7C15, desc.pos_x, desc.pos_var_x),
                *py as f32 + rand_var(seed ^ 0x94D0_49BB, desc.pos_y, desc.pos_var_y),
            ];
            let dir = [rad.cos(), -rad.sin()];
            let g = &gravity;
            let pos = [
                base_pos[0] + start[0] + dir[0] * speed * age + 0.5 * g.x * age * age,
                base_pos[1] + start[1] + dir[1] * speed * age + 0.5 * g.y * age * age,
            ];

            let start_size =
                rand_var(seed ^ 0x2545_F491, desc.start_size, desc.start_size_var).max(0.0);
            let end_size = rand_var(seed ^ 0x68E3_1DA9, desc.end_size, desc.end_size_var).max(0.0);
            let size = lerp(start_size, end_size, t);
            let scale = size / tex_len;

            let start_spin = rand_var(seed ^ 0xC2B2_AE35, desc.start_spin, desc.start_spin_var);
            let end_spin = rand_var(seed ^ 0x1656_67B1, desc.end_spin, desc.end_spin_var);
            let rotation = if g.rotation_is_dir {
                rad + (end_spin - start_spin).to_radians() * t
            } else {
                lerp(start_spin, end_spin, t).to_radians()
            };

            let mut color = lerp_color(argb(desc.start_color), argb(desc.end_color), t);
            color[3] =
                alpha_from_points(desc, argb(desc.start_color)[3], argb(desc.end_color)[3], t);
            if desc.opacity_modify_rgb && color[0] == 0.0 && color[1] == 0.0 && color[2] == 0.0 {
                color[0] = color[3];
                color[1] = color[3];
                color[2] = color[3];
            }

            // 텍스처 origin 보정 (원본 SpriteBatch.Draw origin 인자).
            let origin_dx = (layer.texture.0 as f32 * 0.5 - layer.origin.0 as f32) * scale;
            let origin_dy = (layer.texture.1 as f32 * 0.5 - layer.origin.1 as f32) * scale;
            let (sin_r, cos_r) = rotation.sin_cos();
            let center = [
                pos[0] + origin_dx * cos_r - origin_dy * sin_r,
                pos[1] + origin_dx * sin_r + origin_dy * cos_r,
            ];
            out.push(ParticleSprite {
                center,
                half: [
                    layer.texture.0 as f32 * scale * 0.5,
                    layer.texture.1 as f32 * scale * 0.5,
                ],
                rotation,
                color,
            });
        }
    }
    out
}

/// 파티클 정점 4개 + 인덱스 6개를 배치에 추가한다.
pub fn append_sprite(
    batch: &mut wz_render::SpriteBatch,
    texture_key: wz_render::GpuKey,
    sprite: &ParticleSprite,
    z: f32,
    blend: wz_render::BlendMode,
) {
    let (sin, cos) = sprite.rotation.sin_cos();
    let hw = sprite.half[0];
    let hh = sprite.half[1];
    let corners = [(-hw, -hh), (hw, -hh), (hw, hh), (-hw, hh)];
    let uvs = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
    let mut verts = Vec::with_capacity(4);
    for (i, (x, y)) in corners.iter().enumerate() {
        let rx = x * cos - y * sin;
        let ry = x * sin + y * cos;
        verts.push(wz_render::SpriteVertex::new(
            [sprite.center[0] + rx, sprite.center[1] + ry],
            z,
            [uvs[i].0, uvs[i].1],
            sprite.color,
        ));
    }
    batch.push_triangles(texture_key, blend, &verts, &[0, 1, 2, 0, 2, 3]);
}

/// 원본 블렌드 함수 조합을 우리 BlendMode 로 근사한다.
pub fn blend_mode(desc: &ParticleDesc) -> wz_render::BlendMode {
    match (desc.blend_func_src, desc.blend_func_dst) {
        // SRC_ALPHA + ONE / DEST_ALPHA
        (5, 2) | (5, 7) => wz_render::BlendMode::Additive,
        // ZERO + INV_SRC_ALPHA
        (1, 6) => wz_render::BlendMode::Alpha,
        _ => wz_render::BlendMode::Alpha,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn desc() -> ParticleDesc {
        ParticleDesc {
            name: "fog".into(),
            total_particle: 4,
            angle: 90.0,
            start_color: 0x80_7F_7F_7F,
            end_color: 0,
            start_size: 100.0,
            end_size: 100.0,
            life: 2.0,
            pos_x: 0.0,
            pos_y: 0.0,
            blend_func_src: 5,
            blend_func_dst: 2,
            ..Default::default()
        }
    }

    fn layer() -> MapParticleLayer {
        MapParticleLayer {
            desc: desc(),
            texture: (10, 10, vec![255; 400]),
            origin: (5, 5),
            points: vec![(0, 0)],
            z: 0,
            rx: 0,
            ry: 0,
        }
    }

    #[test]
    fn sampling_is_deterministic_and_bounded() {
        let layer = layer();
        let a = sample_layer(&layer, 1.25, [0.0, 0.0]);
        let b = sample_layer(&layer, 1.25, [0.0, 0.0]);
        assert_eq!(a, b);
        assert_eq!(a.len(), 4);
        assert!(a.iter().all(|p| p.half[0] > 0.0 && p.half[1] > 0.0));
    }

    #[test]
    fn camera_parallax_moves_particles() {
        let mut layer = layer();
        layer.rx = 50;
        layer.ry = -50;
        let still = sample_layer(&layer, 0.0, [0.0, 0.0]);
        let moved = sample_layer(&layer, 0.0, [100.0, 200.0]);
        assert_eq!(still.len(), moved.len());
        assert!(moved[0].center[0] != still[0].center[0]);
        assert!(moved[0].center[1] != still[0].center[1]);
    }

    #[test]
    fn finite_duration_stops() {
        let mut layer = layer();
        layer.desc.duration = 0.5;
        assert!(!sample_layer(&layer, 0.25, [0.0, 0.0]).is_empty());
        assert!(sample_layer(&layer, 1.0, [0.0, 0.0]).is_empty());
    }

    #[test]
    fn blend_combination_is_interpreted() {
        let mut d = desc();
        assert_eq!(blend_mode(&d), wz_render::BlendMode::Additive);
        d.blend_func_src = 5;
        d.blend_func_dst = 6;
        assert_eq!(blend_mode(&d), wz_render::BlendMode::Alpha);
    }
}
