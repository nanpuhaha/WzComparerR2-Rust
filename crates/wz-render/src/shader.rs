//! WGSL 셰이더.
//!
//! # DXBC 를 쓰지 않는 이유
//!
//! 원본 C# 은 게임 클라이언트에서 추출한 DXBC 바이트코드 23개를 번들에 넣고,
//! `Rendering/EffectCompiler/`(792 LOC) 로 D3DX9 효과 객체를 위조해 MonoGame
//! 에 주입했다. 감사 결과 그중 **실제로 사용되는 것은 3개**뿐이다
//! (`docs/shader-audit.md` §1).
//!
//! 여기서는:
//!
//! 1. 추출 바이트코드를 **번호들에 넣지 않는다** (법적 노출 23 → 0)
//! 2. WZ 맵 데이터와 연결되는 **CB 레이아웃만** 재현한다
//!    (이게 실제 ABI 이므로 틀리면 화면이 전부 깨진다)
//! 3. 동작은 **원본 WGSL** 로 구현한다. 픽셀 단위 일치를 목표로 하지 않는다.
//!
//! 원본 대비 사라지는 것: `EffectCompiler` 792 + `D2D*` 534 +
//! `MonogameUtils` 92 + `Texture2DEx` 90 + `MapRender/Effects` 648 = **약 2,156 LOC**

use std::collections::HashMap;

/// WGSL 소스 (빌드 시점에 `include_str!` 로 임베드).
pub mod source {
    /// 맵 커스텀 스프라이트 픽셀 셰이더 (light / waterBack / waterFront).
    pub const MAPLESTORY: &str = include_str!("shader/maplestory.wgsl");
    /// 스프라이트 정점 셰이더.
    pub const SPRITE: &str = include_str!("shader/sprite.wgsl");
    /// 정점↔픽셀 inter-stage ABI. 두 모듈 앞에 합성된다.
    pub const COMMON: &str = include_str!("shader/common.wgsl");
}

/// 정점/픽셀 inter-stage 구조체를 공통 헤더로 앞에 붙여 **단일 소스**로 만든다.
///
/// # 왜 합성하는가
///
/// WGSL 에 include 가 없다. 정점 셰이더와 픽셀 셰이더를 두 파일로 나누면
/// `VsIn` / `VsOut` 을 두 곳에 손으로 적어야 하고, 한쪽만 고치면
/// `@location` 이 어긋난다. 그 결과는 WGSL 검증(naga)을 통과한 채
/// **wgpu 파이프라인 생성 시점**에 터지거나 — 더 나쁘게는 드라이버가
/// 조용히 잘못된 레이아웃으로 그린다.
///
/// 헤더를 앞에 붙이면 두 모듈이 같은 문자열을 공유하므로, 어긋날 수 없다.
/// 파이프라인은 이 합성본에서만 만든다.
///
/// # 반환 형태
///
/// 반환 문자열의 앞부분은 사람이 읽는 주석이고, 뒤에 모듈 본문이 온다.
/// 진입점 이름은 그대로 유지되므로 [`SpriteVertexShader::entry_point`] /
/// [`MaplePixelShader::entry_point`] 는 변하지 않는다.
pub fn assemble(which: WgslModule) -> String {
    let body = match which {
        WgslModule::Sprite => source::SPRITE,
        WgslModule::MaplePixel => source::MAPLESTORY,
    };
    // 공통 헤더를 무조건 앞에 붙인다. WGSL 은 선언 순서에 무관하지만
    // (1) 사람이 diff 를 볼 때 정점이 먼저 보이고, (2) `VsOut` 이
    // 픽셀 진입점 시그니처보다 먼저 선언되어 읽기가 자연스럽다.
    format!("{}\n\n{}", source::COMMON, body)
}

/// 합성 대상 모듈.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WgslModule {
    /// 정점 단계 (`sprite.wgsl`) — `vs_main`.
    Sprite,
    /// 픽셀 단계 (`maplestory.wgsl`) — 3개 진입점.
    MaplePixel,
}

/// inter-stage varying 하나: `location` 번호와 타입 설명.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct InterStageVarying {
    pub location: u32,
    /// 사람이 읽는 타입 설명 (테스트 실패 메시지·리포트용).
    pub desc: String,
}

/// 진입점의 inter-stage 레이아웃을 naga IR 에서 직접 뽑는다.
///
/// `stage` 가 [`naga::ShaderStage::Vertex`] 면 **출력**, [`Fragment`] 면
/// **입력** 레이아웃을 반환한다. 반환 벡터는 `location` 오름차순.
///
/// # 왜 눈으로 못 믿는가
///
/// 정점 셰이더와 픽셀 셰이더를 두 파일로 나누면 `VsOut` 을 두 곳에 적어야 한다.
/// 한쪽만 고쳐도 **WGSL 검증(naga)은 통과한다** — 각 모듈이 개별적으로는 멀쩡하기
/// 때문이다. 깨지는 지점은 파이프라인 생성 순간이고, 메시지도 "화면이 안 나온다"에
/// 가깝다. 그래서 기계가 대조한다.
pub fn inter_stage_layout(
    src: &str,
    entry_name: &str,
    stage: naga::ShaderStage,
) -> Result<Vec<InterStageVarying>, String> {
    let module = naga::front::wgsl::parse_str(src).map_err(|e| format!("WGSL 파싱 실패: {e:?}"))?;

    let func = module
        .entry_points
        .iter()
        .find(|e| e.name == entry_name)
        .ok_or_else(|| {
            let names: Vec<_> = module
                .entry_points
                .iter()
                .map(|e| e.name.as_str())
                .collect();
            format!("진입점 `{entry_name}` 없음 (있는 것: {names:?})")
        })?;
    if func.stage != stage {
        return Err(format!(
            "`{entry_name}` 의 단계가 {:?} 인데 {:?} 를 찾고 있었다",
            func.stage, stage
        ));
    }

    // 구조체 인/출력의 멤버들을 location 순으로 모은다.
    // 구조체가 아닌 단일 스칼라/벡터 입출력은 location 이 하나이므로 그것도 지원.
    let mut out = Vec::new();
    let mut collect = |ty_handle: naga::Handle<naga::Type>, binding: Option<&naga::Binding>| {
        let ty = &module.types[ty_handle];
        match ty.inner {
            naga::TypeInner::Struct { ref members, .. } => {
                for (idx, m) in members.iter().enumerate() {
                    let desc = type_desc(&module, m.ty);
                    match &m.binding {
                        Some(naga::Binding::Location { location, .. }) => {
                            out.push(InterStageVarying {
                                location: *location,
                                desc: format!("{idx}:{desc}"),
                            });
                        }
                        Some(naga::Binding::BuiltIn(naga::BuiltIn::Position { .. })) => {}
                        Some(other) => out.push(InterStageVarying {
                            // built-in 이 아닌 다른 속성은 스파이크에서 쓰지 않는다.
                            // 넘기면 이후에야 발견되는 예외가 되므로 여기서 드러낸다.
                            location: u32::MAX,
                            desc: format!("{idx}: 미지원 바인딩 {other:?}"),
                        }),
                        None => out.push(InterStageVarying {
                            location: u32::MAX,
                            desc: format!("{idx}: 바인딩 없는 멤버"),
                        }),
                    }
                }
            }
            _ => {
                // 구조체가 아닌 경우 — location 이 없으므로 오류로 취급.
                let desc = type_desc(&module, ty_handle);
                let _ = binding;
                out.push(InterStageVarying {
                    location: u32::MAX,
                    desc: format!("inter-stage 입출력이 구조체가 아님: {desc}"),
                });
            }
        }
    };

    match stage {
        naga::ShaderStage::Vertex => {
            let r = func
                .function
                .result
                .as_ref()
                .ok_or_else(|| format!("정점 진입점 `{entry_name}` 에 반환값이 없다"))?;
            collect(r.ty, r.binding.as_ref());
        }
        naga::ShaderStage::Fragment => {
            // 픽셀 진입점의 inter-stage 입력은 **인자**다.
            let a = func.function.arguments.first().ok_or_else(|| {
                format!("픽셀 진입점 `{entry_name}` 에 인자가 없다 (inter-stage 입력 필수)")
            })?;
            collect(a.ty, a.binding.as_ref());
        }
        other => return Err(format!("지원하지 않는 단계: {other:?}")),
    }

    out.sort_by_key(|v| v.location);
    Ok(out)
}

/// naga 타입 핸들을 사람이 읽는 문자열로 (테스트 메시지용).
fn type_desc(module: &naga::Module, ty: naga::Handle<naga::Type>) -> String {
    use naga::TypeInner as Ti;
    match &module.types[ty].inner {
        Ti::Scalar(s) => format!("{kind:?}", kind = s.kind),
        Ti::Vector { size, scalar } => format!("vec{}<{kind:?}>", *size as u32, kind = scalar.kind),
        Ti::Matrix {
            columns,
            rows,
            scalar,
        } => format!(
            "mat{}x{}<{kind:?}>",
            *columns as u32,
            *rows as u32,
            kind = scalar.kind
        ),
        Ti::CooperativeMatrix { columns, rows, .. } => {
            format!("coop_mat{}x{}", *columns as u32, *rows as u32)
        }
        Ti::Atomic(s) => format!("atomic<{kind:?}>", kind = s.kind),
        Ti::Pointer { base, .. } => format!("ptr<{}>", type_desc(module, *base)),
        Ti::ValuePointer { size, scalar, .. } => {
            let n = match size {
                Some(naga::VectorSize::Bi) => 2,
                Some(naga::VectorSize::Tri) => 3,
                Some(naga::VectorSize::Quad) => 4,
                _ => 1,
            };
            format!("value_ptr<vec{n}<{kind:?}>>", kind = scalar.kind)
        }
        Ti::Struct { ref members, .. } => {
            let names: Vec<_> = members.iter().map(|m| type_desc(module, m.ty)).collect();
            format!("struct{{{}}}", names.join(", "))
        }
        Ti::Array { base, size, .. } => {
            let n = match size {
                naga::ArraySize::Constant(n) => n.get().to_string(),
                naga::ArraySize::Dynamic => "dyn".into(),
                naga::ArraySize::Pending(_) => "pending".into(),
            };
            format!("array<{},{n}>", type_desc(module, *base))
        }
        Ti::Image { .. } => "image".into(),
        Ti::Sampler { .. } => "sampler".into(),
        Ti::AccelerationStructure { .. } => "acceleration_structure".into(),
        Ti::RayQuery { .. } => "ray_query".into(),
        Ti::BindingArray { base, size } => {
            let n = match size {
                naga::ArraySize::Constant(n) => n.get().to_string(),
                naga::ArraySize::Dynamic => "dyn".into(),
                naga::ArraySize::Pending(_) => "pending".into(),
            };
            format!("binding_array<{},{n}>", type_desc(module, *base))
        }
    }
}

/// 맵 픽셀 셰이더들이 요구하는 inter-stage 입력 (검증 메타데이터).
///
/// `light` / `waterBack` / `waterFront` 전부 `VsOut` 를 그대로 받는다.
/// `color`(location 0)는 정점 출력에만 있고 픽셀 입력에는 없다 — wgpu 규칙상
/// 픽셀 입력은 정점 출력의 **부분집합**이어야 하므로 이것은 정상이다.
///
/// 실제 대조는 `tests::inter_stage_layout_matches_between_vertex_and_pixel` 과
/// `tests::all_pixel_shaders_use_the_same_inter_stage_input` 가 IR 에서 한다.
/// 이 상수는 그 두 테스트가 무엇을 기대하는지 문서화한다.
pub const PIXEL_REQUIRED_LOCATIONS: &[(&str, u32, &str)] = &[
    ("fs_main", 1, "2:vec2<Float>"),
    ("fs_water_back", 1, "2:vec2<Float>"),
    ("fs_water_front", 1, "2:vec2<Float>"),
];

/// 맵 스프라이트 픽셀 셰이더의 진입점.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MaplePixelShader {
    /// `light` — 수직 그라데이션 + 원거리 조명
    Light,
    /// `waterBack` — 10구간 수심 그라데이션 (텍스처 없음)
    WaterBack,
    /// `waterFront` — 파동 + 디스토션 + 색수차 + 갓레이 + 포화 스크린
    WaterFront,
}

impl MaplePixelShader {
    /// WZ 맵의 셰이더 ID 문자열 → 열거형.
    ///
    /// 원본 `ShaderMaterialFactory.Create` 가 지원하던 3개와 정확히 일치한다
    /// (`ShaderMaterials.cs:18-27`). 그 외 ID 는 원본에서 `throw` 였고
    /// 여기도 `None` 을 돌려준다.
    pub fn from_wz_id(id: &str) -> Option<Self> {
        Some(match id {
            "light" => Self::Light,
            "waterBack" => Self::WaterBack,
            "waterFront" => Self::WaterFront,
            _ => return None,
        })
    }

    pub fn wz_id(&self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::WaterBack => "waterBack",
            Self::WaterFront => "waterFront",
        }
    }

    /// WGSL 진입점 이름.
    pub fn entry_point(&self) -> &'static str {
        match self {
            Self::Light => "fs_main",
            Self::WaterBack => "fs_water_back",
            Self::WaterFront => "fs_water_front",
        }
    }

    /// 픽셀 셰이더가 필요로 하는 바인딩 그룹 크기.
    pub fn binding_count(&self) -> usize {
        match self {
            // uniform + src_tex + sampler = 3
            Self::Light => 3,
            // uniform = 1
            Self::WaterBack => 1,
            // uniform + 3 텍스처 + 3 샘플러 = 7
            Self::WaterFront => 7,
        }
    }
}

/// 스프라이트 정점/픽셀 파이프라인 조합.
///
/// wgpu 는 하나의 `ShaderModule` 에 정점·픽셀 진입점을 둘 다 둘 수 있다.
/// 원본은 MonoGame `Effect` 로 둘을 함께 로드했다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpritePipelineDesc {
    pub vertex_shader: SpriteVertexShader,
    pub pixel_shader: MaplePixelShader,
}

/// 정점 셰이더 종류.
///
/// 원본이 실제 사용하는 정점 셰이더는 **하나**뿐이다
/// (`MsSpriteRenderer.cs:25`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SpriteVertexShader {
    /// `vs_position_color_texture` (원본 `off_42b030`)
    PositionColorTexture,
}

impl SpriteVertexShader {
    pub fn entry_point(&self) -> &'static str {
        "vs_main"
    }
    pub fn wz_id(&self) -> &'static str {
        "vs_position_color_texture"
    }
}

/// 파이프라인 조합 전수. 3개.
pub fn all_pipelines() -> Vec<SpritePipelineDesc> {
    let mut v = Vec::new();
    for p in [
        MaplePixelShader::Light,
        MaplePixelShader::WaterBack,
        MaplePixelShader::WaterFront,
    ] {
        v.push(SpritePipelineDesc {
            vertex_shader: SpriteVertexShader::PositionColorTexture,
            pixel_shader: p,
        });
    }
    v
}

/// WZ 맵 데이터의 셰이더 ID → 파이프라인 맵을 만든다.
pub fn pipeline_table() -> HashMap<&'static str, SpritePipelineDesc> {
    all_pipelines()
        .into_iter()
        .map(|d| (d.pixel_shader.wz_id(), d))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_three_shaders_are_supported() {
        // 원본 `ShaderMaterialFactory.Create` 와 정확히 일치해야 한다.
        // DXBC 23개 중 실제로 쓰이는 것이 3개였다는 감사 결과의 코드화.
        assert_eq!(
            MaplePixelShader::from_wz_id("light"),
            Some(MaplePixelShader::Light)
        );
        assert_eq!(
            MaplePixelShader::from_wz_id("waterBack"),
            Some(MaplePixelShader::WaterBack)
        );
        assert_eq!(
            MaplePixelShader::from_wz_id("waterFront"),
            Some(MaplePixelShader::WaterFront)
        );
    }

    #[test]
    fn unused_dxbc_shaders_are_rejected() {
        // 감사에서 "미사용"으로 판정된 20개. 번들에 포함하지 않으므로
        // 파이프라인도 없어야 한다. 하나라도 받아들이면 DXBC 를 되살리는 꼬리.
        for id in [
            "alpha",
            "alphaMap",
            "alphaMaps",
            "blurRadial",
            "blurX",
            "blurY",
            "colorModulate",
            "default",
            "glitch",
            "grayScale",
            "GrayTintWorldMask",
            "lens",
            "pointLight",
            "refract",
            "rope",
            "textureBinaryMask",
            "textureMask",
            "wave",
        ] {
            assert_eq!(
                MaplePixelShader::from_wz_id(id),
                None,
                "{id} 는 미사용 DXBC — 받아들이면 안 된다"
            );
        }
    }

    #[test]
    fn wz_id_roundtrip() {
        for s in [
            MaplePixelShader::Light,
            MaplePixelShader::WaterBack,
            MaplePixelShader::WaterFront,
        ] {
            assert_eq!(MaplePixelShader::from_wz_id(s.wz_id()), Some(s));
        }
    }

    #[test]
    fn entry_points_are_distinct() {
        let eps: std::collections::HashSet<_> = [
            MaplePixelShader::Light,
            MaplePixelShader::WaterBack,
            MaplePixelShader::WaterFront,
        ]
        .iter()
        .map(|s| s.entry_point())
        .collect();
        assert_eq!(eps.len(), 3, "진입점 이름이 겹치면 안 된다");
    }

    #[test]
    fn binding_counts_are_within_declared_bindings() {
        // WGSL 에서 실제로 선언한 @group(1) @binding 개수와 모순 없어야 한다.
        // (픽셀 재질 리소스는 group(1) — `common.wgsl` 의 그룹 규약 참조)
        let wgsl = source::MAPLESTORY;
        let declared = wgsl
            .lines()
            .filter(|l| l.trim_start().starts_with("@group(1) @binding("))
            .count();
        for s in [
            MaplePixelShader::Light,
            MaplePixelShader::WaterBack,
            MaplePixelShader::WaterFront,
        ] {
            assert!(
                s.binding_count() <= declared,
                "{:?} 의 바인딩 수 {} 가 선언 수 {declared} 를 초과",
                s,
                s.binding_count()
            );
            assert!(s.binding_count() > 0, "{:?} 는 바인딩이 필요하다", s);
        }
    }

    #[test]
    fn binding_groups_do_not_collide_across_stages() {
        // ★ 이 테스트가 실제로 막은 두 번째 버그다.
        //
        // 원본 C# 는 D3D9 레지스터 슬롯을 쓰므로 `light`(cb2+cb4)와
        // `waterFront`(cb0+cb2)는 충돌하지 않는다 — 한 번에 하나만 바인딩되기
        // 때문이다. wgpu 로 옮기면서 둘 다 `@group(0) @binding(0)` 에 재질 CB 를
        // 놓으면 정점 프레임 CB 와 **같은 (group, binding) 을 다른 타입으로**
        // 선언하게 되고, 파이프라인 생성이 실패한다:
        //
        //   Shader global ResourceBinding { group: 0, binding: 0 } is not
        //   available in the pipeline layout
        //
        // 규약: 정점 = group 0, 픽셀 = group 1. 이 테스트가 강제한다.
        let sprite = assemble(WgslModule::Sprite);
        let maple = assemble(WgslModule::MaplePixel);

        let vs = collect_group(&sprite, 0);
        let fs = collect_group(&maple, 1);

        assert_eq!(
            vs.len(),
            1,
            "정점 group(0) 은 프레임 CB 하나여야 한다: {vs:?}"
        );
        assert_eq!(vs[0].0, 0, "정점 프레임 CB 는 binding 0 이어야 한다");

        // 픽셀은 group(1) 에만 있어야 한다 — group(0) 을 쓰면 정점과 충돌한다.
        let fs_on_0 = collect_group(&maple, 0);
        assert!(
            fs_on_0.is_empty(),
            "픽셀 셰이더가 group(0) 을 사용한다 — 정점 프레임 CB 와 충돌한다: {fs_on_0:?}"
        );

        // 그리고 group(1) 의 바인딩 번호 집합이 `waterFront`(가장 많이 필요) 의
        // 7개(0..=6) 를 정확히 덮어야 한다.
        //
        // 주의: `light`(u) / `waterBack`(wb) / `waterFront`(wf) 가 **모두**
        // binding 0 을 쓰는 것은 의도된 설계다. 원본 C# Effect 와 마찬가지로
        // 한 번에 하나의 픽셀 셰이더만 바인딩하므로 충돌하지 않는다. 따라서
        // 셰이더 "사이"의 중복은 허용하고, **셰이더 "안에서"** 유일한지만 본다.
        let got: Vec<u32> = fs.iter().map(|(b, _)| *b).collect();
        let max = got.iter().copied().max().unwrap_or(0);
        assert_eq!(
            max, 6,
            "group(1) 최고 binding 이 6 이어야 한다 (waterFront = uniform+3tex+3sampler)"
        );
        for s in [
            MaplePixelShader::Light,
            MaplePixelShader::WaterBack,
            MaplePixelShader::WaterFront,
        ] {
            // 각 셰이더는 binding 0..N-1 을 빠짐없이, 중복 없이 쓰는다고
            // 가정한다 (WGSL 선언이 연속 블록이므로 성립).
            let n = s.binding_count();
            for i in 0..n {
                let i = i as u32;
                assert!(
                    got.contains(&i),
                    "{:?} 가 binding {i} 를 쓰는데 선언이 없다",
                    s
                );
            }
        }
    }

    /// 모듈에서 특정 `@group(N)` 의 리소스 (binding 번호, 이름) 목록.
    fn collect_group(src: &str, want: u32) -> Vec<(u32, String)> {
        let module = naga::front::wgsl::parse_str(src).expect("파싱 실패");
        let mut out = Vec::new();
        for (_handle, gv) in module.global_variables.iter() {
            if let Some(naga::ResourceBinding { group, binding }) = gv.binding {
                if group == want {
                    out.push((binding, gv.name.clone().unwrap_or_default()));
                }
            }
        }
        out.sort_by_key(|(b, _)| *b);
        out
    }

    #[test]
    fn pipeline_table_is_complete() {
        let t = pipeline_table();
        assert_eq!(t.len(), 3);
        for s in [
            MaplePixelShader::Light,
            MaplePixelShader::WaterBack,
            MaplePixelShader::WaterFront,
        ] {
            let d = t.get(s.wz_id()).expect("파이프라인 누락");
            assert_eq!(d.pixel_shader, s);
        }
    }

    // ── WGSL 검증 ─────────────────────────────────────────────────────────
    // 셰이더는 **컴파일**되어야 한다. 문법 오류가 있으면 GPU 드라이버가
    // 런타임에 죽이거나 조용히 깨진다. naga 로 빌드 타임에 잡는다.
    //
    // 주의: 아래 검증은 **합성본**(공통 헤더 포함) 대상이다. 원본 소스를
    // 그대로 파하면 `VsIn` / `VsOut` 이 정의되지 않아 실패한다.

    fn parse_and_validate(src: &str, label: &str) -> naga::Module {
        let module = naga::front::wgsl::parse_str(src)
            .unwrap_or_else(|e| panic!("{label} 파싱 실패: {e:?}"));
        let mut validator = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        );
        validator
            .validate(&module)
            .unwrap_or_else(|e| panic!("{label} 검증 실패: {e:?}"));
        module
    }

    #[test]
    fn maplestory_wgsl_compiles() {
        parse_and_validate(&assemble(WgslModule::MaplePixel), "maplestory.wgsl");
    }

    #[test]
    fn sprite_wgsl_compiles() {
        parse_and_validate(&assemble(WgslModule::Sprite), "sprite.wgsl");
    }

    #[test]
    fn common_header_defines_the_inter_stage_abi() {
        // 두 모듈에 VsIn / VsOut 을 중복 정의하지 않는다. 공통 헤더만 정의한다.
        // 중복 정의하면 naga 가 "duplicate definition" 으로 실패한다.
        for (label, body) in [
            ("sprite.wgsl", source::SPRITE),
            ("maplestory.wgsl", source::MAPLESTORY),
        ] {
            for ty in ["struct VsIn", "struct VsOut"] {
                assert!(
                    !body.contains(ty),
                    "{label} 이 `{ty}` 을 직접 정의한다 — common.wgsl 이 유일한 출처여야 한다"
                );
            }
        }
        assert!(source::COMMON.contains("struct VsIn"));
        assert!(source::COMMON.contains("struct VsOut"));
    }

    #[test]
    fn inter_stage_layout_matches_between_vertex_and_pixel() {
        // ★ 이 테스트가 실제로 막은 버그다.
        //
        // 이전에는 sprite.wgsl 의 `@location(0) color` 와 maplestory.wgsl 의
        // `@location(0) uv` 가 같은 위치 번호를 다른 타입으로 쓰고 있었다.
        // 각 모듈은 개별적으로는 naga 검증을 통과하므로, 이 불일치는
        // 파이프라인 생성 전까지 아무도 몰랐다.
        let sprite = assemble(WgslModule::Sprite);
        let maple = assemble(WgslModule::MaplePixel);

        let vs_out = inter_stage_layout(&sprite, "vs_main", naga::ShaderStage::Vertex)
            .expect("정점 inter-stage 레이아웃 추출 실패");
        let fs_in = inter_stage_layout(&maple, "fs_main", naga::ShaderStage::Fragment)
            .expect("픽셀 inter-stage 레이아웃 추출 실패");

        // location 0 = 정점 색(또는 색수차 R 채널), 1 = uv, 2 = world_y
        fn by_loc(v: &[InterStageVarying], loc: u32) -> &InterStageVarying {
            v.iter()
                .find(|x| x.location == loc)
                .unwrap_or_else(|| panic!("location {loc} 없음 (전체: {v:?})"))
        }

        let uv_vs = by_loc(&vs_out, 1);
        let uv_fs = by_loc(&fs_in, 1);
        assert_eq!(
            uv_vs.desc, uv_fs.desc,
            "location 1 의 타입이 정점/픽셀에서 다르다 — inter-stage 불일치"
        );

        let wy_vs = by_loc(&vs_out, 2);
        let wy_fs = by_loc(&fs_in, 2);
        assert_eq!(
            wy_vs.desc, wy_fs.desc,
            "location 2(world_y)의 타입이 정점/픽셀에서 다르다"
        );

        // location 0 은 픽셀 입력에 없어도 된다 (부분집합 규칙).
        // 하지만 정점 출력에는 반드시 있어야 한다.
        assert!(
            vs_out.iter().any(|v| v.location == 0),
            "정점 출력에 location 0(color)이 없다"
        );
    }

    #[test]
    fn all_pixel_shaders_use_the_same_inter_stage_input() {
        // 3개 픽셀 진입점 전부가 동일한 inter-stage 입력을 요구해야 한다.
        // 하나라도 다르면 그 셰이더만 파이프라인 생성 시점에 실패한다.
        let maple = assemble(WgslModule::MaplePixel);
        let baseline = inter_stage_layout(&maple, "fs_main", naga::ShaderStage::Fragment)
            .expect("fs_main 레이아웃 추출 실패");

        // `ScalarKind` 의 Debug 표현은 WGSL 스칼라 이름(`Float`)이다.
        // uv = location 1(vec2<Float>), world_y = location 2(Float).
        for (entry, loc, want) in PIXEL_REQUIRED_LOCATIONS {
            let got = inter_stage_layout(&maple, entry, naga::ShaderStage::Fragment)
                .unwrap_or_else(|e| panic!("{entry} 레이아웃 추출 실패: {e}"));
            let v = got
                .iter()
                .find(|v| v.location == *loc)
                .unwrap_or_else(|| panic!("{entry} 에 location {loc} 없음"));
            assert_eq!(
                &v.desc, want,
                "{entry} 의 location {loc} 타입이 의도와 다르다"
            );
        }

        // 세 진입점의 입력 레이아웃이 완전히 같아야 한다.
        for entry in ["fs_water_back", "fs_water_front"] {
            let got = inter_stage_layout(&maple, entry, naga::ShaderStage::Fragment)
                .unwrap_or_else(|e| panic!("{entry} 레이아웃 추출 실패: {e}"));
            let sig: Vec<_> = got.iter().map(|v| (v.location, v.desc.clone())).collect();
            let base: Vec<_> = baseline
                .iter()
                .map(|v| (v.location, v.desc.clone()))
                .collect();
            assert_eq!(sig, base, "{entry} 의 inter-stage 입력이 fs_main 과 다르다");
        }
    }

    #[test]
    fn declared_entry_points_exist_in_wgsl() {
        // 진입점은 합성본에서 찾는다 (선언 위치는 공통 헤더 뒤로 밀리지만
        // 진입점 이름 자체는 그대로다).
        let vs = parse_and_validate(&assemble(WgslModule::Sprite), "sprite");
        let vs_names: Vec<&str> = vs.entry_points.iter().map(|e| e.name.as_str()).collect();
        {
            let p = SpriteVertexShader::PositionColorTexture;
            assert!(
                vs_names.contains(&p.entry_point()),
                "정점 진입점 `{}` 없음 (있는 것: {vs_names:?})",
                p.entry_point()
            );
        }

        let ps = parse_and_validate(&assemble(WgslModule::MaplePixel), "maplestory");
        let ps_names: Vec<&str> = ps.entry_points.iter().map(|e| e.name.as_str()).collect();
        for p in [
            MaplePixelShader::Light,
            MaplePixelShader::WaterBack,
            MaplePixelShader::WaterFront,
        ] {
            assert!(
                ps_names.contains(&p.entry_point()),
                "픽셀 진입점 `{}` 없음 (있는 것: {ps_names:?})",
                p.entry_point()
            );
        }
    }

    #[test]
    fn vertex_and_pixel_stages_are_present() {
        use naga::ShaderStage;
        let vs = parse_and_validate(&assemble(WgslModule::Sprite), "sprite");
        assert!(
            vs.entry_points
                .iter()
                .any(|e| e.stage == ShaderStage::Vertex),
            "정점 진입점이 없다"
        );

        let ps = parse_and_validate(&assemble(WgslModule::MaplePixel), "maplestory");
        assert!(
            ps.entry_points
                .iter()
                .any(|e| e.stage == ShaderStage::Fragment),
            "픽셀 진입점이 없다"
        );
    }

    /// 주석(`//`, `/* */`) 을 제거한 코드를 돌려준다.
    ///
    /// 감사 주석에 "DXBC" 라는 단어가 등장하는 것은 정상이다. 검사 대상은
    /// **코드/데이터** 이다 — 주석은 사람이 읽는 것이므로 걸러야 한다.
    fn strip_comments(src: &str) -> String {
        let mut out = String::with_capacity(src.len());
        let mut chars = src.chars().peekable();
        let mut in_line = false;
        let mut in_block = false;
        while let Some(c) = chars.next() {
            if in_line {
                if c == '\n' {
                    in_line = false;
                    out.push('\n');
                }
                continue;
            }
            if in_block {
                if c == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    in_block = false;
                } else if c == '\n' {
                    out.push('\n');
                }
                continue;
            }
            if c == '/' && chars.peek() == Some(&'/') {
                chars.next();
                in_line = true;
                continue;
            }
            if c == '/' && chars.peek() == Some(&'*') {
                chars.next();
                in_block = true;
                continue;
            }
            out.push(c);
        }
        out
    }

    #[test]
    fn no_extracted_binary_is_embedded() {
        // 법적 방어선 (리포트 §12 R1): 주석을 제외한 코드에
        // DXBC 컨테이너 매직 / D3DX 효과 바이너리 참조가 없어야 한다.
        for (name, src) in [
            ("maplestory.wgsl", source::MAPLESTORY),
            ("sprite.wgsl", source::SPRITE),
        ] {
            let code = strip_comments(src);
            for needle in ["DXBC", "d3dx_effect", "mgfxo", "VS_3_0", "PS_3_0"] {
                assert!(
                    !code.contains(needle),
                    "{name} 코드에 `{needle}` 가 있다 — 추출 바이너리가 섞인 듯"
                );
            }
        }
    }

    #[test]
    fn comment_stripper_works() {
        // 방어선 자체가 올바른지 확인 (헬퍼가 고장나면 검사 무의미)
        let src = "// DXBC 주석\nlet a = 1; /* DXBC 블록 */ let b = 2;";
        let code = strip_comments(src);
        assert!(!code.contains("DXBC"), "{code}");
        assert!(code.contains("let a = 1;"));
        assert!(code.contains("let b = 2;"));
    }

    #[test]
    fn shaders_contain_no_binary_blob() {
        // WGSL 은 텍스트다. 제어 문자(0x00-0x08, 0x0B, 0x0C, 0x0E-0x1F)가
        // 있으면 바이너리가 섞인 것.
        for (name, src) in [
            ("maplestory.wgsl", source::MAPLESTORY),
            ("sprite.wgsl", source::SPRITE),
        ] {
            for (i, c) in src.chars().enumerate() {
                let bad = (c as u32) < 0x20 && c != '\n' && c != '\r' && c != '\t';
                assert!(!bad, "{name} 위치 {i} 에 제어 문자 {:?}", c as u32);
            }
        }
    }

    #[test]
    fn all_shaders_have_a_source_embedded() {
        // include_str! 가 실제로 파일을 읽어왔는지 (경로 오류 조기 발견)
        assert!(
            source::MAPLESTORY.len() > 1000,
            "maplestory.wgsl 이 비어 있다"
        );
        assert!(source::SPRITE.len() > 200, "sprite.wgsl 이 비어 있다");
        assert!(source::MAPLESTORY.contains("@fragment"));
        assert!(source::SPRITE.contains("@vertex"));
    }
}
