//! wz-render — wgpu 2D 렌더러.
//!
//! # 역할
//!
//! 원본 C# 의 렌더링 스택(MonoGame + SharpDX + Direct2D + DXBC 위조)을
//! wgpu 하나로 대체한다. 백엔드는 Vulkan / Metal / DX12 로 크로스플랫폼
//! (리포트 §9.3).
//!
//! # 계층 규칙
//!
//! 리포트 ADR-4: `wgpu` 타입은 **크레이트 경계를 넘지 않는다.**
//! 도메인(Tier 1~2)은 이 크레이트를 모른다.
//! - `Texture2D` 대신 `GpuKey` (불투명 키, 실제 리소스는 이 크레이트가 소유)
//! - `SpriteBatch` 같은 구조는 GUI/렌더러 내부에만 존재
//!
//! # 진행 상태
//!
//! | 모듈 | 상태 |
//! |---|---|
//! | [`shader`] — WGSL 4종 + 파이프라인 정의 + naga 검증 | ✅ |
//! | [`device`] — wgpu 디바이스/어댑터/서피스 | ✅ |
//! | [`batch`] — 2D 배치기 (텍스처·블렌드 단위 batch break) | ✅ |
//! | [`atlas`] — 텍스처 아틀라스 (shelf packer) | ✅ |
//! | [`camera`] — 2D 카메라 + follow/lerp | ✅ |
//! | [`light`] — 라이트맵 (vertex color, 커스텀 셰이더 불필요) | ✅ |
//!
//! 라이트맵은 원본도 `BasicEffect` + 정점 색상이라 **DXBC 나 WGSL 이 필요 없다**
//! (`LightRenderer.cs:18`).

pub mod atlas;
pub mod batch;
pub mod camera;
pub mod device;
pub mod light;
pub mod shader;

pub use atlas::{AtlasPage, AtlasShelfPacker};
pub use batch::{DrawCommand, SpriteBatch, SpriteVertex};
pub use camera::{Camera2D, SpriteUniformsGpu};
pub use device::RenderContext;
pub use light::{LightMap, LightSource};
pub use shader::{
    all_pipelines, pipeline_table, MaplePixelShader, SpritePipelineDesc, SpriteVertexShader,
};

/// GPU 리소스 불투명 키.
///
/// 도메인(Tier 1~2)이 렌더러에 자원을 요청할 때 쓰는stable한 식별자.
/// 실제 `wgpu::TextureView` 는 이 크레이트 내부에만 존재한다 (리포트 ADR-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GpuKey(pub u32);

/// 텍스처 어틀라이스lot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuSlot {
    pub key: GpuKey,
    /// 어틀라스 텍스처 내 사각형.
    pub rect: AtlasRect,
}

/// 어틀라스 내 사각형 (uv + 픽셀 크기).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AtlasRect {
    pub x: u32,
    pub y: u32,
    pub w: u32,
    pub h: u32,
}

impl AtlasRect {
    /// 픽셀 좌표 → UV.
    pub fn uv(&self, atlas_w: u32, atlas_h: u32) -> [f32; 4] {
        [
            self.x as f32 / atlas_w as f32,
            self.y as f32 / atlas_h as f32,
            (self.x + self.w) as f32 / atlas_w as f32,
            (self.y + self.h) as f32 / atlas_h as f32,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_pipelines_cover_three_shaders() {
        // DXBC 감사 결론: 사용되는 셰이더는 3개.
        assert_eq!(all_pipelines().len(), 3);
    }

    #[test]
    fn atlas_rect_uv_is_normalized() {
        let r = AtlasRect {
            x: 0,
            y: 0,
            w: 512,
            h: 256,
        };
        let uv = r.uv(1024, 512);
        assert_eq!(uv, [0.0, 0.0, 0.5, 0.5]);

        let r2 = AtlasRect {
            x: 512,
            y: 256,
            w: 512,
            h: 256,
        };
        assert_eq!(r2.uv(1024, 512), [0.5, 0.5, 1.0, 1.0]);
    }

    #[test]
    fn atlas_rect_uv_non_square_atlas() {
        // 실제 어틀라스는 정사가 아닐 수 있다 (leftover packing)
        let r = AtlasRect {
            x: 100,
            y: 50,
            w: 200,
            h: 100,
        };
        let uv = r.uv(1024, 2048);
        assert!((uv[0] - 100.0 / 1024.0).abs() < 1e-6);
        assert!((uv[1] - 50.0 / 2048.0).abs() < 1e-6);
        assert!((uv[2] - 300.0 / 1024.0).abs() < 1e-6);
        assert!((uv[3] - 150.0 / 2048.0).abs() < 1e-6);
    }

    #[test]
    fn gpu_key_is_hashable_and_ordered() {
        use std::collections::HashSet;
        let mut s = HashSet::new();
        for i in 0..100u32 {
            s.insert(GpuKey(i));
        }
        assert_eq!(s.len(), 100);

        let mut v = vec![GpuKey(3), GpuKey(1), GpuKey(2)];
        v.sort();
        assert_eq!(v, vec![GpuKey(1), GpuKey(2), GpuKey(3)]);
    }
}
