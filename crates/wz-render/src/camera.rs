//! 2D 직교 카메라 및 WGSL 셰이더 CB ABI 정합 유니폼.
//!
//! # 좌표계 및 단위
//!
//! - 월드 좌표: 메이플스토리 2D 픽셀 좌표 (X: 오른쪽 증가, Y: 아래쪽 증가).
//! - 뷰포트 크기: 렌더링 서피스 폭과 높이 (픽셀).
//! - 줌 (zoom / scale): 1.0 = 1:1 픽셀, 2.0 = 2배 확대.
//!
//! # WGSL CB 바인딩 호환성 (리포트 §9.3 & `sprite.wgsl`)
//!
//! 정점 셰이더 CB0 + CB1 의 메모리 레이아웃:
//! - CB0: `vp` (64B) + `vp_inv` (64B) + `resolution_time` (16B) = 144B
//! - CB1: `world` (64B) = 64B
//! - 합계 208B (`SpriteUniforms`)

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec2};

/// WGSL `SpriteUniforms` 과 100% 바이너리 일치하는 GPU 버퍼 데이터 구조체.
///
/// 오프셋:
/// - 0..64: `vp` (mat4x4<f32>)
/// - 64..128: `vp_inv` (mat4x4<f32>)
/// - 128..144: `resolution_time` (vec4<f32>) = [width, height, time, delta_time]
/// - 144..208: `world` (mat4x4<f32>)
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct SpriteUniformsGpu {
    pub vp: [f32; 16],
    pub vp_inv: [f32; 16],
    pub resolution_time: [f32; 4],
    pub world: [f32; 16],
}

impl Default for SpriteUniformsGpu {
    fn default() -> Self {
        Self {
            vp: Mat4::IDENTITY.to_cols_array(),
            vp_inv: Mat4::IDENTITY.to_cols_array(),
            resolution_time: [800.0, 600.0, 0.0, 0.0],
            world: Mat4::IDENTITY.to_cols_array(),
        }
    }
}

/// 2D 직교 투영 및 추종 카메라.
#[derive(Debug, Clone)]
pub struct Camera2D {
    /// 카메라 중심 위치 (월드 픽셀 좌표).
    pub center: Vec2,
    /// 줌 배율 (기본값 1.0).
    pub zoom: f32,
    /// 뷰포트 해상도 (폭, 높이).
    pub viewport_size: Vec2,
    /// 회전 각도 (라디안, 기본 0.0).
    pub rotation: f32,
}

impl Camera2D {
    pub fn new(viewport_w: f32, viewport_h: f32) -> Self {
        Self {
            center: Vec2::new(viewport_w * 0.5, viewport_h * 0.5),
            zoom: 1.0,
            viewport_size: Vec2::new(viewport_w, viewport_h),
            rotation: 0.0,
        }
    }

    /// 부드러운 위치 추종 (Lerp).
    pub fn lerp_to(&mut self, target: Vec2, factor: f32) {
        let t = factor.clamp(0.0, 1.0);
        self.center = self.center.lerp(target, t);
    }

    /// 뷰 행렬 (World -> View).
    pub fn view_matrix(&self) -> Mat4 {
        // 1. 카메라 중심으로 이동
        // 2. 줌 및 회전 적용
        // 3. 화면 중심으로 다시 이동
        let half_w = self.viewport_size.x * 0.5;
        let half_h = self.viewport_size.y * 0.5;

        Mat4::from_translation(glam::vec3(half_w, half_h, 0.0))
            * Mat4::from_scale(glam::vec3(self.zoom, self.zoom, 1.0))
            * Mat4::from_rotation_z(self.rotation)
            * Mat4::from_translation(glam::vec3(-self.center.x, -self.center.y, 0.0))
    }

    /// 2D 직교 투영 행렬 (View -> NDC).
    ///
    /// Y축 아래쪽이 화면 아래가 되도록 설정 (Y-down).
    pub fn proj_matrix(&self) -> Mat4 {
        Mat4::orthographic_lh(
            0.0,
            self.viewport_size.x,
            self.viewport_size.y,
            0.0,
            -1000.0,
            1000.0,
        )
    }

    /// View-Projection 행렬 (World -> NDC).
    pub fn vp_matrix(&self) -> Mat4 {
        self.proj_matrix() * self.view_matrix()
    }

    /// 스크린 픽셀 좌표 -> 월드 좌표 역변환.
    pub fn screen_to_world(&self, screen_pos: Vec2) -> Vec2 {
        let vp_inv = self.vp_matrix().inverse();
        // screen_pos -> NDC (-1..1)
        let ndc_x = (screen_pos.x / self.viewport_size.x) * 2.0 - 1.0;
        let ndc_y = 1.0 - (screen_pos.y / self.viewport_size.y) * 2.0;

        let ndc_vec = glam::vec4(ndc_x, ndc_y, 0.0, 1.0);
        let world_vec = vp_inv * ndc_vec;
        let w = if world_vec.w.abs() > 1e-6 {
            world_vec.w
        } else {
            1.0
        };

        Vec2::new(world_vec.x / w, world_vec.y / w)
    }

    /// 월드 좌표 -> 스크린 픽셀 좌표 변환.
    pub fn world_to_screen(&self, world_pos: Vec2) -> Vec2 {
        let vp = self.vp_matrix();
        let world_vec = glam::vec4(world_pos.x, world_pos.y, 0.0, 1.0);
        let ndc = vp * world_vec;
        let w = if ndc.w.abs() > 1e-6 { ndc.w } else { 1.0 };

        let norm_x = (ndc.x / w + 1.0) * 0.5;
        let norm_y = (1.0 - ndc.y / w) * 0.5;

        Vec2::new(norm_x * self.viewport_size.x, norm_y * self.viewport_size.y)
    }

    /// GPU 바인딩용 유니폼 빌드.
    pub fn build_uniforms(&self, time_seconds: f32, delta_seconds: f32) -> SpriteUniformsGpu {
        let vp = self.vp_matrix();
        let vp_inv = vp.inverse();

        SpriteUniformsGpu {
            vp: vp.to_cols_array(),
            vp_inv: vp_inv.to_cols_array(),
            resolution_time: [
                self.viewport_size.x,
                self.viewport_size.y,
                time_seconds,
                delta_seconds,
            ],
            world: Mat4::IDENTITY.to_cols_array(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uniform_gpu_memory_layout() {
        assert_eq!(std::mem::size_of::<SpriteUniformsGpu>(), 208);
        assert_eq!(std::mem::align_of::<SpriteUniformsGpu>(), 4);
    }

    #[test]
    fn test_camera_center_projection() {
        let camera = Camera2D::new(800.0, 600.0);
        // 중앙 위치(400, 300)는 스크린 중앙(400, 300)으로 매핑되어야 함
        let screen = camera.world_to_screen(camera.center);
        assert!((screen.x - 400.0).abs() < 1e-4);
        assert!((screen.y - 300.0).abs() < 1e-4);

        // 스크린(400, 300) -> 월드 역변환 시 (400, 300)
        let world = camera.screen_to_world(Vec2::new(400.0, 300.0));
        assert!((world.x - 400.0).abs() < 1e-4);
        assert!((world.y - 300.0).abs() < 1e-4);
    }

    #[test]
    fn test_camera_zoom_and_roundtrip() {
        let mut camera = Camera2D::new(800.0, 600.0);
        camera.zoom = 2.0;
        camera.center = Vec2::new(100.0, 200.0);

        let test_pt = Vec2::new(150.0, 250.0);
        let screen = camera.world_to_screen(test_pt);
        let recovered = camera.screen_to_world(screen);

        assert!((recovered.x - test_pt.x).abs() < 1e-3);
        assert!((recovered.y - test_pt.y).abs() < 1e-3);
    }
}
