// 스프라이트 정점 셰이더 (WGSL 재작성)
//
// 원본: `vs_position_color_texture` — 게임 클라이언트 셰이더 `off_42b030`
//      (KMST1186, 정점 단계)
//
// CB0 (144 B): vp, vp_inv, resolution_time
// CB1 ( 64 B): world
//
// 파라미터 바인딩 근거:
//   EffectResources.cs:220-252  (NativeShaderDesc "vs_position_color_texture")
//   MsSpriteRenderer.cs:25       ("vs_position_color_texture" 로 생성)
//
// 원본 C# 호출:
//   this.vertexShader = EffectResources.CreateNativeShader(graphicsDevice, "vs_position_color_texture");
//
// # CB 레이아웃이 ABI 인 이유
//
// WZ 맵 데이터의 `world` 변환과 `vp` (뷰-투영) 행렬이 이 CB 를 통해 들어온다.
// 레이아웃이 틀리면 모든 스프라이트가 엉뚱한 자리에 그려지므로
// `EffectResources.cs` 의 오프셋을 1바이트 단위로 그대로 재현한다.
//
// `vp_inv` / `resolution_time` 은 원본 픽셀 셰이더들이 쓰지만 이 정점
// 셰이더에서는 소비하지 않는다. ABI 호환을 위해 CB 에 남겨둔다.

struct SpriteUniforms {
    // cb0 (slot 0) — 144 B
    vp: mat4x4<f32>,            // +0
    vp_inv: mat4x4<f32>,        // +64
    resolution_time: vec4<f32>, // +128

    // cb1 (slot 1) — 64 B
    world: mat4x4<f32>,         // +0
}

@group(0) @binding(0) var<uniform> s: SpriteUniforms;

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;

    let world_pos = s.world * in.position;
    out.pos = s.vp * world_pos;
    out.color = in.color;
    out.uv = in.texcoord;
    out.world_y = world_pos.y;

    return out;
}
