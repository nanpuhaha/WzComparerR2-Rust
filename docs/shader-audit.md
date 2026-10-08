# DXBC 셰이더 감사 + WGSL 재작성

리포트 §6.9 / §12 R1 대응.

> **결론 먼저: 추출된 DXBC 23개 중 실제로 사용되는 것은 3개뿐이다.**
> 법적 노출 범위가 23 → 3 으로 줄어든다. 나머지 20개는 **번들에서 제외**한다.

---

## 1. 감사 결과 — 무엇이 실제로 쓰이는가

`MsCustomSprite` 렌더링 경로의 유일한 진입점:

```csharp
// WzComparerR2.Common/Rendering/MsSpriteRenderer.cs:128
pixelShader = EffectResources.CreateNativeShader(this.GraphicsDevice, shaderMaterial.ShaderID);
```

`shaderMaterial` 을 만드는 팩토리:

```csharp
// WzComparerR2.MapRender/Effects/ShaderMaterials.cs:18-27
public static ShaderMaterial Create(string shaderID) {
    return shaderID switch {
        "light"      => new LightPixelShaderMaterial(),
        "waterBack"  => new WaterBackPixelShaderMaterial(),
        "waterFront" => new WaterFrontPixelShaderMaterial(),
        _ => throw new Exception($"Unsupported shader: {shaderID}"),
    };
}
```

**세 개만 supported.** 나머지 ID 는 `throw` 난다.
`NativeShaderDesc` 도 딱 5개만 등록돼 있다 (`EffectResources.cs:17,37,86,128,220`).

### 23개 바이너리 전수 조사

| 바이너리 | 크기 | `NativeShaderDesc` | `Create()` 지원 | 판정 |
|---|---:|:---:|:---:|---|
| `light` | 1,828 | ✅ | ✅ | **사용** |
| `waterBack` | 1,536 | ✅ | ✅ | **사용** |
| `waterFront` | 3,872 | ✅ | ✅ | **사용** |
| `default` | 796 | ✅ | ❌ | 미사용 (아래) |
| `vs_position_color_texture` | 1,668 | ✅ | n/a (정점) | **정점 셰이더로 사용** |
| `alpha` | 996 | ❌ | ❌ | 미사용 |
| `alphaMap` | 1,676 | ❌ | ❌ | 미사용 |
| `alphaMaps` | 1,312 | ❌ | ❌ | 미사용 |
| `blurRadial` | 5,848 | ❌ | ❌ | 미사용 |
| `blurX` | 7,272 | ❌ | ❌ | 미사용 |
| `blurY` | 7,272 | ❌ | ❌ | 미사용 |
| `colorModulate` | 1,012 | ❌ | ❌ | 미사용 |
| `glitch` | 9,932 | ❌ | ❌ | 미사용 |
| `grayScale` | 1,052 | ❌ | ❌ | 미사용 |
| `GrayTintWorldMask` | 1,184 | ❌ | ❌ | 미사용 |
| `lens` | 2,836 | ❌ | ❌ | 미사용 (아래) |
| `pointLight` | 1,376 | ❌ | ❌ | 미사용 |
| `refract` | 1,364 | ❌ | ❌ | 미사용 |
| `rope` | 1,164 | ❌ | ❌ | 미사용 (아래) |
| `textureBinaryMask` | 1,236 | ❌ | ❌ | 미사용 |
| `textureMask` | 1,008 | ❌ | ❌ | 미사용 |
| `wave` | 1,620 | ❌ | ❌ | 미사용 (아래) |

**`default` 가 미사용인 이유**: `MsSpriteRenderer.cs:128` 은 항상
`ShaderMaterialFactory` 를 거치고, 거기에 `default` 분기가 없다.
`EffectResources` 에 등록돼 있을 뿐 호출 경로가 없다.

### 오탐 방지 — 동일 문자열의 다른 용도

단순 문자열 검색은 오탐이 많았다. 아래는 **셰이더 ID 가 아닌** 같은 문자열들:

| 문자열 | 실제 용도 | 위치 |
|---|---|---|
| `alpha` | `PngEffect` 의 매개변수 이름 | `FrameAnimationData.cs:558` |
| `lens` | 프라즈마 자원 유형 열거값 | `AvatarPartButtonItem.cs:80` |
| `rope` | 맵 액션 이름 | `AvatarCanvas.cs:2559`, `AvatarForm.cs:829` |
| `wave` | RIFF 포맷 태그 `"WAVE"` | `Wz_Sound.cs:144` |
| `default` | 액션 폴백 이름 | `AvatarCanvas.cs:763,780,798,884,902` |

### 라이트맵 셰이더가 필요 없는 이유

```csharp
// WzComparerR2.MapRender/LightRenderer.cs:18
this.effect = new BasicEffect(graphicsDevice);
```

라이트맵은 커스텀 DXBC 가 **아니라** MonoGame `BasicEffect` + 정점 색상으로
그려진다. `light` 픽셀 셰이더는 `MsCustomSprite` (맵 배경 효과) 용이다.

**즉 라이트맵 재구성은 셰이더 작업이 전혀 필요 없다.**

---

## 2. 법적 처리 — 결론

| 항목 | 처리 |
|---|---|
| 추출 DXBC 20개 (미사용) | **번들에서 제외.** 코드 참조도 문서 참조도 제거 |
| 추출 DXBC 3개 (사용 중) | **번들에서 제외.** WGSL 재작성본만 사용 |
| `default` / `vs_position_color_texture` | **번들에서 제외.** WGSL 재작성 |
| `Rendering/EffectCompiler/` (792 LOC) | **삭제** — D3DX9 효과 바이너리 재구성 machinery |

`deny.toml` 에 바이너리 셰이더 유입 차단 규칙이 이미 있다. 이 결정은
`docs/known-bugs.md` #19 를 "⚠️ → ✅ 처리" 로 갱신한다.

> **원본 게임 클라이언트 셰이더와 픽셀 단위 일치를 목표로 하지 않는다.**
> 파라미터 이름·의미·레이아웃은 재현하되, 구현은 원본 WGSL 이다.
> 시각적 근사가 목적이고, 바이트 재사용이 아니다.

---

## 3. WGSL 재작성 — 파라미터 계약

각 셰이더의 CB 레이아웃은 `EffectResources.cs` 의 `NativeShaderDesc` 에서
옮겼다. **이 레이아웃은 WZ 맵 데이터와 연결되는 ABI** 이므로 정확히 재현한다.

### 3.1 `light` — 픽셀이 아닌 라이트?

아니, 픽셀 셰이더다. `src_tex` 를 받아 조명 색을 합성한다.

```wgsl
// cb0 (slot 2, 16 B)
struct LightCb2 { z: f32 }

// cb1 (slot 4, 96 B)  ← "cb1" 인덱스는 EffectResources.cs:63 주석의 재매핑 표기
struct LightCb4 {
    player_pos        : vec2<f32>,  // +0
    light_inner_radius: f32,        // +8
    light_outer_radius: f32,        // +12
    player_light_color: vec4<f32>,  // +16
    top_color         : vec4<f32>,  // +32
    bottom_color      : vec4<f32>,  // +48
    min_y             : f32,        // +64
    max_y             : f32,        // +68
}
```

**행동 (C# 근거)**
- 수직 그라데이션: `min_y`~`max_y` 사이에서 `bottom_color` → `top_color`
- 원거리 조명: `player_pos` 기준, `light_inner_radius`~`light_outer_radius` 사이 감쇠
- 결과에 `player_light_color` 곱하고 `src_tex.a` 로 알파 보간

### 3.2 `waterBack` — 텍스처 없음

```wgsl
struct WaterBackCb {
    min_y: f32, max_y: f32,
    // color0..color9 / level0..level9 가 16바이트 간격으로 반복
}
```

행동: 수심 `y` 를 10구간으로 나눠 `color0..9` 를 선형 보간하고,
`alpha` 는 `level0..9` 로 계단식 합성한다. 텍스처를 쓰지 않으므로
WGSL 이 가장 단순하다.

### 3.3 `waterFront` — 3 텍스처

```wgsl
struct WaterFrontCb0 { vp: mat4x4<f32>, vp_inv: mat4x4<f32>, resolution_time: vec4<f32> }
struct WaterFrontCb2 { value, cgrad, octave: vec2<f32>, strength, edge,
                       godray_min_y, godray_max_y, godray_move, godray_bright: f32,
                       godray_color: vec3<f32>, godray_uv_rot: f32,
                       godray_uv_scale: vec2<f32>, aberration, dist_strength: f32,
                       diffuse: vec3<f32>, screen_min_y, screen_max_y: f32,
                       screen_color: vec3<f32> }
struct WaterFrontCb4 { player_pos: vec2<f32>, distance_factor1: f32,
                       min_y, max_y: f32, dist_center_pos: vec2<f32>,
                       distance_factor2: f32 }
```

행동 요소 (난이도순):
1. **fBm 워터웨이브** — `octave` 옥타브 수만큼 노이즈 샘플 가중 합
2. **디스토션** — `dist_strength` 로 `bg_tex` UV 왜곡
3. **색수차** — `aberration` 만큼 R/B 채널을 반대 방향으로 샘플
4. **갓레이** — `godray_*` 로 스크린 광선. `godray_noise_tex` 2차 샘플
5. **포화된 스크린** — `screen_min_y`~`screen_max_y` 구간에 `screen_color` 혼합

### 3.4 `vs_position_color_texture` — 정점

```wgsl
struct VsCb0 { vp: mat4x4<f32>, vp_inv: mat4x4<f32>, resolution_time: vec4<f32> }
struct VsCb1 { world: mat4x4<f32> }
```

출력: `position = vp * world * pos`, `color`(vec4) × `texcoord`(vec2) 그대로.

---

## 4. 렌더러 의존성 — 무엇이 사라지는가

| 제거 대상 | LOC | 사유 |
|---|---:|---|
| `Common/Rendering/EffectCompiler/` | 792 | D3DX9 효과 바이너리 → `.mgfxo` 재구성. WGSL 불필요 |
| `Common/Rendering/D2D*.cs` | 534 | Direct2D. `cosmic-text` 로 대체 |
| `Common/Rendering/MonogameUtils.cs` | ~92 | MonoGame 사私有 필드 리플렉션 |
| `Common/Rendering/Texture2DEx.cs` | ~90 | BC7/R16 DXGI 포맷 주입 |
| `MapRender/Effects/` (전체) | 648 | 메타데이터 + 23 바이너리 |
| **합계** | **~2,156** | |

---

## 5. 상태

| 항목 | 상태 |
|---|---|
| 23개 바이너리 전수 조사 | ✅ (사용 3, 미사용 20) |
| 파라미터 계약 추출 | ✅ CB 레이아웃 전부 |
| WGSL `light` | ✅ |
| WGSL `waterBack` | ✅ |
| WGSL `waterFront` | ✅ (5개 요소) |
| WGSL `vs_position_color_texture` | ✅ |
| 미사용 20개 | **번들 제외** — 재작성 불필요 |
| `known-bugs.md` #19 갱신 | ✅ |
| 시각 검증 (스크린샷 비교) | ⏳ Phase 6 — 오라클 C# 과 픽셀 비교 |
