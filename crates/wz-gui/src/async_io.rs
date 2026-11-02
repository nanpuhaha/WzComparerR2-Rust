// wz-gui — 비동기 I/O 및 백그라운드 파싱.
//
// `docs/gui-architecture.md` §5: UI 스레드는 블로킹 I/O 를 하지 않는다.
// 워커 스레드가 결과를 채널로 보내고, UI 스레드는 `poll_load_result` 로
// 매 프레임 폴링한다.

use std::sync::{Arc, mpsc};
use std::thread;

use rfd::FileDialog;

use crate::state::{AppState, LoadMsg, TreeRow};
use wz_core::WzArchive;
use wz_core::dirtree::{DirTree, RawEntry};
use wz_core::dirtree::node_type as nt;
use wz_core::image::{ImageValue, Node as ImageNode};
use wz_db::MapData;

// ── 파일 열기 ─────────────────────────────────────────────────────────

pub fn open_file_dialog(state: &mut AppState) {
    if state.loading {
        return;
    }
    let picked = FileDialog::new()
        .add_filter("WZ files", &["wz"])
        .add_filter("All files", &["*"])
        .pick_file();
    let Some(path) = picked else { return; };

    state.loading = true;
    state.status = format!("로딩 중: {}", path.display());
    state.tree = None;
    state.archive = None;
    state.archive_path = Some(path.clone());
    *state.shared_archive.lock().unwrap() = None;
    state.image_children.clear();
    state.expanded.clear();
    state.selected = None;
    state.image_textures.clear();
    state.map_back_image = None;
    state.map_tile_images.clear();
    state.current_map = None;
    state.avatar_image = None;

    let shared = state.shared_archive.clone();
    let (tx, rx) = mpsc::channel();
    state.load_rx = Some(rx);

    thread::spawn(move || {
        let result = (|| -> Result<DirTree, String> {
            let archive = WzArchive::open(&path).map_err(|e| format!("open: {e}"))?;
            let tree = archive.read_tree().map_err(|e| format!("read_tree: {e}"))?;
            *shared.lock().unwrap() = Some(Arc::new(archive));
            Ok(tree)
        })();
        let msg = match result {
            Ok(t) => LoadMsg::Loaded { tree: t },
            Err(e) => LoadMsg::Failed(e),
        };
        let _ = tx.send(msg);
    });
}

pub fn open_file_dialog_with_path(state: &mut AppState, path: std::path::PathBuf) {
    if state.loading { return; }
    state.loading = true;
    state.status = format!("로딩 중: {}", path.display());
    state.tree = None;
    state.archive = None;
    state.archive_path = Some(path.clone());
    *state.shared_archive.lock().unwrap() = None;
    state.image_children.clear();
    state.expanded.clear();
    state.selected = None;
    state.image_textures.clear();
    state.map_back_image = None;
    state.map_tile_images.clear();
    state.map_obj_images.clear();
    state.current_map = None;
    state.avatar_image = None;

    let shared = state.shared_archive.clone();
    let (tx, rx) = std::sync::mpsc::channel();
    state.load_rx = Some(rx);

    std::thread::spawn(move || {
        let result = (|| -> Result<DirTree, String> {
            let archive = WzArchive::open(&path).map_err(|e| format!("open: {e}"))?;
            let tree = archive.read_tree().map_err(|e| format!("read_tree: {e}"))?;
            *shared.lock().unwrap() = Some(std::sync::Arc::new(archive));
            Ok(tree)
        })();
        let msg = match result {
            Ok(t) => crate::state::LoadMsg::Loaded { tree: t },
            Err(e) => crate::state::LoadMsg::Failed(e),
        };
        let _ = tx.send(msg);
    });
}

pub fn poll_load_result(state: &mut AppState, ctx: &egui::Context) {
    let Some(rx) = &state.load_rx else { return; };
    match rx.try_recv() {
        Ok(LoadMsg::Loaded { tree }) => {
            state.status = format!("로드 완료: {} 엔트리(루트)", tree.root.entries.len());
            state.archive = state.shared_archive.lock().unwrap().clone();
            // 맵 WZ 자동 로드: 첫 이미지 엔트리를 map_path 로 설정하고 로드한다.
            let first_img = tree.root.entries.iter().find(|e| e.node_type == nt::IMAGE);
            if let Some(e) = first_img {
                if let Some(name) = &e.name {
                    state.map_path = name.to_string();
                }
            }
            state.tree = Some(tree);
            state.loading = false;
            state.load_rx = None;
            // 자동 맵 로드 (이미지 엔트리가 있을 때)
            if state.archive.is_some() && state.tree.as_ref().map(|t| t.root.entries.iter().any(|e| e.node_type == nt::IMAGE)).unwrap_or(false) {
                load_map_by_path(state);
            }
            ctx.request_repaint();
        }
        Ok(LoadMsg::Failed(e)) => {
            state.status = format!("로드 실패: {e}");
            state.loading = false;
            state.load_rx = None;
        }
        Err(mpsc::TryRecvError::Empty) => {}
        Err(mpsc::TryRecvError::Disconnected) => {
            state.loading = false;
            state.load_rx = None;
        }
    }
}

pub fn poll_map_images(state: &mut AppState, ctx: &egui::Context) {
    let Some(rx) = &state.map_images_rx else { return; };
    match rx.try_recv() {
        Ok(imgs) => {
            state.map_back_image = imgs.back;
            state.map_tile_images = imgs.tiles;
            state.map_obj_images = imgs.objs;
            state.map_images_rx = None;
            state.viewport_dirty = true;
            ctx.request_repaint();
        }
        Err(mpsc::TryRecvError::Empty) => {}
        Err(mpsc::TryRecvError::Disconnected) => {
            state.map_images_rx = None;
        }
    }
}

// ── 트리 행 빌드 ──────────────────────────────────────────────────────

pub fn build_rows(
    rows: &mut Vec<TreeRow>,
    tree: &DirTree,
    archive: Option<&WzArchive>,
    state: &mut AppState,
    depth: usize,
    parent_path: &str,
) {
    for e in &tree.root.entries {
        let name = e.name.clone().unwrap_or_else(|| format!("<ref@{}>", e.file_pos).into());
        let path = if parent_path.is_empty() {
            format!("/{}", name)
        } else {
            format!("{}/{}", parent_path, name)
        };
        let kind = match e.node_type {
            nt::DIRECTORY => "dir",
            nt::IMAGE => "img",
            nt::IMAGE_NAMED_BY_REF => "img*",
            _ => "entry",
        };
        let expandable = e.node_type == nt::DIRECTORY || e.node_type == nt::IMAGE;
        let expanded = state.expanded.contains(&path);

        rows.push(TreeRow {
            depth,
            name: name.to_string(),
            kind: kind.to_string(),
            path: path.clone(),
            expandable,
            expanded,
        });

        if expanded && e.node_type == nt::IMAGE {
            if !state.image_children.contains_key(&path) {
                let children = parse_image_children(archive, e);
                state.image_children.insert(path.clone(), children);
            }
            if let Some(children) = state.image_children.get(&path) {
                for (cn, ct) in children {
                    rows.push(TreeRow {
                        depth: depth + 1,
                        name: cn.clone(),
                        kind: ct.clone(),
                        path: format!("{}/{}", path, cn),
                        expandable: false,
                        expanded: false,
                    });
                }
            }
        }
    }
}

fn parse_image_children(archive: Option<&WzArchive>, e: &RawEntry) -> Vec<(String, String)> {
    let Some(arch) = archive else { return vec![]; };
    let Some(off) = e.offset else { return vec![]; };
    let Ok(node) = arch.parse_image_at(off) else { return vec![]; };
    let mut out = Vec::new();
    if let Some(ImageValue::Property(p)) = &node.value {
        for c in &p.children {
            out.push((c.name.to_string(), value_kind(c)));
        }
    }
    for c in &node.children {
        out.push((c.name.to_string(), value_kind(c)));
    }
    out
}

pub fn value_kind(n: &ImageNode) -> String {
    match &n.value {
        Some(ImageValue::Png(_)) => "canvas".into(),
        Some(ImageValue::Property(_)) => "prop".into(),
        Some(ImageValue::Int(_)) | Some(ImageValue::Short(_)) | Some(ImageValue::Long(_))
        | Some(ImageValue::Uint(_)) | Some(ImageValue::Float(_)) | Some(ImageValue::Double(_))
        | Some(ImageValue::Bool(_)) => "val".into(),
        Some(ImageValue::Str(_)) => "str".into(),
        Some(ImageValue::Vector(_)) => "vec".into(),
        Some(ImageValue::Uol(_)) => "uol".into(),
        _ => "node".into(),
    }
}


pub fn extract_png_ref(node: &ImageNode) -> Option<wz_core::image::PngRef> {
    if let Some(ImageValue::Png(p)) = &node.value {
        return Some(p.clone());
    }
    for ch in &node.children {
        if let Some(p) = extract_png_ref(ch) {
            return Some(p);
        }
    }
    if let Some(ImageValue::Property(p)) = &node.value {
        if let Some(res) = extract_png_ref(p) {
            return Some(res);
        }
    }
    None
}

// ── 맵 로딩 ───────────────────────────────────────────────────────────

pub fn find_data_root(archive_path: Option<&std::path::Path>) -> Option<std::path::PathBuf> {
    // archive_path 에서 "Data" 디렉터리를 찾는다.
    if let Some(p) = archive_path {
        let mut cur = p.to_path_buf();
        while let Some(parent) = cur.parent() {
            if parent.file_name().and_then(|s| s.to_str()) == Some("Data") {
                return Some(parent.to_path_buf());
            }
            cur = parent.to_path_buf();
        }
    }
    // 폴백: 일반적인 설치 경로
    let candidates = [
        "<WZ_DATA_ROOT>",
    ];
    for c in candidates {
        let p = std::path::PathBuf::from(c);
        if p.join("Map").exists() { return Some(p); }
    }
    None
}

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
    load_map_tiles(state);
    load_map_objs(state);

    state.map_status = format!(
        "map_id={:?} bgm={:?} town={} fh={} portals={} life={} back={} obj={} tile={} (back_img={})",
        map.info.map_id, map.info.bgm, map.info.town,
        map.footholds.len(), map.portals.len(), map.life.len(),
        map.backgrounds.len(), map.objects.len(), map.tiles.len(),
        state.map_back_image.is_some()
    );
    state.current_map = Some(map);
    state.viewport_dirty = true;
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
    let fh_count = map.footholds.len();
    let portal_count = map.portals.len();
    let life_count = map.life.len();
    let bgm = map.info.bgm.clone().unwrap_or_default();
    let town = map.info.town;
    state.map_status = format!(
        "map_id={:?} bgm={} town={} footholds={} portals={} life={}",
        map.info.map_id, bgm, town, fh_count, portal_count, life_count
    );
    state.current_map = Some(map);
    load_map_tiles(state);
    load_map_objs(state);
    state.viewport_dirty = true;
}

pub fn load_back_image(data_root: &std::path::Path, b_s: &str) -> Option<(u32, u32, Vec<u8>)> {
    load_canvas_image(data_root, "Back", b_s, "")
}

pub fn load_canvas_image(
    data_root: &std::path::Path,
    subdir: &str,
    canvas_name: &str,
    inner_path: &str,
) -> Option<(u32, u32, Vec<u8>)> {
    let canvas_dir = data_root.join("Map").join(subdir).join("_Canvas");
    let entries = std::fs::read_dir(&canvas_dir).ok()?;
    let img_name = format!("{}.img", canvas_name);
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wz") { continue; }
        let Ok(arch) = WzArchive::open(&path) else { continue; };
        let Ok(tree) = arch.read_tree() else { continue; };
        let e = tree.root.entries.iter().find(|e| {
            e.name.as_deref() == Some(canvas_name) || e.name.as_deref() == Some(img_name.as_str())
        });
        let Some(e) = e else { continue; };
        let Some(off) = e.offset else { continue; };
        let Ok(node) = arch.parse_image_at(off) else { continue; };

        let segs: Vec<&str> = inner_path.split('/').filter(|s| !s.is_empty()).collect();
        let target = if segs.is_empty() {
            &node
        } else {
            let mut cur = &node;
            let mut ok = true;
            for seg in &segs {
                let next = if let Some(ImageValue::Property(p)) = &cur.value {
                    p.children.iter().find(|c| c.name.as_ref() == *seg)
                        .or_else(|| cur.children.iter().find(|c| c.name.as_ref() == *seg))
                } else {
                    cur.children.iter().find(|c| c.name.as_ref() == *seg)
                };
                match next {
                    Some(n) => cur = n,
                    None => { ok = false; break; }
                }
            }
            if !ok { continue; }
            cur
        };

        fn find_png(n: &ImageNode) -> Option<wz_core::image::PngRef> {
            use wz_core::image::ImageValue;
            if let Some(ImageValue::Png(p)) = &n.value { return Some(p.clone()); }
            for c in &n.children {
                if let Some(p) = find_png(c) { return Some(p); }
            }
            if let Some(ImageValue::Property(p)) = &n.value {
                if let Some(res) = find_png(p) { return Some(res); }
            }
            None
        }
        let Some(pr) = find_png(target) else { continue; };
        let file_src = wz_pool::FileSource::open(&path).ok()?;
        let src: std::sync::Arc<dyn wz_pool::ByteSource> = std::sync::Arc::new(file_src);
        if let Ok((w, h, rgba)) = wz_image::decode_png_ref_to_rgba(&pr, &src) {
            return Some((w, h, rgba));
        }
    }
    None
}

pub fn load_map_tiles(state: &mut AppState) {
    state.map_tile_images.clear();
    state.map_obj_images.clear();
    let (Some(map), Some(root)) = (state.current_map.as_ref(), state.data_root.as_ref()) else { return; };
    for tile in map.tiles.iter() {
        if let Some((w, h, rgba)) = load_canvas_image(root, "Tile", &tile.t_s, &format!("{}/{}", tile.u, tile.no)) {
            state.map_tile_images.push((tile.x, tile.y, w, h, rgba));
        }
    }
    state.viewport_dirty = true;
}

pub fn load_map_objs(state: &mut AppState) {
    let (Some(map), Some(root)) = (state.current_map.as_ref(), state.data_root.as_ref()) else { return; };
    for obj in map.objects.iter() {
        let inner = format!("{}/{}/{}/0", obj.l0, obj.l1, obj.l2);
        if let Some((w, h, rgba)) = load_canvas_image(root, "Obj", &obj.o_s, &inner) {
            state.map_obj_images.push((obj.x, obj.y, w, h, rgba));
        }
    }
    state.viewport_dirty = true;
}


pub fn load_avatar_body(state: &mut AppState) {
    // Character/_Canvas/_Canvas_000.wz 에서 첫 번째 이미지를 로드한다.
    let canvas_path = "<WZ_DATA_ROOT>/Character/_Canvas/_Canvas_000.wz";
    let cp = std::path::PathBuf::from(canvas_path);
    let Ok(arch) = WzArchive::open(&cp) else {
        state.avatar_status = "Character _Canvas 열기 실패".into();
        return;
    };
    let Ok(tree) = arch.read_tree() else {
        state.avatar_status = "트리 읽기 실패".into();
        return;
    };
    let Some(img_entry) = tree.root.entries.iter().find(|e| e.node_type == nt::IMAGE) else {
        state.avatar_status = "이미지 없음".into();
        return;
    };
    let Some(off) = img_entry.offset else {
        state.avatar_status = "오프셋 없음".into();
        return;
    };
    let Ok(node) = arch.parse_image_at(off) else {
        state.avatar_status = "이미지 파싱 실패".into();
        return;
    };
    fn find_png(n: &ImageNode) -> Option<wz_core::image::PngRef> {
        use wz_core::image::ImageValue;
        if let Some(ImageValue::Png(p)) = &n.value { return Some(p.clone()); }
        for c in &n.children { if let Some(p) = find_png(c) { return Some(p); } }
        if let Some(ImageValue::Property(p)) = &n.value { if let Some(res) = find_png(p) { return Some(res); } }
        None
    }
    let Some(pr) = find_png(&node) else {
        state.avatar_status = "캔버스 없음".into();
        return;
    };
    let file_src = match wz_pool::FileSource::open(&cp) { Ok(f) => f, Err(_) => { state.avatar_status = "파일 열기 실패".into(); return; } };
    let src: std::sync::Arc<dyn wz_pool::ByteSource> = std::sync::Arc::new(file_src);
    match wz_image::decode_png_ref_to_rgba(&pr, &src) {
        Ok((w, h, rgba)) => {
            state.avatar_status = format!("Body 이미지 로드: {}x{} ({} bytes)", w, h, rgba.len());
            state.avatar_image = Some((w, h, rgba));
            state.viewport_dirty = true;
        }
        Err(e) => {
            state.avatar_status = format!("디코딩 실패: {e}");
        }
    }
}
// ── 트리 행 렌더링 ────────────────────────────────────────────────────

pub fn render_row(ui: &mut egui::Ui, state: &mut AppState, row: &TreeRow) {
    ui.horizontal(|ui| {
        ui.add_space((row.depth as f32) * 14.0);
        if row.expandable {
            let label = if row.expanded { "▼" } else { "▶" };
            if ui.small_button(label).clicked() {
                if row.expanded {
                    state.expanded.remove(&row.path);
                } else {
                    state.expanded.insert(row.path.clone());
                }
            }
        } else {
            ui.add_space(18.0);
        }
        let selected = state.selected.as_deref() == Some(row.path.as_str());
        let text = format!("{}  [{}]", row.name, row.kind);
        if ui.selectable_label(selected, text).clicked() {
            state.selected = Some(row.path.clone());
            if row.kind == "img" {
                load_image_view(state, &row.path);
            }
        }
    });
}

fn load_image_view(state: &mut AppState, path: &str) {
    if state.image_textures.contains_key(path) {
        state.current_image_path = Some(path.to_string());
        return;
    }
    let Some(tree) = &state.tree else { return; };
    let archive = state.shared_archive.lock().unwrap().clone();
    let Some(archive) = archive else { return; };
    let name = path.split('/').rfind(|s| !s.is_empty()).unwrap_or("");
    let entry = tree.root.entries.iter().find(|e| e.name.as_deref() == Some(name));
    let Some(e) = entry else {
        state.current_image_info = Some(format!("엔트리를 찾을 수 없음: {name}"));
        return;
    };
    let Some(off) = e.offset else {
        state.current_image_info = Some("오프셋 없음".into());
        state.current_image_path = Some(path.to_string());
        return;
    };
    let Ok(node) = archive.parse_image_at(off) else {
        state.current_image_info = Some("이미지 파싱 실패".into());
        state.current_image_path = Some(path.to_string());
        return;
    };
    let png_ref = extract_png_ref(&node);
    if let Some(pr) = png_ref {
        state.current_image_info = Some(format!(
            "canvas: {}x{} format={} scale={} pages={}",
            pr.width, pr.height, pr.format, pr.scale, pr.pages
        ));
    } else {
        state.current_image_info = Some("캔버스 없음".into());
    }
    state.current_image_path = Some(path.to_string());
}

// ── 아바타 렌더링 (PoC 3.6) ──────────────────────────────────────────

pub fn render_avatar_body(state: &mut AppState) {
    let Some((w, h, rgba)) = state.avatar_image.clone() else {
        state.avatar_status = "Body 이미지를 먼저 로드하세요".into();
        return;
    };
    use wz_render::{atlas::AtlasShelfPacker, batch::SpriteBatch, camera::Camera2D, device::RenderContext};

    let mut rc = match RenderContext::new_headless() {
        Ok(r) => r,
        Err(e) => { state.avatar_status = format!("RenderContext 실패: {e}"); return; }
    };
    let mut packer = AtlasShelfPacker::new(w.max(1), h.max(1));
    let slot = match packer.allocate_and_write(w, h, &rgba) {
        Some(s) => s,
        None => { state.avatar_status = "아틀라스 할당 실패".into(); return; }
    };
    for page in packer.pages() {
        let _ = rc.create_texture(page.key, page.width, page.height, &page.pixels);
    }
    let mut batch = SpriteBatch::new();
    batch.push_atlas_quad(slot.key, [0.0, 0.0, w as f32, h as f32], 0.0, &slot.rect, (w, h), [1.0, 1.0, 1.0, 1.0]);
    batch.finish();

    let cam = Camera2D::new(w as f32, h as f32);
    let uniforms = cam.build_uniforms(0.0, 0.0);
    match rc.render_to_rgba(w, h, &batch, &uniforms, [0.08, 0.08, 0.12, 1.0]) {
        Ok(out) => {
            state.avatar_status = format!("아바타 렌더 완료: {}x{}", w, h);
            state.avatar_rendered = Some((w, h, out));
        }
        Err(e) => { state.avatar_status = format!("렌더 실패: {e}"); }
    }
}

pub fn load_avatar_from_loader(state: &mut AppState) {
    let char_wz = "<WZ_DATA_ROOT>/Character/Character.wz";
    match wz_avatar::load_from_character_wz(std::path::Path::new(char_wz)) {
        Some(av) => {
            state.avatar_status = format!("로드: {} 파트", av.parts.len());
            if let Some(first) = av.parts.first() {
                state.avatar_image = Some((first.width, first.height, first.rgba.clone()));
            }
            // 아틀라스 패킹 + 다중 파트 렌더
            use wz_render::{atlas::AtlasShelfPacker, batch::SpriteBatch, camera::Camera2D, device::RenderContext};
            let mut packer = AtlasShelfPacker::new(512, 512);
            // 선택된 슬롯만 렌더한다 (PoC 3.9: 29슬롯 선택)
            let selected: Vec<usize> = if state.avatar_selected_slots.is_empty() {
                (0..av.parts.len()).collect()
            } else {
                state.avatar_selected_slots.iter().copied().collect()
            };
            let mut slots = Vec::new();
            for &i in &selected {
                if let Some(part) = av.parts.get(i) {
                    if let Some(slot) = packer.allocate_and_write(part.width, part.height, &part.rgba) {
                        slots.push((part, slot));
                    }
                }
            }
            let mut rc = match RenderContext::new_headless() {
                Ok(r) => r,
                Err(e) => { state.avatar_status = format!("RC 실패: {e}"); return; }
            };
            for page in packer.pages() {
                let _ = rc.create_texture(page.key, page.width, page.height, &page.pixels);
            }
            let mut batch = SpriteBatch::new();
            for (part, slot) in &slots {
                batch.push_atlas_quad(slot.key, [0.0, 0.0, part.width as f32, part.height as f32], 0.0, &slot.rect, (512, 512), [1.0, 1.0, 1.0, 1.0]);
            }
            batch.finish();
            let (rw, rh) = (256u32, 256u32);
            let cam = Camera2D::new(rw as f32, rh as f32);
            let uniforms = cam.build_uniforms(0.0, 0.0);
            match rc.render_to_rgba(rw, rh, &batch, &uniforms, [0.08, 0.08, 0.12, 1.0]) {
                Ok(out) => {
                    state.avatar_status = format!("렌더 완료: {} 파트, {}x{}", av.parts.len(), rw, rh);
                    state.avatar_rendered = Some((rw, rh, out));
                }
                Err(e) => { state.avatar_status = format!("렌더 실패: {e}"); }
            }
        }
        None => {
            state.avatar_status = "아바타 로드 실패".into();
        }
    }
}
