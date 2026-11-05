// wz-gui — 비동기 I/O 및 백그라운드 파싱.
//
// `docs/gui-architecture.md` §5: UI 스레드는 블로킹 I/O 를 하지 않는다.
// 워커 스레드가 결과를 채널로 보내고, UI 스레드는 `poll_load_result` 로
// 매 프레임 폴링한다.

use crate::state::{AppState, AvatarPartData};

fn apply_chair_metadata(state: &mut AppState, av: &wz_avatar::LoadedAvatar) {
    state.chair_type = av.chair_info.as_ref().and_then(|info| {
        info.custom_chair
            .as_ref()
            .map(|custom| match &custom.chair_type {
                wz_avatar::CustomChairType::ScaleAvatarChair => "ScaleAvatarChair".to_string(),
                wz_avatar::CustomChairType::Unsupported => "Unsupported".to_string(),
                wz_avatar::CustomChairType::Other(name) => name.clone(),
            })
    });
    state.chair_event_point = av
        .chair_info
        .as_ref()
        .and_then(|info| info.custom_chair.as_ref())
        .and_then(|custom| custom.event_point.as_ref())
        .map(|event| (event.index, event.point, event.term));
}

fn part_data(p: &wz_avatar::AvatarPart) -> AvatarPartData {
    AvatarPartData {
        slot: p.slot.clone(),
        width: p.width,
        height: p.height,
        rgba: p.rgba.clone(),
        position: (p.position.x, p.position.y),
        z_index: p.z_index,
    }
}

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
            state.avatar_loaded_parts = av.parts.iter().map(part_data).collect();
            if let Some(first) = av.parts.first() {
                state.avatar_image = Some((first.width, first.height, first.rgba.clone()));
            }
            state.effect_frames.clear();
            state.effect_delays.clear();
            state.effect_origins.clear();
            state.effect_origin = (0, 0);
            state.effect_last_time = 0.0;
            state.chair_frames.clear();
            state.chair_delays.clear();
            state.chair_frame = 0;
            state.chair_last_time = 0.0;
            state.chair_type = None;
            state.chair_event_point = None;
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
            state.avatar_loaded_parts = av.parts.iter().map(part_data).collect();
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
            state.chair_frames = av
                .chair_frames
                .iter()
                .map(|frame| frame.iter().map(part_data).collect())
                .collect();
            state.chair_delays = av.chair_delays.clone();
            state.chair_frame = 0;
            state.chair_last_time = 0.0;
            apply_chair_metadata(state, &av);
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
    let parts: Vec<AvatarPartData> = state
        .avatar_loaded_parts
        .iter()
        .map(|p| {
            let mut rgba = p.rgba.clone();
            if apply_prism {
                wz_avatar::apply_prism_rgba8(&mut rgba, &prism, false);
            }
            AvatarPartData { rgba, ..p.clone() }
        })
        .collect();
    if let Some(first) = parts.first() {
        state.avatar_image = Some((first.width, first.height, first.rgba.clone()));
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
        if let Some(p) = parts.get(i) {
            if let Some(s) = packer.allocate_and_write(p.width, p.height, &p.rgba) {
                slots.push((p.clone(), s));
            }
        }
    }
    if slots.is_empty() {
        state.avatar_status = "선택된 파트가 없습니다".into();
        return;
    }

    // 파트의 최종 좌표를 합쳐 아바타를 중앙에 배치하고, 256x256 에 맞게 축소한다.
    let (rw, rh) = (256u32, 256u32);
    let effective_pos = |p: &AvatarPartData| -> (i32, i32) {
        if p.slot == "effect" {
            (-state.effect_origin.0, -state.effect_origin.1)
        } else {
            p.position
        }
    };
    let mut min_x = i32::MAX;
    let mut min_y = i32::MAX;
    let mut max_x = i32::MIN;
    let mut max_y = i32::MIN;
    for (p, _) in &slots {
        let (x, y) = effective_pos(p);
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x + p.width as i32);
        max_y = max_y.max(y + p.height as i32);
    }
    let bbox_w = (max_x - min_x).max(1) as f32;
    let bbox_h = (max_y - min_y).max(1) as f32;
    let scale = (rw as f32 / bbox_w).min(rh as f32 / bbox_h).min(1.0);
    let off_x = (rw as f32 - bbox_w * scale) * 0.5 - min_x as f32 * scale;
    let off_y = (rh as f32 - bbox_h * scale) * 0.5 - min_y as f32 * scale;

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
    for (p, slot) in &slots {
        let (x, y) = effective_pos(p);
        batch.push_atlas_quad(
            slot.key,
            [
                off_x + x as f32 * scale,
                off_y + y as f32 * scale,
                p.width as f32 * scale,
                p.height as f32 * scale,
            ],
            p.z_index as f32,
            &slot.rect,
            (512, 512),
            [1.0, 1.0, 1.0, 1.0],
        );
    }
    batch.finish();
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

/// 체어 애니메이션 틱. 다음 프레임으로 넘어갔으면 `true`.
pub fn tick_chair(state: &mut AppState, now: f64) -> bool {
    if !state.chair_auto_play || state.chair_frames.is_empty() {
        return false;
    }
    let frame = state.chair_frame % state.chair_frames.len();
    let delay = state
        .chair_delays
        .get(frame)
        .copied()
        .unwrap_or(120)
        .max(20) as f64
        / 1000.0;
    if state.chair_last_time <= 0.0 {
        state.chair_last_time = now;
        return false;
    }
    if now - state.chair_last_time < delay {
        return false;
    }
    state.chair_last_time = now;
    state.chair_frame = (frame + 1) % state.chair_frames.len();
    apply_chair_frame(state);
    true
}

/// 현재 `chair_frame` 의 rolling house 파츠를 아바타 파트 목록에 반영한다.
pub fn apply_chair_frame(state: &mut AppState) {
    if state.chair_frames.is_empty() {
        return;
    }
    let idx = state.chair_frame % state.chair_frames.len();
    state
        .avatar_loaded_parts
        .retain(|p| !p.slot.starts_with("rolling_part") && p.slot != "android_effect");
    state
        .avatar_loaded_parts
        .extend(state.chair_frames[idx].iter().cloned());
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
    let origin = state.effect_origin;
    if let Some(part) = state
        .avatar_loaded_parts
        .iter_mut()
        .find(|p| p.slot == "effect")
    {
        part.width = w;
        part.height = h;
        part.rgba = rgba;
        part.position = (-origin.0, -origin.1);
    } else {
        state.avatar_loaded_parts.push(AvatarPartData {
            slot: "effect".into(),
            width: w,
            height: h,
            rgba,
            position: (-origin.0, -origin.1),
            z_index: 0,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_flip_chair_smoke() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let Some(av) =
            wz_avatar::load_avatar_with_equipment(&root, 2000, &[("Chair".to_string(), 3019313)])
        else {
            return;
        };
        let mut state = AppState {
            avatar_loaded_parts: av
                .parts
                .iter()
                .map(|p| AvatarPartData {
                    slot: p.slot.clone(),
                    width: p.width,
                    height: p.height,
                    rgba: p.rgba.clone(),
                    position: (p.position.x, p.position.y),
                    z_index: p.z_index,
                })
                .collect(),
            ..Default::default()
        };
        render_loaded_avatar(&mut state);
        if state.avatar_status.contains("RC 실패") {
            return; // GPU 가 없는 환경은 skip
        }
        assert!(
            state.avatar_rendered.is_some(),
            "render failed: {}",
            state.avatar_status
        );
    }

    #[test]
    fn render_rolling_house_chair_smoke() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let Some(av) =
            wz_avatar::load_avatar_with_equipment(&root, 2000, &[("Chair".to_string(), 3018010)])
        else {
            return;
        };
        assert!(
            av.parts.iter().any(|p| p.slot.starts_with("rolling_part")),
            "rolling house parts missing"
        );
        let mut state = AppState {
            avatar_loaded_parts: av.parts.iter().map(part_data).collect(),
            chair_frames: av
                .chair_frames
                .iter()
                .map(|frame| frame.iter().map(part_data).collect())
                .collect(),
            chair_delays: av.chair_delays.clone(),
            ..Default::default()
        };
        assert!(
            state.chair_frames.len() > 1,
            "expected multiple rolling house frames"
        );
        render_loaded_avatar(&mut state);
        if state.avatar_status.contains("RC 실패") {
            return;
        }
        assert!(
            state.avatar_rendered.is_some(),
            "render failed: {}",
            state.avatar_status
        );

        // 두 번째 프레임으로 교체해도 렌더가 유지되어야 한다.
        state.chair_frame = 1;
        apply_chair_frame(&mut state);
        render_loaded_avatar(&mut state);
        assert!(
            state.avatar_rendered.is_some(),
            "frame render failed: {}",
            state.avatar_status
        );
    }

    #[test]
    fn extracts_event_point_chair_metadata() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let Some(av) =
            wz_avatar::load_avatar_with_equipment(&root, 2000, &[("Chair".to_string(), 3018093)])
        else {
            return;
        };
        let mut state = AppState::default();
        apply_chair_metadata(&mut state, &av);
        assert_eq!(state.chair_type.as_deref(), Some("eventPointChair"));
        assert_eq!(state.chair_event_point, Some((1, 3, 10000)));
    }

    #[test]
    fn render_android_chair_smoke() {
        let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
            return;
        };
        let Some(av) =
            wz_avatar::load_avatar_with_equipment(&root, 2000, &[("Chair".to_string(), 3018020)])
        else {
            return;
        };
        assert!(av.chair_frames.len() > 1, "android chair frames missing");
        let mut state = AppState {
            avatar_loaded_parts: av.parts.iter().map(part_data).collect(),
            chair_frames: av
                .chair_frames
                .iter()
                .map(|frame| frame.iter().map(part_data).collect())
                .collect(),
            chair_delays: av.chair_delays.clone(),
            ..Default::default()
        };
        render_loaded_avatar(&mut state);
        if state.avatar_status.contains("RC 실패") {
            return;
        }
        assert!(
            state.avatar_rendered.is_some(),
            "render failed: {}",
            state.avatar_status
        );
    }

    #[test]
    fn apply_chair_frame_replaces_rolling_parts() {
        let body = AvatarPartData {
            slot: "body".into(),
            width: 1,
            height: 1,
            rgba: vec![1, 2, 3, 4],
            position: (0, 0),
            z_index: 0,
        };
        let rolling = |n: u8| AvatarPartData {
            slot: "rolling_part0".into(),
            width: 1,
            height: 1,
            rgba: vec![n, n, n, 255],
            position: (0, 0),
            z_index: 0,
        };
        let android = |n: u8| AvatarPartData {
            slot: "android_effect".into(),
            width: 1,
            height: 1,
            rgba: vec![n, n, n, 255],
            position: (0, 0),
            z_index: 0,
        };
        let mut state = AppState {
            avatar_loaded_parts: vec![body.clone(), rolling(1), android(1)],
            chair_frames: vec![vec![rolling(1), android(1)], vec![rolling(2), android(2)]],
            chair_frame: 1,
            ..Default::default()
        };
        apply_chair_frame(&mut state);
        assert_eq!(state.avatar_loaded_parts.len(), 3);
        assert!(state.avatar_loaded_parts.iter().any(|p| p.slot == "body"));
        let part = state
            .avatar_loaded_parts
            .iter()
            .find(|p| p.slot == "rolling_part0")
            .expect("rolling part");
        assert_eq!(part.rgba, vec![2, 2, 2, 255]);
        let android_part = state
            .avatar_loaded_parts
            .iter()
            .find(|p| p.slot == "android_effect")
            .expect("android part");
        assert_eq!(android_part.rgba, vec![2, 2, 2, 255]);
    }

    #[test]
    fn tick_chair_advances_after_delay() {
        let mut state = AppState {
            chair_frames: vec![vec![], vec![]],
            chair_delays: vec![100, 100],
            chair_auto_play: true,
            chair_last_time: 10.0,
            ..Default::default()
        };
        assert!(!tick_chair(&mut state, 10.05));
        assert_eq!(state.chair_frame, 0);
        assert!(tick_chair(&mut state, 10.2));
        assert_eq!(state.chair_frame, 1);
    }

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
            .find(|p| p.slot == "effect")
            .expect("effect part");
        assert_eq!((part.width, part.height), (3, 3));
        assert_eq!(part.rgba.len(), 36);
        assert_eq!(part.position, (7, -8));
    }
}
