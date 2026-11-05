// wz-gui — 비동기 I/O 및 백그라운드 파싱.
//
// `docs/gui-architecture.md` §5: UI 스레드는 블로킹 I/O 를 하지 않는다.
// 워커 스레드가 결과를 채널로 보내고, UI 스레드는 `poll_load_result` 로
// 매 프레임 폴링한다.

use crossbeam_channel;

use super::*;
use crate::state::{AppState, LoadMsg, TreeRow};
use wz_core::dirtree::node_type as nt;
use wz_core::dirtree::{DirTree, RawEntry};
use wz_core::image::{ImageValue, Node as ImageNode};
use wz_core::WzArchive;

// ── 파일 열기 ─────────────────────────────────────────────────────────

pub fn poll_load_result(state: &mut AppState, ctx: &egui::Context) {
    let Some(rx) = &state.load_rx else {
        return;
    };
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
            if state.archive.is_some()
                && state
                    .tree
                    .as_ref()
                    .map(|t| t.root.entries.iter().any(|e| e.node_type == nt::IMAGE))
                    .unwrap_or(false)
            {
                load_map_by_path(state);
            }
            ctx.request_repaint();
        }
        Ok(LoadMsg::Failed(e)) => {
            state.status = format!("로드 실패: {e}");
            state.loading = false;
            state.load_rx = None;
        }
        Err(crossbeam_channel::TryRecvError::Empty) => {}
        Err(crossbeam_channel::TryRecvError::Disconnected) => {
            state.loading = false;
            state.load_rx = None;
        }
    }
}

pub fn poll_map_images(state: &mut AppState, ctx: &egui::Context) {
    let Some(rx) = &state.map_images_rx else {
        return;
    };
    match rx.try_recv() {
        Ok(imgs) => {
            // back 은 이미 동기적으로 로드되었으므로 덮어쓰지 않는다.
            state.map_tile_images = imgs.tiles;
            state.map_obj_images = imgs.objs;
            state.map_reactor_images = imgs.reactors;
            state.map_background_images = imgs.backgrounds;
            state.map_particle_layers = imgs.particles;
            state.map_images_rx = None;
            state.viewport_dirty = true;
            ctx.request_repaint();
        }
        Err(crossbeam_channel::TryRecvError::Empty) => {}
        Err(crossbeam_channel::TryRecvError::Disconnected) => {
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
        let name = e
            .name
            .clone()
            .unwrap_or_else(|| format!("<ref@{}>", e.file_pos).into());
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
    let Some(arch) = archive else {
        return vec![];
    };
    let Some(off) = e.offset else {
        return vec![];
    };
    let Ok(node) = arch.parse_image_at(off) else {
        return vec![];
    };
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
        Some(ImageValue::Int(_))
        | Some(ImageValue::Short(_))
        | Some(ImageValue::Long(_))
        | Some(ImageValue::Uint(_))
        | Some(ImageValue::Float(_))
        | Some(ImageValue::Double(_))
        | Some(ImageValue::Bool(_)) => "val".into(),
        Some(ImageValue::Str(_)) => "str".into(),
        Some(ImageValue::Vector(_)) => "vec".into(),
        Some(ImageValue::Uol(_)) => "uol".into(),
        _ => "node".into(),
    }
}

// 캔버스 탐색은 wz_core::image::Node::first_canvas() 를 사용한다 (단일 규칙).

// ── 맵 로딩 ───────────────────────────────────────────────────────────

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
    let Some(tree) = &state.tree else {
        return;
    };
    let archive = state.shared_archive.lock().unwrap().clone();
    let Some(archive) = archive else {
        return;
    };
    let name = path.split('/').rfind(|s| !s.is_empty()).unwrap_or("");
    let entry = tree
        .root
        .entries
        .iter()
        .find(|e| e.name.as_deref() == Some(name));
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
    let png_ref = node.first_canvas();
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
