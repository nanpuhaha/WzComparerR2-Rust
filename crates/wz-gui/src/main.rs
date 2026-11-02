// wz-gui — Phase 8 데스크톱 GUI PoC 1.
//
// PoC 0 (egui+wgpu 컴파일/윈도우) 위에 WZ 트리 브라우저를 얹는다.
//   - rfd 네이티브 다이얼로그로 WZ 파일 열기 (워커 스레드, UI 비블로킹)
//   - DirTree 를 가상 스크롤 트리 뷰로 표시 (expand/collapse)
//   - 이미지 엔트리 확장 시 parse_image_at 로 하위 노드 지연 로드
//   - 선택된 이미지의 첫 캔버스를 CPU 디코딩 → egui 텍스처로 표시
//   - 프로퍼티 인스펙터 (이름/타입/크기/체크섬/오프셋)

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;

use egui::TextureHandle;
use wz_core::WzArchive;
use wz_pool;
use wz_core::dirtree::{DirTree, RawEntry};
use wz_core::dirtree::node_type as nt;
use wz_core::image::{ImageValue, Node as ImageNode};
use wz_db::MapData;
use wz_render::camera::Camera2D;
use wz_render::atlas::AtlasShelfPacker;
use wz_render::batch::SpriteBatch;
use wz_render::GpuKey;
use glam::Vec2;

// ── 로딩 메시지 ───────────────────────────────────────────────────────

enum LoadMsg {
    Loaded { tree: DirTree },
    Failed(String),
}

// ── 트리 행 ───────────────────────────────────────────────────────────

#[derive(Clone)]
struct TreeRow {
    depth: usize,
    name: String,
    kind: String,        // "dir" | "img" | "prop" | "canvas"
    path: String,        // 고유 키 (확장/선택에 사용)
    expandable: bool,
    expanded: bool,
    offset: Option<u64>,
    data_len: i32,
    cs32: i32,
}

// ── 앱 상태 ───────────────────────────────────────────────────────────

struct AppState {
    archive: Option<Arc<WzArchive>>,
    shared_archive: Arc<Mutex<Option<Arc<WzArchive>>>>,
    tree: Option<DirTree>,
    // 확장된 이미지 엔트리 → 파싱된 하위 (name, type) 캐시
    image_children: HashMap<String, Vec<(String, String)>>,
    // 확장된 경로 집합
    expanded: HashSet<String>,
    selected: Option<String>,
    // 디코딩된 이미지 텍스처 캐시 (경로 → 텍스처)
    image_textures: HashMap<String, TextureHandle>,
    // 현재 이미지 뷰어 상태
    current_image_path: Option<String>,
    current_image_info: Option<String>,
    // PoC 2.5: 맵
    map_path: String,
    current_map: Option<MapData>,
    map_status: String,
    map_back_image: Option<(u32, u32, Vec<u8>)>,
    data_root: Option<std::path::PathBuf>,
    // PoC 3.5: 아바타
    avatar_image: Option<(u32, u32, Vec<u8>)>,
    avatar_status: String,
    // PoC 2: 맵 뷰포트
    camera: Camera2D,
    viewport_texture: Option<TextureHandle>,
    viewport_dirty: bool,
    viewport_size: (u32, u32),
    viewport_dragging: bool,
    viewport_last_pos: Option<egui::Pos2>,
    // 로딩
    loading: bool,
    load_rx: Option<mpsc::Receiver<LoadMsg>>,
    status: String,
    search: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            archive: None,
            shared_archive: Arc::new(Mutex::new(None)),
            tree: None,
            image_children: HashMap::new(),
            expanded: HashSet::new(),
            selected: None,
            image_textures: HashMap::new(),
            current_image_path: None,
            current_image_info: None,
            map_path: "Map0/000010000".into(),
            current_map: None,
            map_back_image: None,
            data_root: None,
            map_status: "맵 ID 를 입력하고 'Load Map'".into(),
            avatar_image: None,
            avatar_status: "Body 이미지를 로드하세요".into(),
            camera: Camera2D::new(512.0, 512.0),
            viewport_texture: None,
            viewport_dirty: true,
            viewport_size: (512, 512),
            viewport_dragging: false,
            viewport_last_pos: None,
            loading: false,
            load_rx: None,
            status: "WZ 파일을 열어 주세요 (Ctrl+O)".into(),
            search: String::new(),
        }
    }
}

struct PocApp {
    state: AppState,
}

impl Default for PocApp {
    fn default() -> Self {
        Self { state: AppState::default() }
    }
}

// ── eframe App ────────────────────────────────────────────────────────

impl eframe::App for PocApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // 로딩 결과 폴링
        poll_load_result(&mut self.state, ui.ctx());

        // 메뉴바
        egui::Panel::top("menubar").show(ui, |ui| {
            ui.horizontal(|ui| {
                if ui.button("열기 (Ctrl+O)").clicked() {
                    open_file_dialog(&mut self.state);
                }
                if self.state.loading {
                    ui.spinner();
                    ui.label(&self.state.status);
                } else {
                    ui.label(&self.state.status);
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(format!("wz-gui PoC 1"));
                });
            });
        });

        // 좌측: 트리 뷰어
        egui::Panel::left("tree_panel")
            .default_size(320.0)
            .show(ui, |ui| {
                ui.label("WZ 트리");
                ui.text_edit_singleline(&mut self.state.search);
                ui.separator();
                render_tree_panel(ui, &mut self.state);
            });

        // 우측: 인스펙터 + 이미지 뷰어
        egui::CentralPanel::default().show(ui, |ui| {
            render_inspector(ui, &mut self.state);
            ui.separator();
            ui.heading("맵 뷰포트 (PoC 2)");
            render_viewport(ui, &mut self.state);

            ui.separator();
            ui.heading("아바타 (PoC 3)");
            render_avatar_panel(ui, &mut self.state);
        });
    }
}

// ── 파일 열기 ─────────────────────────────────────────────────────────

fn open_file_dialog(state: &mut AppState) {
    if state.loading {
        return;
    }
    let picked = rfd::FileDialog::new()
        .add_filter("WZ files", &["wz"])
        .add_filter("All files", &["*"])
        .pick_file();
    let Some(path) = picked else { return; };

    state.loading = true;
    state.status = format!("로딩 중: {}", path.display());
    state.tree = None;
    state.archive = None;
    *state.shared_archive.lock().unwrap() = None;
    state.image_children.clear();
    state.expanded.clear();
    state.selected = None;
    state.image_textures.clear();

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

fn poll_load_result(state: &mut AppState, ctx: &egui::Context) {
    let Some(rx) = &state.load_rx else { return; };
    match rx.try_recv() {
        Ok(LoadMsg::Loaded { tree }) => {
            state.status = format!("로드 완료: {} 엔트리(루트)", tree.root.entries.len());
            state.archive = state.shared_archive.lock().unwrap().clone();
            state.tree = Some(tree);
            state.loading = false;
            state.load_rx = None;
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

// ── 트리 뷰어 ─────────────────────────────────────────────────────────

fn render_tree_panel(ui: &mut egui::Ui, state: &mut AppState) {
    if state.tree.is_none() {
        ui.label("WZ 파일을 열어 주세요.");
        return;
    }
    let tree = state.tree.clone().unwrap();
    let archive = state.shared_archive.lock().unwrap().clone();

    // 보이는 행 계산 (DFS, 확장 상태 반영)
    let mut rows: Vec<TreeRow> = Vec::new();
    build_rows(&mut rows, &tree, archive.as_deref(), state, 0, "", "");
    // 검색 필터
    if !state.search.is_empty() {
        let q = state.search.to_lowercase();
        rows.retain(|r| r.name.to_lowercase().contains(&q));
    }

    let row_height = 18.0;
    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show_rows(ui, row_height, rows.len(), |ui, range| {
            for row in &rows[range] {
                render_row(ui, state, row);
            }
        });
}

fn build_rows(
    rows: &mut Vec<TreeRow>,
    tree: &DirTree,
    archive: Option<&WzArchive>,
    state: &mut AppState,
    depth: usize,
    parent_path: &str,
    _parent_block: &str,
) {
    for (i, e) in tree.root.entries.iter().enumerate() {
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
            offset: e.offset,
            data_len: e.data_len,
            cs32: e.cs32,
        });

        if expanded {
            // 디렉터리: DirTree 에는 하위 블록이 별도로 있다 — CLI 는 blocks 리스트를
            // 쓴다. PoC 1 에서는 단일 블록만 보여준다 (단순화).
            if e.node_type == nt::IMAGE {
                // 이미지 파싱 (캐시)
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
                            offset: None,
                            data_len: 0,
                            cs32: 0,
                        });
                    }
                }
            }
        }
        let _ = i;
    }
    // blocks (하위 디렉터리) 는 PoC 1 에서 생략 — 단일 루트만 표시.
}

fn parse_image_children(archive: Option<&WzArchive>, e: &RawEntry) -> Vec<(String, String)> {
    let Some(arch) = archive else { return vec![]; };
    let Some(off) = e.offset else { return vec![]; };
    let Ok(node) = arch.parse_image_at(off) else { return vec![]; };
    let mut out = Vec::new();
    // root Property 의 children
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

fn value_kind(n: &ImageNode) -> String {
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

fn render_row(ui: &mut egui::Ui, state: &mut AppState, row: &TreeRow) {
    ui.horizontal(|ui| {
        // 들여쓰기
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
            // 이미지면 뷰어 갱신
            if row.kind == "img" {
                load_image_view(state, &row.path);
            }
        }
    });
}

// ── 이미지 뷰어 ───────────────────────────────────────────────────────

fn load_image_view(state: &mut AppState, path: &str) {
    if state.image_textures.contains_key(path) {
        state.current_image_path = Some(path.to_string());
        return;
    }
    // 경로에서 이미지 엔트리를 찾아 파싱
    let Some(tree) = &state.tree else { return; };
    let archive = state.shared_archive.lock().unwrap().clone();
    let Some(archive) = archive else { return; };
    // 경로에서 이름 추출 (마지막 세그먼트)
    let name = path.split('/').filter(|s| !s.is_empty()).last().unwrap_or("");
    // 트리에서 해당 엔트리 찾기
    let entry = find_entry(tree, name);
    let Some(e) = entry else {
        state.current_image_info = Some(format!("엔트리를 찾을 수 없음: {name}"));
        return;
    };
    let Some(off) = e.offset else {
        state.current_image_info = Some("오프셋 없음".into());
        return;
    };
    let Ok(node) = archive.parse_image_at(off) else {
        state.current_image_info = Some("이미지 파싱 실패".into());
        return;
    };
    // 첫 캔버스 찾기
    let png_ref = extract_png_ref(&node);
    let Some(pr) = png_ref else {
        state.current_image_info = Some("캔버스 없음".into());
        state.current_image_path = Some(path.to_string());
        return;
    };
    // 디코딩
    // PoC 1 은 메타데이터만 표시한다. 실제 픽셀 디코딩은 PoC 2(맵 뷰포트)에서
    // archive 의 raw texels 를 읽어 decode_page_to_rgba8 로 처리한다.
    state.current_image_info = Some(format!(
        "canvas: {}x{} format={} scale={} pages={}",
        pr.width, pr.height, pr.format, pr.scale, pr.pages
    ));
    state.current_image_path = Some(path.to_string());
}


fn load_avatar_body(state: &mut AppState) {
    // Character.wz 에서 첫 번째 Body 이미지를 찾아 디코딩한다.
    let char_root = "<WZ_DATA_ROOT>/Character";
    let char_wz = std::path::PathBuf::from(char_root).join("Character.wz");
    let Ok(arch) = wz_core::WzArchive::open(&char_wz) else {
        state.avatar_status = "Character.wz 열기 실패".into();
        return;
    };
    let Ok(tree) = arch.read_tree() else {
        state.avatar_status = "트리 읽기 실패".into();
        return;
    };
    // Body 디렉터리 찾기
    let body_entry = tree.root.entries.iter().find(|e| e.name.as_deref() == Some("Body"));
    let Some(_body_entry) = body_entry else {
        state.avatar_status = "Body 디렉터리 없음".into();
        return;
    };
    // Body 디렉터리의 첫 이미지 찾기 (blocks 커서: Body 는 몇 번째 디렉터리인가)
    let body_idx = tree.root.entries.iter().position(|e| e.name.as_deref() == Some("Body")).unwrap();
    let mut cursor = 0usize;
    for e in &tree.root.entries[..body_idx] {
        if e.node_type == wz_core::dirtree::node_type::DIRECTORY { cursor += 1; }
    }
    let Some(sub) = tree.blocks.get(cursor) else {
        state.avatar_status = "Body 하위 블록 없음".into();
        return;
    };
    let first_img = sub.entries.iter().find(|e| e.node_type == wz_core::dirtree::node_type::IMAGE);
    let Some(img_entry) = first_img else {
        state.avatar_status = "Body 이미지 없음".into();
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
    // 첫 PngRef 찾기
    fn find_png(n: &wz_core::image::Node) -> Option<wz_core::image::PngRef> {
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
    let file_src = match wz_pool::FileSource::open(&char_wz) { Ok(f) => f, Err(_) => { state.avatar_status = "파일 열기 실패".into(); return; } };
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


fn find_data_root(_map_path: &str) -> Option<std::path::PathBuf> {
    // GUI 는 archive 경로를 아직 보관하지 않는다. 일반적인 설치 경로를 시도한다.
    let candidates = [
        "<WZ_DATA_ROOT>",
    ];
    for c in candidates {
        let p = std::path::PathBuf::from(c);
        if p.join("Map").exists() { return Some(p); }
    }
    None
}

fn load_back_image(data_root: &std::path::Path, b_s: &str) -> Option<(u32, u32, Vec<u8>)> {
    use wz_core::WzArchive;
use wz_core::image::ImageValue;
    use wz_image::decoder::decode_raw_page_to_rgba8;
    use wz_image::TextureFormat;

    // Back/_Canvas/_Canvas_*.wz 중 b_s.img 를 포함한 파일을 찾는다.
    let canvas_dir = data_root.join("Map").join("Back").join("_Canvas");
    let entries = std::fs::read_dir(&canvas_dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wz") { continue; }
        let Ok(arch) = WzArchive::open(&path) else { continue; };
        let Ok(tree) = arch.read_tree() else { continue; };
        // 루트 엔트리에서 {b_s}.img 또는 {b_s} 찾기
        let name_img = format!("{}.img", b_s);
        let e = tree.root.entries.iter().find(|e| {
            e.name.as_deref() == Some(b_s) || e.name.as_deref() == Some(name_img.as_str())
        });
        let Some(e) = e else { continue; };
        let Some(off) = e.offset else { continue; };
        let Ok(node) = arch.parse_image_at(off) else { continue; };
        // 첫 PngRef 찾기
        fn find_png(n: &wz_core::image::Node) -> Option<wz_core::image::PngRef> {
            if let Some(ImageValue::Png(p)) = &n.value { return Some(p.clone()); }
            for c in &n.children {
                if let Some(p) = find_png(c) { return Some(p); }
            }
            if let Some(ImageValue::Property(p)) = &n.value {
                if let Some(res) = find_png(p) { return Some(res); }
            }
            None
        }
        let Some(pr) = find_png(&node) else { continue; };
        let file_src = wz_pool::FileSource::open(&path).ok()?;
        let src: std::sync::Arc<dyn wz_pool::ByteSource> = std::sync::Arc::new(file_src);
        let (w, h, rgba) = wz_image::decode_png_ref_to_rgba(&pr, &src).ok()?;
        return Some((w, h, rgba));
    }
    None
}


fn load_map_by_path(state: &mut AppState) {
    let path_for_data = state.map_path.clone();
    let Some(tree) = &state.tree else {
        state.map_status = "WZ 파일을 먼저 열어 주세요".into();
        return;
    };
    let Some(archive) = state.shared_archive.lock().unwrap().clone() else {
        state.map_status = "아카이브 없음".into();
        return;
    };
    let path = state.map_path.clone();
    let Some(e) = find_entry_by_path(tree, &path) else {
        state.map_status = format!("엔트리 없음: {path}");
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

    // 데이터 루트 추정 (Map WZ 경로에서 "Data" 디렉터리까지 거슬러 올라간다).
    let data_root = find_data_root(&path_for_data);
    state.data_root = data_root.clone();

    // 첫 번째 백그라운드 이미지 로드 (PoC 2.6: Back WZ → 디코딩 → 텍스처).
    let back_img = if let (Some(root), Some(first)) = (data_root.as_ref(), map.backgrounds.first()) {
        load_back_image(root, &first.b_s)
    } else {
        None
    };
    state.map_back_image = back_img;

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


fn load_map_from_selection(state: &mut AppState) {
    let Some(sel) = state.selected.clone() else { return; };
    let Some(tree) = &state.tree else { return; };
    let Some(archive) = state.shared_archive.lock().unwrap().clone() else { return; };
    let name = sel.split('/').filter(|s| !s.is_empty()).last().unwrap_or("");
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
    state.viewport_dirty = true;
}



/// 경로("Map0/000010000" 또는 "Map0/000010000.img")로 DirTree 엔트리를 찾는다.
/// 디렉터리 하위 블록은 build_arena 와 동일한 커서 규칙으로 추적한다.
fn find_entry_by_path(tree: &DirTree, path: &str) -> Option<RawEntry> {
    let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    if segs.is_empty() { return None; }
    let mut cursor = 0usize;
    let mut block = &tree.root;
    for (i, seg) in segs.iter().enumerate() {
        // 현재 블록에서 세그먼트 찾기 (.img 유무 모두 시도)
        let idx = block.entries.iter().position(|e| {
            e.name.as_deref() == Some(seg)
                || e.name.as_deref() == Some(&format!("{}.img", seg))
        })?;
        // 대상 이전의 디렉터리 엔트리 수만큼 커서를 전진시킨다
        // (build_arena 와 동일한 DFS 커서 규칙).
        for e in &block.entries[..idx] {
            if e.node_type == nt::DIRECTORY {
                cursor += 1;
            }
        }
        let entry = block.entries[idx].clone();
        if i == segs.len() - 1 {
            return Some(entry);
        }
        if entry.node_type == nt::DIRECTORY {
            let sub = tree.blocks.get(cursor)?;
            cursor += 1;
            block = sub;
        } else {
            return None;
        }
    }
    None
}


fn find_entry(tree: &DirTree, name: &str) -> Option<RawEntry> {
    tree.root.entries.iter().find(|e| e.name.as_deref() == Some(name)).cloned()
}

fn extract_png_ref(node: &ImageNode) -> Option<wz_core::image::PngRef> {
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

// ── 인스펙터 ──────────────────────────────────────────────────────────

fn render_inspector(ui: &mut egui::Ui, state: &mut AppState) {
    ui.heading("인스펙터");
    if let Some(sel) = &state.selected.clone() {
        ui.label(format!("선택: {sel}"));
        // 선택된 엔트리의 메타데이터 찾기
        if let Some(tree) = &state.tree {
            let name = sel.split('/').filter(|s| !s.is_empty()).last().unwrap_or("");
            if let Some(e) = tree.root.entries.iter().find(|e| e.name.as_deref() == Some(name)) {
                ui.label(format!("타입: {}", e.node_type));
                ui.label(format!("data_len: {}", e.data_len));
                ui.label(format!("cs32: {}", e.cs32));
                ui.label(format!("file_pos: {}", e.file_pos));
                ui.label(format!("offset: {:?}", e.offset));
            }
        }
    } else {
        ui.label("노드를 선택하세요.");
    }

    ui.separator();
    ui.heading("이미지 뷰어");
    if let Some(path) = &state.current_image_path {
        ui.label(format!("이미지: {path}"));
        if let Some(info) = &state.current_image_info {
            ui.label(info);
        }
    } else {
        ui.label("이미지 노드를 선택하세요.");
    }

    ui.separator();
    ui.heading("맵 (PoC 2.5)");
    ui.horizontal(|ui| {
        ui.label("맵 경로:");
        ui.text_edit_singleline(&mut state.map_path);
        if ui.button("Load Map").clicked() {
            load_map_by_path(state);
        }
    });
    if ui.button("선택 노드를 맵으로 로드").clicked() {
        load_map_from_selection(state);
    }
    ui.label(&state.map_status);
    if let Some(m) = &state.current_map {
        ui.label(format!("bgm: {:?}", m.info.bgm));
        ui.label(format!("town/swim/fly: {} / {} / {}", m.info.town, m.info.swim, m.info.fly));
        if let Some(vr) = m.info.vrect {
            ui.label(format!("vrect: {:?}", vr));
        }
    }
}

// ── 맵 뷰포트 (PoC 2) ────────────────────────────────────────────────

fn render_viewport(ui: &mut egui::Ui, state: &mut AppState) {
    // 마우스 입력 처리: 드래그=팬, 스크롤=줌
    let resp = ui.allocate_response(
        egui::vec2(state.viewport_size.0 as f32, state.viewport_size.1 as f32),
        egui::Sense::click_and_drag(),
    );
    if resp.dragged() {
        if let Some(pos) = resp.interact_pointer_pos() {
            if let Some(last) = state.viewport_last_pos {
                let dx = pos.x - last.x;
                let dy = pos.y - last.y;
                // 카메라 팬: 화면 이동을 월드 이동으로 변환 (줌 반영)
                state.camera.center.x -= dx / state.camera.zoom;
                state.camera.center.y -= dy / state.camera.zoom;
                state.viewport_dirty = true;
            }
            state.viewport_last_pos = Some(pos);
            state.viewport_dragging = true;
        }
    } else {
        state.viewport_dragging = false;
        state.viewport_last_pos = None;
    }
    if resp.hovered() {
        let scroll = ui.input(|i| i.smooth_scroll_delta.y);
        if scroll != 0.0 {
            let factor = if scroll > 0.0 { 1.1 } else { 1.0 / 1.1 };
            state.camera.zoom = (state.camera.zoom * factor).clamp(0.1, 10.0);
            state.viewport_dirty = true;
        }
    }

    // 더티면 오프스크린 재렌더
    if state.viewport_dirty {
        state.viewport_dirty = false;
        let (w, h) = state.viewport_size;
        let mut rc = match wz_render::RenderContext::new_headless() {
            Ok(r) => r,
            Err(_) => {
                state.current_image_info = Some("RenderContext 생성 실패".into());
                return;
            }
        };
        // 1x1 흰색 아틀라스
        let mut packer = AtlasShelfPacker::new(64, 64);
        let white = vec![255u8; 64 * 64 * 4];
        let slot = packer.allocate_and_write(1, 1, &[255, 255, 255, 255]).unwrap();
        let atlas_key = slot.key;
        let atlas_w = 64u32;
        let atlas_h = 64u32;
        for page in packer.pages() {
            let _ = rc.create_texture(page.key, page.width, page.height, &page.pixels);
        }
        let _ = white;

        // 데모 그리드 또는 맵 백그라운드 이미지
        let mut batch = SpriteBatch::new();
        if let Some((bw, bh, bpx)) = &state.map_back_image {
            // 백그라운드 이미지를 별도 텍스처로 업로드
            let back_key = GpuKey(2);
            let _ = rc.create_texture(back_key, *bw, *bh, bpx);
            batch.push_quad(back_key, [0.0, 0.0, *bw as f32, *bh as f32], 0.0,
                [0.0, 0.0, 1.0, 1.0], [1.0, 1.0, 1.0, 1.0]);
        } else {
            for gx in -2..=2i32 {
                for gy in -2..=2i32 {
                    let color = [
                        0.3 + (gx as f32 + 2.0) * 0.12,
                        0.3 + (gy as f32 + 2.0) * 0.12,
                        0.6,
                        1.0,
                    ];
                    let x = gx as f32 * 40.0 - 20.0;
                    let y = gy as f32 * 40.0 - 20.0;
                    batch.push_quad(atlas_key, [x, y, 36.0, 36.0], 0.0,
                        slot.rect.uv(atlas_w, atlas_h), color);
                }
            }
        }
        batch.finish();

        let uniforms = state.camera.build_uniforms(0.0, 0.0);
        if let Ok(rgba) = rc.render_to_rgba(w, h, &batch, &uniforms, [0.08, 0.08, 0.12, 1.0]) {
            let ci = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);
            state.viewport_texture = Some(ui.ctx().load_texture("viewport", ci, egui::TextureOptions::LINEAR));
        }
    }

    if let Some(tex) = &state.viewport_texture {
        ui.add(egui::Image::new(tex).max_width(state.viewport_size.0 as f32));
    } else {
        ui.label("(뷰포트 렌더링 대기)");
    }

    // 맵이 로드됐으면 발판을 뷰포트 위에 선으로 그린다.
    if let Some(m) = &state.current_map {
        let painter = ui.painter();
        let rect = resp.rect;
        for fh in &m.footholds {
            let s1 = state.camera.world_to_screen(Vec2::new(fh.x1 as f32, fh.y1 as f32));
            let s2 = state.camera.world_to_screen(Vec2::new(fh.x2 as f32, fh.y2 as f32));
            let p1 = egui::pos2(rect.min.x + s1.x, rect.min.y + s1.y);
            let p2 = egui::pos2(rect.min.x + s2.x, rect.min.y + s2.y);
            painter.line_segment([p1, p2], egui::Stroke::new(1.5, egui::Color32::GREEN));
        }
        ui.label(format!("발판 {}개 표시", m.footholds.len()));
    }

    ui.label(format!("카메라: pos=({:.1},{:.1}) zoom={:.2}  (드래그=팬, 휠=줌)",
        state.camera.center.x, state.camera.center.y, state.camera.zoom));
}
// ── 아바타 패널 (PoC 3) ────────────────────────────────────────────────

const AVATAR_SLOTS: [&str; 29] = [
    "Body", "Head", "Face", "Hair", "Cap", "Longcoat", "Pants", "Shoes",
    "Glove", "Cape", "Shield", "Weapon", "SubWeapon", "Earrings", "EyeAccessory",
    "FaceAccessory", "Pendant", "Belt", "Medal", "Ring1", "Ring2", "Ring3",
    "Ring4", "Pet", "TamingMob", "Mechanic", "Android", "Effect", "Extra",
];

fn render_avatar_panel(ui: &mut egui::Ui, state: &mut AppState) {
    ui.label("29개 슬롯 (PoC 3.5 — Body 이미지 로딩)");
    egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
        for (i, slot) in AVATAR_SLOTS.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{:>2}. {}", i, slot));
                ui.label(egui::RichText::new("(empty)").weak());
            });
        }
    });
    if ui.button("Body 이미지 로드 (Character.wz)").clicked() {
        load_avatar_body(state);
    }
    ui.label(&state.avatar_status);
    if let Some((w, h, rgba)) = &state.avatar_image {
        let ci = egui::ColorImage::from_rgba_unmultiplied([*w as usize, *h as usize], rgba);
        let tex = ui.ctx().load_texture("avatar_body", ci, egui::TextureOptions::LINEAR);
        ui.add(egui::Image::new(&tex).max_width(200.0));
        ui.label(format!("Body: {}x{}", w, h));
    }
}


// ── main ──────────────────────────────────────────────────────────────

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 700.0])
            .with_title("WzComparerR2-Rust GUI PoC 1"),
        ..Default::default()
    };
    eframe::run_native(
        "wz-gui",
        options,
        Box::new(|_cc| Ok(Box::new(PocApp::default()))),
    )
}
