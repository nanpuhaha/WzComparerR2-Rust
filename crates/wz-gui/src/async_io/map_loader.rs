// wz-gui — 비동기 I/O 및 백그라운드 파싱.
//
// `docs/gui-architecture.md` §5: UI 스레드는 블로킹 I/O 를 하지 않는다.
// 워커 스레드가 결과를 채널로 보내고, UI 스레드는 `poll_load_result` 로
// 매 프레임 폴링한다.



use crate::state::AppState;
use wz_db::MapData;
use super::*;

// ── 파일 열기 ─────────────────────────────────────────────────────────

pub fn load_map_by_path(state: &mut AppState) {
    if state.tree.is_none() {
        state.map_status = "WZ 파일을 먼저 열어 주세요".into();
        return;
    }
    let Some(archive) = state.shared_archive.lock().unwrap().clone() else {
        state.map_status = "아카이브 없음".into();
        return;
    };
    let path = state.map_path.clone();
    let Some(off) = archive.find_entry_offset(&path) else {
        state.map_status = format!("엔트리 없음: {path}");
        return;
    };
    let Ok(node) = archive.parse_image_at(off) else {
        state.map_status = "이미지 파싱 실패".into();
        return;
    };
    let map = MapData::parse_from_node(&node);

    let data_root = find_data_root(state.archive_path.as_deref());
    state.data_root = data_root.clone();

    let back_img = if let (Some(root), Some(first)) = (data_root.as_ref(), map.backgrounds.first()) {
        load_back_image(root, &first.b_s)
    } else {
        None
    };
    state.map_back_image = back_img;

    // 타일/오브젝트 이미지는 워커 스레드에서 로드한다 (UI 블로킹 방지).
    if let Some(root) = data_root.clone() {
        let tiles_clone = map.tiles.clone();
        let objs_clone = map.objects.clone();
        let (tx, rx) = crossbeam_channel::unbounded();
        state.map_images_rx = Some(rx);
        std::thread::spawn(move || {
            let mut tiles = Vec::new();
            for tile in tiles_clone.iter() {
                if let Some((w, h, rgba)) = load_canvas_image(&root, "Tile", &tile.t_s, &format!("{}/{}", tile.u, tile.no)) {
                    tiles.push((tile.x, tile.y, w, h, rgba));
                }
            }
            let mut objs = Vec::new();
            for obj in &objs_clone {
                let inner = format!("{}/{}/{}/0", obj.l0, obj.l1, obj.l2);
                if let Some((w, h, rgba)) = load_canvas_image(&root, "Obj", &obj.o_s, &inner) {
                    objs.push((obj.x, obj.y, w, h, rgba));
                }
            }
            let _ = tx.send(crate::state::MapImages {
                tiles,
                objs,
            });
        });
    }

    state.map_status = format!(
        "map_id={:?} bgm={:?} town={} fh={} portals={} life={} back={} obj={} tile={} (back_img={})",
        map.info.map_id, map.info.bgm, map.info.town,
        map.footholds.len(), map.portals.len(), map.life.len(),
        map.backgrounds.len(), map.objects.len(), map.tiles.len(),
        state.map_back_image.is_some()
    );
    state.current_map = Some(map);
    state.viewport_dirty = true;
    state.viewport_map_dirty = true;
}

pub fn load_map_from_selection(state: &mut AppState) {
    let Some(sel) = state.selected.clone() else { return; };
    let Some(tree) = &state.tree else { return; };
    let Some(archive) = state.shared_archive.lock().unwrap().clone() else { return; };
    let name = sel.split('/').rfind(|s| !s.is_empty()).unwrap_or("");
    let Some(e) = tree.root.entries.iter().find(|e| e.name.as_deref() == Some(name)) else {
        state.map_status = format!("엔트리 없음: {name}");
        return;
    };
    let Some(off) = e.offset else {
        state.map_status = "오프셋 없음".into();
        return;
    };
    let Ok(node) = archive.parse_image_at(off) else {
        state.map_status = "이미지 파싱 실패".into();
        return;
    };
    let map = MapData::parse_from_node(&node);
    state.map_status = format!(
        "map_id={:?} bgm={:?} town={} fh={} portals={} life={}",
        map.info.map_id, map.info.bgm, map.info.town,
        map.footholds.len(), map.portals.len(), map.life.len()
    );
    // map_path 를 갱신하고 load_map_by_path 로 이미지 로딩을 위임한다.
    state.map_path = name.to_string();
    state.current_map = Some(map);
    // 이미지 로딩은 load_map_by_path 의 워커가 처리한다.
    // (map_path 가 설정되어 있으므로 그대로 호출)
    load_map_by_path(state);
}

