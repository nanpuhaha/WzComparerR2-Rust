// MapleStory 커스텀 스프라이트 픽셀 셰이더 (WGSL 재작성)
//
// 원본: 게임 클라이언트에서 추출된 DXBC 23개 중 **실제로 사용되는 3개**
// (감사 근거: docs/shader-audit.md §1)
//
// # 법적 상태
//
// 이 파일들은 **원본 WGSL 구현**이다. 추출된 게임 셰이더 바이트코드는
// 포함하지 않으며, 산출물에 포함되지 않는다. 파라미터 이름·의미·버퍼
// 레이아웃만 C# `EffectResources.cs` 의 `NativeShaderDesc` 에서 재현했다
// (이 레이아웃이 WZ 맵 데이터와 연결되는 ABI 이므로 정확한 축이다).
//
// 원본과 **픽셀 단위 일치를 목표로 하지 않는다.** 시각적 근사가 목적이며
// 바이트 재사용이 아니다.

// ═══════════════════════════════════════════════════════════════════════════
// 1. light — 수직 그라데이션 + 원거리 조명 합성
// ═══════════════════════════════════════════════════════════════════════════
//
// 원본 파일명: g_module_light_ps (KMST1186)
//
// CB2 (16 B): z
// CB4 (96 B): player_pos, light_inner_radius, light_outer_radius,
//             player_light_color, top_color, bottom_color, min_y, max_y
// Sampler 0: src_tex
//
// 파라미터 바인딩 근거:
//   ShaderMaterials.cs:176-199 (LightPixelShaderMaterial)
//   EffectResources.cs:37-85  (NativeShaderDesc "light")

struct LightUniforms {
    // cb0 (slot 2) — 16 B
    z: f32,
    _pad0: f32,
    _pad1: f32,
    _pad2: f32,

    // cb1 (slot 4) — 96 B
    player_pos: vec2<f32>,          // +0
    light_inner_radius: f32,        // +8
    light_outer_radius: f32,        // +12
    player_light_color: vec4<f32>,  // +16
    top_color: vec4<f32>,           // +32
    bottom_color: vec4<f32>,        // +48
    min_y: f32,                     // +64
    max_y: f32,                     // +68
    _pad3: vec2<f32>,               // +72 (SizeInBytes=96 에 맞춘 패딩)
}

@group(1) @binding(0) var<uniform> u: LightUniforms;
@group(1) @binding(1) var src_tex: texture_2d<f32>;
@group(1) @binding(2) var src_sampler: sampler;

// inter-stage 구조체(VsIn / VsOut) 는 `common.wgsl` 에 있다. 이 파일에는
// 중복 정의하지 않는다 — Rust 쪽에서 두 모듈 소스에 공통 헤더를 문자열로
// 앞에 붙여 합성한다 (`shader.rs` 의 `assemble()`). 두 곳에 따로 적으면
// inter-stage ABI 가 조용히 어긋나고, 그건 컴파일이 아니라 **wgpu 파이프라인
// 생성 시점**에야 드러나는 런타임 고가 된다.

/// 0..1 로 클램프된 선형 보간.
fn lerp_clamped(a: f32, b: f32, t: f32) -> f32 {
    return a + (b - a) * clamp(t, 0.0, 1.0);
}

/// 내부가 진한 값, 외부가 흐린 값인 조명 감쇠.
fn light_falloff(dist: f32, inner: f32, outer: f32) -> f32 {
    if outer <= inner {
        // 반경이 없으면 감쇠 없음 (점광)
        return 1.0;
    }
    if dist <= inner {
        return 1.0;
    }
    if dist >= outer {
        return 0.0;
    }
    // 내부는 선형으로 유지하고 바깥에서 0 으로 수렴
    let t = (dist - inner) / (outer - inner);
    // 제곱으로 감쇠시켜 자연스러운 falloff 를 만든다
    let k = 1.0 - t;
    return k * k;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // 1. 소스 텍스처 (알파가 마스크 역할)
    let src = textureSample(src_tex, src_sampler, in.uv);

    // 2. 수직 그라데이션 — bottom_color → top_color
    //    min_y ~ max_y 구간을 0..1 로 매핑
    let y_range = u.max_y - u.min_y;
    let y_t = select(
        0.0,
        (in.world_y - u.min_y) / y_range,
        abs(y_range) > 1e-6,
    );
    let grad_color = mix(u.bottom_color, u.top_color, clamp(y_t, 0.0, 1.0));

    // 3. 원거리 조명 — 원본 `LightRenderer` 는 거리 기반 감쇠를 사용한다.
    //    UV 스페이스에서의 거리를 플레이어 위치 대비 계산한다.
    let delta = in.uv - u.player_pos;
    let dist = length(delta);
    let falloff = light_falloff(dist, u.light_inner_radius, u.light_outer_radius);
    let player_light = u.player_light_color * falloff;

    // 4. 합성: 텍스처 × 정점 색상 × (그라데이션 + 조명)
    let lit = grad_color + player_light;
    var rgb = src.rgb * in.color.rgb * lit.rgb;
    let alpha = src.a * in.color.a * lit.a;

    // z 는 원본의 깊이 기반 감쇠용이다. 0 이면 비활성.
    if (u.z > 0.0) {
        rgb = rgb * u.z;
    }

    return vec4<f32>(rgb, alpha);
}

// ═══════════════════════════════════════════════════════════════════════════
// 2. waterBack — 10구간 수심 그라데이션 (텍스처 없음)
// ═══════════════════════════════════════════════════════════════════════════
//
// 원본 파일명: g_module_water_back_ps (KMST1186)
//
// CB2 (176 B): min_y, max_y, color0..9 (vec3), level0..9 (f32)
// Sampler: 없음 — 이 셰이더가 가장 단순하다.
//
// 파라미터 바인딩 근거:
//   ShaderMaterials.cs:222-265 (WaterBackPixelShaderMaterial)
//   EffectResources.cs:86-127   (NativeShaderDesc "waterBack")

// vec3(12) + f32(4) = 16 바이트씩 10세트가 딱 맞아 160 바이트.
// min_y(4) + max_y(4) = 8 → 총 168, SizeInBytes=176 이므로 8바이트 패딩.
struct WaterBackUniforms {
    min_y: f32,
    max_y: f32,
    _pad0: f32,
    _pad1: f32,

    color0: vec3<f32>, level0: f32,
    color1: vec3<f32>, level1: f32,
    color2: vec3<f32>, level2: f32,
    color3: vec3<f32>, level3: f32,
    color4: vec3<f32>, level4: f32,
    color5: vec3<f32>, level5: f32,
    color6: vec3<f32>, level6: f32,
    color7: vec3<f32>, level7: f32,
    color8: vec3<f32>, level8: f32,
    color9: vec3<f32>, level9: f32,
}

@group(1) @binding(0) var<uniform> wb: WaterBackUniforms;

@fragment
fn fs_water_back(in: VsOut) -> @location(0) vec4<f32> {
    // 색 배열에 묶어서 접근 (WGSL 은 동적 인덱스를 지원한다)
    var colors: array<vec3<f32>, 10> = array<vec3<f32>, 10>(
        wb.color0, wb.color1, wb.color2, wb.color3, wb.color4,
        wb.color5, wb.color6, wb.color7, wb.color8, wb.color9,
    );
    var levels: array<f32, 10> = array<f32, 10>(
        wb.level0, wb.level1, wb.level2, wb.level3, wb.level4,
        wb.level5, wb.level6, wb.level7, wb.level8, wb.level9,
    );

    let y_range = wb.max_y - wb.min_y;
    let y_t = select(0.0, (in.world_y - wb.min_y) / y_range, abs(y_range) > 1e-6);

    // --- 알파: 계단식 (step) 합성 ---
    // level_i 는 i 번째 구간의 불투명도 기여. 누적 합으로 시야층을 만든다.
    // 맨 위(작은 y)일수록 알파가 낮아야 하므로 역순으로 누적한다.
    var alpha = 0.0;
    for (var i = 0u; i < 10u; i = i + 1u) {
        let bound = (f32(i) + 1.0) / 10.0;
        if (y_t >= bound) {
            alpha = alpha + levels[i];
        }
    }
    alpha = clamp(alpha, 0.0, 1.0);

    // --- 색: 인접 두 구간 사이 선형 보간 ---
    let scaled = clamp(y_t, 0.0, 0.9999) * 10.0;
    let idx = u32(floor(scaled));
    let next = min(idx + 1u, 9u);
    let frac = fract(scaled);
    let rgb = mix(colors[idx], colors[next], frac);

    return vec4<f32>(rgb, alpha);
}

// ═══════════════════════════════════════════════════════════════════════════
// 3. waterFront — 파동 + 디스토션 + 색수차 + 갓레이 + 포화 스크린
// ═══════════════════════════════════════════════════════════════════════════
//
// 원본 파일명: g_module_water_front_ps (KMST1186)
//
// CB0 (144 B): vp, vp_inv, resolution_time
// CB2 (112 B): value, cgrad, octave, strength, edge, godray_min_y,
//              godray_max_y, godray_move, godray_bright, godray_color,
//              godray_uv_rot, godray_uv_scale, aberration, dist_strength,
//              diffuse, screen_min_y, screen_max_y, screen_color
// CB4 ( 96 B): player_pos, distance_factor1, min_y, max_y,
//              dist_center_pos, distance_factor2
// Sampler 1: noise_tex, 2: godray_noise_tex, 3: bg_tex
//
// 파라미터 바인딩 근거:
//   ShaderMaterials.cs:284-349 (WaterFrontPixelShaderMaterial)
//   EffectResources.cs:128-219  (NativeShaderDesc "waterFront")

struct WaterFrontUniforms {
    // cb0 (slot 0) — 144 B
    vp: mat4x4<f32>,
    vp_inv: mat4x4<f32>,
    resolution_time: vec4<f32>,

    // cb1 (slot 2) — 112 B
    value: vec2<f32>,
    cgrad: vec2<f32>,
    octave: vec2<f32>,
    strength: f32,
    edge: f32,
    godray_min_y: f32,
    godray_max_y: f32,
    godray_move: f32,
    godray_bright: f32,
    godray_color: vec3<f32>,
    godray_uv_rot: f32,
    godray_uv_scale: vec2<f32>,
    aberration: f32,
    dist_strength: f32,
    diffuse: vec3<f32>,
    screen_min_y: f32,
    screen_max_y: f32,
    screen_color: vec3<f32>,
    _pad0: f32,
    _pad1: f32,

    // cb2 (slot 4) — 96 B
    player_pos: vec2<f32>,
    distance_factor1: f32,
    _pad2: f32,
    min_y: f32,
    max_y: f32,
    dist_center_pos: vec2<f32>,
    distance_factor2: f32,
    _pad3: f32,
    _pad4: vec2<f32>,
}

@group(1) @binding(0) var<uniform> wf: WaterFrontUniforms;
@group(1) @binding(1) var noise_tex: texture_2d<f32>;
@group(1) @binding(2) var godray_noise_tex: texture_2d<f32>;
@group(1) @binding(3) var bg_tex: texture_2d<f32>;
@group(1) @binding(4) var noise_sampler: sampler;
@group(1) @binding(5) var godray_sampler: sampler;
@group(1) @binding(6) var bg_sampler: sampler;

/// 2D 회전 (갓레이 UV 회전용).
fn rot2(p: vec2<f32>, a: f32) -> vec2<f32> {
    let c = cos(a);
    let s = sin(a);
    return vec2<f32>(p.x * c - p.y * s, p.x * s + p.y * c);
}

/// 단일 옥타브 노이즈. 노이즈 텍스처의 한 채널을 샘플한다.
fn noise_octave(uv: vec2<f32>, t: f32) -> f32 {
    return textureSample(noise_tex, noise_sampler, uv + vec2<f32>(t, t * 0.5)).r;
}

/// fBm — `octave.x` 개만큼 샘플을 가중 합한다.
fn fbm(uv: vec2<f32>, octaves: u32, t: f32) -> f32 {
    var sum = 0.0;
    var amp = 0.5;
    var freq = 1.0;
    // 옥타브 수는 WZ 데이터가 정하므로 상한을 건다 (파편 셰이더 DoS 방지)
    let n = min(octaves, 8u);
    for (var i = 0u; i < n; i = i + 1u) {
        sum = sum + noise_octave(uv * freq, t) * amp;
        amp = amp * 0.5;
        freq = freq * 2.0;
    }
    return sum;
}

@fragment
fn fs_water_front(in: VsOut) -> @location(0) vec4<f32> {
    let t = wf.resolution_time.w;      // 시간
    let res = wf.resolution_time.xy;    // 해상도
    let aspect = res.x / max(res.y, 1.0);

    var uv = in.uv;

    // ── 1. 디스토션 — 플레이어 중심으로 UV 를 왜곡 ──
    // 거리가 멀수록 약하게 (distance_factor1/2 로 감쇠 곡선 제어)
    let to_center = uv - wf.player_pos;
    let dist = length(to_center);
    let d1 = 1.0 / (1.0 + dist * wf.distance_factor1);
    let d2 = 1.0 / (1.0 + dist * wf.distance_factor2);
    let dmask = d1 * d2;
    // dist_center_pos 주변에 왜곡 중심이 이동
    let wave_n = fbm(uv * wf.value, u32(wf.octave.x), t);
    let disp = (wave_n - 0.5) * wf.dist_strength * dmask;
    uv = uv + to_center * disp * 2.0;

    // ── 2. 색수차 — R/B 채널을 반대 방향으로 샘플 ──
    let ab = wf.aberration * 0.002;
    let dir = normalize(to_center + vec2<f32>(1e-6, 0.0));
    let bg_r = textureSample(bg_tex, bg_sampler, uv + dir * ab).r;
    let bg_g = textureSample(bg_tex, bg_sampler, uv).g;
    let bg_b = textureSample(bg_tex, bg_sampler, uv - dir * ab).b;
    var color = vec3<f32>(bg_r, bg_g, bg_b);

    // ── 3. 파동 디퓨즈 ──
    // 세로 그라데이션(cgrad) 위에 fBm 을 얹고 strength 로 조절
    let y_t = select(0.0, (in.world_y - wf.min_y) / max(wf.max_y - wf.min_y, 1e-6), abs(wf.max_y - wf.min_y) > 1e-6);
    let depth_fade = 1.0 - y_t;
    let wave = (wave_n - 0.5) * wf.strength * (wf.cgrad.x + y_t * wf.cgrad.y);
    color = color + wf.diffuse * wave;
    color = color + wf.diffuse * (1.0 - depth_fade) * 0.15;

    // ── 4. 갓레이 (태양 광선) ──
    // godray_min_y~max_y 구간에서만 나타나며, 위로 흐르고(uv_rot) 위로 스크롤(move)된다
    let gy_range = wf.godray_max_y - wf.godray_min_y;
    let gy_t = select(0.0, (in.world_y - wf.godray_min_y) / gy_range, abs(gy_range) > 1e-6);
    if (gy_t > 0.0 && gy_t < 1.0) {
        var guv = (uv - vec2<f32>(0.5, 0.5)) * wf.godray_uv_scale;
        guv = rot2(guv, wf.godray_uv_rot);
        guv = guv + vec2<f32>(0.0, -t * wf.godray_move);
        let ray = textureSample(godray_noise_tex, godray_sampler, guv).r;
        // 가장자리에서 감쇠 (edge)
        let edge_fade = 1.0 - abs(gy_t * 2.0 - 1.0);
        let ray_strength = ray * edge_fade * wf.godray_bright
            * smoothstep(0.0, 0.2, gy_t) * (1.0 - smoothstep(0.8, 1.0, gy_t));
        color = color + wf.godray_color * ray_strength;
    }

    // ── 5. 포화 스크린 (수면 반사광) ──
    let sy_range = wf.screen_max_y - wf.screen_min_y;
    let sy_t = select(0.0, (in.world_y - wf.screen_min_y) / sy_range, abs(sy_range) > 1e-6);
    let screen = smoothstep(0.0, 1.0, clamp(sy_t, 0.0, 1.0));
    color = mix(color, wf.screen_color, screen * 0.5);

    return vec4<f32>(clamp(color, vec3<f32>(0.0), vec3<f32>(1.0)), 1.0);
}
