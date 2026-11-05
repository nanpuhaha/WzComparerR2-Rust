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
    match wz_avatar::load_avatar_with_equipment(&data_root, 2000, &equipment) {
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
        if let Some((_slot, w, h, rgba)) = parts.get(i) {
            if let Some(s) = packer.allocate_and_write(*w, *h, rgba) {
                slots.push((*w, *h, s));
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
    for (w, h, slot) in &slots {
        batch.push_atlas_quad(
            slot.key,
            [0.0, 0.0, *w as f32, *h as f32],
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
