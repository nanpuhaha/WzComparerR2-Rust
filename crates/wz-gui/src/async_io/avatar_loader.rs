// wz-gui — 비동기 I/O 및 백그라운드 파싱.
//
// `docs/gui-architecture.md` §5: UI 스레드는 블로킹 I/O 를 하지 않는다.
// 워커 스레드가 결과를 채널로 보내고, UI 스레드는 `poll_load_result` 로
// 매 프레임 폴링한다.

use crate::state::AppState;

// ── 파일 열기 ─────────────────────────────────────────────────────────

pub fn load_avatar_from_loader(state: &mut AppState) {
    let Some(data_root) = crate::async_io::canvas::find_data_root(None) else {
        state.avatar_status = "WZ_DATA_ROOT 를 설정하세요 (MapleStory Data 디렉터리)".into();
        return;
    };
    let char_wz = data_root.join("Character").join("Character.wz");
    match wz_avatar::load_from_character_wz_with_skin(&char_wz, state.avatar_body_skin) {
        Some(av) => {
            state.avatar_status = format!("로드: {} 파트", av.parts.len());
            state.avatar_loaded_parts = av
                .parts
                .iter()
                .map(|p| (p.slot.clone(), p.width, p.height, p.rgba.clone()))
                .collect();
            if let Some(first) = av.parts.first() {
                state.avatar_image = Some((first.width, first.height, first.rgba.clone()));
            }
            state.effect_frames.clear();
            state.effect_delays.clear();
            state.effect_origins.clear();
            state.effect_origin = (0, 0);
            state.effect_last_time = 0.0;
            render_loaded_avatar(state);
        }
        None => {
            state.avatar_status = "아바타 로드 실패".into();
        }
    }
}

// ── StringLinker 로드 ─────────────────────────────────────────────────

pub fn load_avatar_with_equipment_input(state: &mut AppState) {
    let input = state.avatar_equipment_input.clone();
    if input.trim().is_empty() {
        state.avatar_status = "장비 ID를 입력하세요 (예: Cap:1000000,Coat:1040000)".into();
        return;
    }
    let data_root = match state.data_root.clone() {
        Some(r) => r,
        None => {
            state.avatar_status = "data_root 가 설정되지 않았습니다".into();
            return;
        }
    };
    // 파싱: "Slot:ID,Slot:ID" 형식
    let mut equipment: Vec<(String, i32)> = Vec::new();
    for part in input.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let mut split = part.split(':');
        let slot = split.next().unwrap_or("").trim().to_string();
        let id_str = split.next().unwrap_or("").trim();
        if slot.is_empty() || id_str.is_empty() {
            continue;
        }
        if let Ok(id) = id_str.parse::<i32>() {
            equipment.push((slot, id));
        }
    }
    if equipment.is_empty() {
        state.avatar_status = "유효한 장비 ID가 없습니다".into();
        return;
    }
    match wz_avatar::load_avatar_with_equipment_and_mix(
        &data_root,
        2000,
        &equipment,
        &wz_avatar::MixOptions {
            hair: state
                .hair_mix_enabled
                .then_some((state.hair_mix_color, state.hair_mix_opacity)),
            face: state
                .face_mix_enabled
                .then_some((state.face_mix_color, state.face_mix_opacity)),
            effect_frame: state.effect_frame,
            custom_origins: parse_custom_origins(&state.custom_origin_input),
        },
    ) {
        Some(av) => {
            state.avatar_status = format!("장비 로드: {} 파트", av.parts.len());
            state.avatar_loaded_parts = av
                .parts
                .iter()
                .map(|p| (p.slot.clone(), p.width, p.height, p.rgba.clone()))
                .collect();
            if let Some(first) = av.parts.first() {
                state.avatar_image = Some((first.width, first.height, first.rgba.clone()));
            }
            state.effect_frames = av
                .effect_frames
                .iter()
                .map(|p| (p.width, p.height, p.rgba.clone()))
                .collect();
            state.effect_delays = av.effect_delays.clone();
            state.effect_origins = av.effect_origins.iter().map(|p| (p.x, p.y)).collect();
            state.effect_origin = state
                .effect_origins
                .get(state.effect_frame)
                .copied()
                .unwrap_or((0, 0));
            state.effect_last_time = 0.0;
            render_loaded_avatar(state);
        }
        None => {
            state.avatar_status = "장비 로드 실패".into();
        }
    }
}

pub fn render_loaded_avatar(state: &mut AppState) {
    if state.avatar_loaded_parts.is_empty() {
        state.avatar_status = "로드된 파트가 없습니다".into();
        return;
    }
    // 프리즘은 WZ 가 아니라 UI 가 공급한다 (C# `AvatarPart.PrismData` 와 동일).
    // 원본 파트는 보존하고, 렌더용 사본에만 색 변환을 적용한다.
    let prism = wz_avatar::PrismData {
        color_type: state.prism_color_type,
        hue: state.prism_hue,
        saturation: state.prism_saturation,
        brightness: state.prism_brightness,
        convert_pure_black: state.prism_convert_pure_black,
    };
    let apply_prism = state.prism_enabled && prism.is_valid();
    let parts: Vec<(String, u32, u32, Vec<u8>)> = state
        .avatar_loaded_parts
        .iter()
        .map(|(slot, w, h, rgba)| {
            let mut rgba = rgba.clone();
            if apply_prism {
                wz_avatar::apply_prism_rgba8(&mut rgba, &prism, false);
            }
            (slot.clone(), *w, *h, rgba)
        })
        .collect();
    if let Some((_, w, h, rgba)) = parts.first() {
        state.avatar_image = Some((*w, *h, rgba.clone()));
    }
    use wz_render::{
        atlas::AtlasShelfPacker, batch::SpriteBatch, camera::Camera2D, device::RenderContext,
    };
    let mut packer = AtlasShelfPacker::new(512, 512);
    let selected: Vec<usize> = if state.avatar_selected_slots.is_empty() {
        (0..parts.len()).collect()
    } else {
        state.avatar_selected_slots.iter().copied().collect()
    };
    let mut slots = Vec::new();
    for &i in &selected {
        if let Some((slot, w, h, rgba)) = parts.get(i) {
            if let Some(s) = packer.allocate_and_write(*w, *h, rgba) {
                // effect 프레임은 origin 픽셀이 아바타 기준점(0,0)에 오도록 배치한다.
                // 나머지 파트는 기존처럼 좌상단 기준을 유지한다.
                let pos = if slot == "effect" {
                    [
                        -(state.effect_origin.0 as f32),
                        -(state.effect_origin.1 as f32),
                    ]
                } else {
                    [0.0, 0.0]
                };
                slots.push((*w, *h, s, pos));
            }
        }
    }
    let mut rc = match RenderContext::new_headless() {
        Ok(r) => r,
        Err(e) => {
            state.avatar_status = format!("RC 실패: {e}");
            return;
        }
    };
    for page in packer.pages() {
        let _ = rc.create_texture(page.key, page.width, page.height, &page.pixels);
    }
    let mut batch = SpriteBatch::new();
    for (w, h, slot, pos) in &slots {
        batch.push_atlas_quad(
            slot.key,
            [pos[0], pos[1], *w as f32, *h as f32],
            0.0,
            &slot.rect,
            (512, 512),
            [1.0, 1.0, 1.0, 1.0],
        );
    }
    batch.finish();
    let (rw, rh) = (256u32, 256u32);
    let cam = Camera2D::new(rw as f32, rh as f32);
    let uniforms = cam.build_uniforms(0.0, 0.0);
    match rc.render_to_rgba(rw, rh, &batch, &uniforms, [0.08, 0.08, 0.12, 1.0]) {
        Ok(out) => {
            state.avatar_status = format!(
                "렌더 완료: {} 파트, {}x{}{}",
                slots.len(),
                rw,
                rh,
                if apply_prism { " (prism)" } else { "" }
            );
            state.avatar_rendered = Some((rw, rh, out));
        }
        Err(e) => {
            state.avatar_status = format!("렌더 실패: {e}");
        }
    }
}

/// `name:x,y;name2:x,y` 형식의 커스텀 오리진 입력을 파싱한다.
fn parse_custom_origins(input: &str) -> std::collections::BTreeMap<String, wz_avatar::Point> {
    let mut out = std::collections::BTreeMap::new();
    for part in input.split(';') {
        let Some((name, coords)) = part.split_once(':') else {
            continue;
        };
        let mut xy = coords.split(',');
        let (Some(x), Some(y)) = (xy.next(), xy.next()) else {
            continue;
        };
        if let (Ok(x), Ok(y)) = (x.trim().parse::<i32>(), y.trim().parse::<i32>()) {
            out.insert(name.trim().to_string(), wz_avatar::Point::new(x, y));
        }
    }
    out
}

/// Effect 자동 재생 틱. 다음 프레임으로 넘어갔으면 `true`.
pub fn tick_effect(state: &mut AppState, now: f64) -> bool {
    if !state.effect_auto_play || state.effect_frames.is_empty() {
        return false;
    }
    let frame = state.effect_frame % state.effect_frames.len();
    let delay = state
        .effect_delays
        .get(frame)
        .copied()
        .unwrap_or(200)
        .max(20) as f64
        / 1000.0;
    if state.effect_last_time <= 0.0 {
        state.effect_last_time = now;
        return false;
    }
    if now - state.effect_last_time < delay {
        return false;
    }
    state.effect_last_time = now;
    state.effect_frame = (frame + 1) % state.effect_frames.len();
    apply_effect_frame(state);
    true
}

/// 현재 `effect_frame` 의 RGBA/origin 을 파트 목록에 반영한다.
///
/// 자동 재생 틱과 수동 프레임 슬라이더가 같은 경로를 쓴다.
pub fn apply_effect_frame(state: &mut AppState) {
    let Some((w, h, rgba)) = state
        .effect_frames
        .get(state.effect_frame % state.effect_frames.len().max(1))
        .cloned()
    else {
        return;
    };
    state.effect_origin = state
        .effect_origins
        .get(state.effect_frame % state.effect_origins.len().max(1))
        .copied()
        .unwrap_or((0, 0));
    if let Some(part) = state
        .avatar_loaded_parts
        .iter_mut()
        .find(|p| p.0 == "effect")
    {
        part.1 = w;
        part.2 = h;
        part.3 = rgba;
    } else {
        state
            .avatar_loaded_parts
            .push(("effect".into(), w, h, rgba));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_effect_frame_updates_origin_and_part() {
        let mut state = AppState {
            effect_frames: vec![(2, 2, vec![1; 16]), (3, 3, vec![2; 36])],
            effect_origins: vec![(5, 6), (-7, 8)],
            effect_frame: 1,
            ..Default::default()
        };

        apply_effect_frame(&mut state);

        assert_eq!(state.effect_origin, (-7, 8));
        let part = state
            .avatar_loaded_parts
            .iter()
            .find(|p| p.0 == "effect")
            .expect("effect part");
        assert_eq!((part.1, part.2), (3, 3));
        assert_eq!(part.3.len(), 36);
    }
}
