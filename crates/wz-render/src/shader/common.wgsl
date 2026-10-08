// 정점 → 픽셀 inter-stage ABI (단일 진실 원천).
//
// # 왜 별도 파일인가
//
// WGSL 에는 include 가 없다. 그래서 정점 셰이더(sprite.wgsl)와 픽셀
// 셰이더(maplestory.wgsl)를 두 파일로 나누면 **같은 inter-stage 구조체를
// 두 곳에 손으로 적어야 한다.** 한쪽만 고치면 `@location` 이 어긋나고,
// 그 결과는 컴파일이 아니라 **파이프라인 생성 시점의 wgpu validation error**,
// 즉 "화면이 안 나온다" 형태의 런타임 고이다.
//
// 실제로 그 상태였다: `sprite.wgsl` 은 `@location(0) color`, 픽셀 모듈은
// `@location(0) uv` 를 선언했다. 두 개를 함께 쓰면 inter-stage 대입이
// 성립하지 않는다.
//
// 이 정의는 Rust 쪽에서 두 소스에 **문자열로 앞에 붙여서** 합성한다
// (`shader.rs` 의 `assemble()`). 그래서:
//   - 두 모듈의 inter-stage 레이아웃은 **구조적으로 동일**하다
//   - 정점 진입점은 sprite 모듈에만, 픽셀 진입점은 maplestory 모듈에만 있어도 된다
//
// # 바인딩 그룹 규약 (중요)
//
// 원본 C# 는 D3D9 **레지스터 슬롯** 공간을 쓴다: `light` 는 cb2+cb4,
// `waterFront` 는 cb0+cb2, 정점 셰이더는 cb0+cb1. 전부 같은 전역 슬롯 공간의
// 다른 번호이고, 원본 Effect 는 한 번에 하나의 픽셀 셰이더만 바인딩하므로
// 충돌하지 않는다.
//
// wgpu 의 `@group`/`@binding` 은 이와 다르다. **정점과 픽셀이 같은
// 파이프라인에 묶이면 두 단계의 리소스가 한 그룹 공간을 공유**하고,
// 같은 `(group, binding)` 을 서로 다른 리소스 타입으로 선언하면
// 파이프라인 생성이 실패한다:
//
//   Shader global ResourceBinding { group: 0, binding: 0 } is not available
//   in the pipeline layout / Derived bind group layout type is not consistent
//   between stages
//
// 그래서 그룹을 **역할**로 나눈다:
//
// | group | 단계 | 내용 |
// |---|---|---|
// | 0 | 정점 | 프레임 CB (`vp`, `vp_inv`, `resolution_time`, `world`) |
// | 1 | 픽셀 | 재질 CB + 텍스처 + 샘플러 (셰이더별 1~7개) |
//
// 이 규약은 `shader::tests::binding_groups_do_not_collide_across_stages` 가
// IR 에서 기계적으로 강제한다.

// 스프라이트 배치의 정점 입력. 원본 `MsSpriteRenderer` 의 버텍스 버퍼 포맷.
struct VsIn {
    // 원본은 position2/color4/texcoord2 였지만 WGSL 정렬 규칙(vec2 = 8B 정렬)에
    // 맞춰 CPU 쪽이 40바이트(10 float)로 패킹한다:
    //   position  vec4  offset  0  (16B)
    //   color     vec4  offset 16  (16B)
    //   texcoord  vec2  offset 32  ( 8B)
    // → array_stride = 40. 36 으로 잡으면 wgpu 가 파이프라인 생성 시점에 잡는다.
    @location(0) position: vec4<f32>,
    @location(1) color: vec4<f32>,
    @location(2) texcoord: vec2<f32>,
}

// 정점 출력 = 픽셀 입력. **두 모듈이 반드시 이 하나를 공유해야 한다.**
struct VsOut {
    // NDC / 화면 좌표 (프래그먼트에서는 픽셀 좌표로 보간된다)
    @builtin(position) pos: vec4<f32>,
    // 정점 색. 맵 배경 레이어는 1,0 으로 상수가 된다.
    @location(0) color: vec4<f32>,
    // 텍스처 좌표 (아틀라스 UV)
    @location(1) uv: vec2<f32>,
    /// 맵 스프라이트 셰이더들이 수직 그라데이션에 쓰는 월드 Y.
    /// 정점 셰이더가 여기서 뽑아주면 픽셀 셰이더는 보간만 하면 된다.
    @location(2) world_y: f32,
}