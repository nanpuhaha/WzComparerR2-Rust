# Phase 6 렌더 스파이크 — 실측 결과

> 담당: @frontend-ui-ux-engineer
> 날짜: 2026-10-06
> 도구: `cargo run -p render-spike` (헤드리스, `wz-rs/tools/render-spike/`)

## 0. 결론 먼저

**Phase 6 GUI(egui + wgpu) 착수는 정당하다.** 추측이 아니라 실측으로 확인했다.

다만 **착수 전에 고쳐야 할 결함 2개가 발견됐다.** 둘 다 `wz-render` 커밋
(`f0aee31`)에 이미 들어 있던 것이고, 문법 검증만으로는 절대 잡히지 않던 종류다.

| 항목 | 실측 결과 |
|---|---|
| 어댑터 | **AMD Radeon(TM) Graphics (IntegratedGpu Vulkan)** |
| 백엔드 | **Vulkan** (wgpu 30, 이 머신) |
| 파이프라인 생성 | ✅ **3/3** (`light` / `waterBack` / `waterFront`) |
| 실제 드로우 + GPU 되읽기 | ✅ **12,288 / 16,384 바이트** |
| 되읽기 비율 검증 | **정확히 0.75** — 삼각형 기하와 일치 |
| inter-stage 레이아웃 | 정점 3개 = 픽셀 3개, **완전 일치** |
| `cargo test --workspace` | ✅ 전부 통과 (1개 `#[ignore]` 기존) |
| `cargo clippy --workspace --all-targets -D warnings` | ✅ **0 경고** |

## 1. 왜 스파이크가 필요한가

리포트 §9.3 은 "egui + wgpu" 를 **제안**했을 뿐 실측이 없었다. 문서만 보고
6~8주 착수하면, 렌더러가 통째로 못 가는 상황을 뒤늦게 알게 된다.

스파이크의 목표는 "된다/안 된다" 가 아니라 **무엇이 깨지는지** 를 찾는 것이었다.
결과적으로 **코드 2개만 고치면 되는** 상태임을 확인했다. 그 2개가 아래 §2, §3.

## 2. 결함 ① — inter-stage ABI 불일치 (커밋에 이미 있었음)

`sprite.wgsl` 의 정점 출력과 `maplestory.wgsl` 의 픽셀 입력이
**서로 다른 레이아웃**을 선언하고 있었다.

| location | `sprite.wgsl` (정점) | `maplestory.wgsl` (픽셀, 커밋 시점) |
|---|---|---|
| 0 | `color: vec4<f32>` | `uv: vec2<f32>` ← **충돌** |
| 1 | `uv: vec2<f32>` | `world_y: f32` |
| 2 | `world_y: f32` | — |

**왜 아무도 몰랐나:** 두 모듈이 **각각은** naga 검증을 통과한다. WGSL 문법도 맞고
타입도 자기 자신 안에서는 맞다. 깨지는 지점은 **둘을 하나의 파이프라인으로 묶는 순간**이며,
그때의 에러 메시지도 "화면이 안 나온다" 에 가깝다.

**수정:** `common.wgsl` 을 만들어 `VsIn` / `VsOut` 을 **유일한 출처**로 만들고,
Rust 쪽에서 `assemble()` 로 두 소스에 앞에 붙인다. 이제 어긋날 수 없다.

```rust
pub fn assemble(which: WgslModule) -> String {
    let body = match which { WgslModule::Sprite => source::SPRITE,
                             WgslModule::MaplePixel => source::MAPLESTORY };
    format!("{}\n\n{}", source::COMMON, body)
}
```

**재발 방지 (기계 검증):** `shader::inter_stage_layout()` 이 naga IR 에서
진입점의 입출력 레이아웃을 직접 뽑는다. 사람이 `@location` 을 눈으로 대조하는
방식은 반드시 한 번은 놓친다 — 놓친 대가가 컴파일 에러가 아니므로.

- `inter_stage_layout_matches_between_vertex_and_pixel`
- `all_pixel_shaders_use_the_same_inter_stage_input`

## 3. 결함 ② — 바인딩 그룹 충돌 (설계 결함, 스파이크가 처음으로 드러냄)

wgpu 는 파이프라인 생성 시 이 에러를 냈다:

```
Shader global ResourceBinding { group: 0, binding: 0 } is not available
in the pipeline layout
  Derived bind group layout type is not consistent between stages
```

**원인 — D3D9 → wgpu Meaning의 차이:**

원본 C# 는 **레지스터 슬롯** 전역 공간을 쓴다 (`WzComparerR2.MapRender/Effects/EffectResources.cs`):

| 셰이더 | 슬롯 |
|---|---|
| `light` | cb2 + cb4 |
| `waterBack` | cb2 |
| `waterFront` | cb0 + cb2 |
| `vs_position_color_texture` | cb0 + cb1 |

전부 같은 공간의 다른 번호이고, 원본 `Effect` 는 **한 번에 하나의 픽셀 셰이더만**
바인딩하므로 충돌하지 않는다.

wgpu 는 다르다. 정점과 픽셀이 한 파이프라인에 묶이면 두 단계가 그룹 공간을 공유하고,
같은 `(group, binding)` 을 다른 타입으로 선언하면 실패한다. 그대로 옮기면
정점 프레임 CB와 픽셀 재질 CB가 둘 다 `@group(0) @binding(0)` 이 된다.

**수정 — 그룹을 역할로 분리:**

| group | 단계 | 내용 |
|---|---|---|
| 0 | 정점 | 프레임 CB (`vp`, `vp_inv`, `resolution_time`, `world`) |
| 1 | 픽셀 | 재질 CB + 텍스처 + 샘플러 (셰이더별 1~7개) |

`light`/`waterBack`/`waterFront` 가 **모두** group(1) binding 0 을 쓰는 것은 의도된
설계다 — 원본과 동일하게 한 번에 하나만 바인딩되므로 충돌하지 않는다.
이 전제를 코드에 박아 두는 테스트: `binding_groups_do_not_collide_across_stages`.

**Phase 6 설계에 대한 함의:** 재질 CB 를 셰이더마다 group 1 에서 재사용하면
**셰이더 간 전환이 바인드 그룹 교체 한 번**으로 끝난다. 배치기(`batch`) 설계에서
텍스처/블렌드 단위 batch break 와 정렬된다.

## 4. 스파이크가 추가로 잡은 것 ( Lesser )

**버텍스 스트라이드 36 → 40.** `pos vec4(16) + color vec4(16) + uv vec2(8)` = 40 바이트.
처음에 36 으로 잡았고, wgpu 가 파이프라인 생성 시점에 잡았다:

```
Vertex attribute at location 2 stride 40 exceeds the limit 36
```

`common.wgsl` 의 `VsIn` 주석에 레이아웃 표와 `array_stride = 40` 을 명시해 두었다.
**`AtlasRect` UV 나 `SpriteBatch` 버텍스 패킹을 만들 때는 이 stride 를 재사용**한다.

## 5. 남은 것 / 권고

| 항목 | 상태 | 권고 |
|---|---|---|
| `SpriteBatch` / 어틀라스 / 카메라 | 미착수 | Phase 6 착수 시. 위 stride·그룹 규약을 선행으로 사용 |
| **GUI 툴킷 선택** | 미측정 | **egui 0.36 + wgpu 0.30 조합을 실제로 띄워 볼 것.** 스파이크는 **헤드리스**였으므로 이안·IME·한글 폰트는 검증되지 않았다. WzComparerR2 는 한글 UI 라 이것이 6~8주 착수 전에 확인할 **두 번째 위험** 요소다 |
| 픽셀 단위 오라클 비교 | 미착수 | `docs/shader-audit.md` §5 의 ⏳ 항목. `@qa-test-automation-engineer` 의 오라클 하네스와 함께 |
| CI 편입 | 미착수 | @devops-sre-engineer 의 매트릭스에 스파이크 추가 권장. **GPU 없는 러너에서는 스킵(exit 0)** 하도록 설계했음 |

### 스파이크의 GPU 부재 처리

스파이크는 GPU 없으면 **실패가 아니라 스킵**(exit 0)한다. GPU 부재를 렌더러
결함으로 오인해 CI 가 거짓 음성을 내는 것을 막기 위함이다.

## 6. 재현 방법

```bash
cd .claude/worktrees/space-bunny/wz-rs
cargo run -p render-spike        # GPU 있으면 도달성 검사, 없으면 SKIP
cargo test -p wz-render           # inter-stage / 바인딩 그룹 계약 검증 (22개)
```