// wz-gui — application state (single source of truth).
//
// `docs/gui-architecture.md` §6.1: 모든 패널이 공유하는 `AppState`.
// UI 스레드는 이 구조체를 직접 읽고 쓰지만, 블로킹 I/O 는 하지 않는다
// (async_io.rs 의 워커 스레드가 결과를 채널로 보낸다).

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};

use egui::TextureHandle;
use wz_core::dirtree::DirTree;
use wz_core::WzArchive;
use wz_db::MapData;
use wz_render::camera::Camera2D;
use wz_render::GpuKey;

/// 맵 이미지 로딩 결과 (워커 스레드 → UI 스레드).
pub struct MapImages {
    pub tiles: Vec<(i32, i32, u32, u32, Vec<u8>)>,
    pub objs: Vec<(i32, i32, u32, u32, Vec<u8>)>,
}

// ── 로딩 메시지 ───────────────────────────────────────────────────────

pub enum LoadMsg {
    Loaded { tree: DirTree },
    Failed(String),
}

// ── 트리 행 ───────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct TreeRow {
    pub depth: usize,
    pub name: String,
    pub kind: String,
    pub path: String,
    pub expandable: bool,
    pub expanded: bool,
}

// ── 앱 상태 ───────────────────────────────────────────────────────────

pub struct AppState {
    pub archive: Option<Arc<WzArchive>>,
    pub archive_path: Option<std::path::PathBuf>,
    pub shared_archive: Arc<Mutex<Option<Arc<WzArchive>>>>,
    pub tree: Option<DirTree>,
    pub image_children: HashMap<String, Vec<(String, String)>>,
    pub expanded: HashSet<String>,
    pub selected: Option<String>,
    pub image_textures: HashMap<String, TextureHandle>,
    pub current_image_path: Option<String>,
    pub current_image_info: Option<String>,
    pub map_path: String,
    pub current_map: Option<MapData>,
    pub map_back_image: Option<(u32, u32, Vec<u8>)>,
    pub data_root: Option<std::path::PathBuf>,
    pub string_linker: Option<wz_db::StringLinker>,
    pub string_linker_status: String,
    pub map_tile_images: Vec<(i32, i32, u32, u32, Vec<u8>)>,
    pub map_obj_images: Vec<(i32, i32, u32, u32, Vec<u8>)>,
    pub map_status: String,
    pub map_images_rx: Option<crossbeam_channel::Receiver<MapImages>>,
    pub avatar_image: Option<(u32, u32, Vec<u8>)>,
    pub avatar_status: String,
    pub avatar_rendered: Option<(u32, u32, Vec<u8>)>,
    pub avatar_selected_slots: std::collections::HashSet<usize>,
    pub avatar_slot_selection: [usize; 29],
    pub avatar_loaded_parts: Vec<(String, u32, u32, Vec<u8>)>,
    pub avatar_equipment_input: String,
    pub avatar_body_skin: i32,
    pub camera: Camera2D,
    pub viewport_texture: Option<TextureHandle>,
    pub viewport_dirty: bool,
    /// 맵 데이터가 바뀌었을 때만 true. 카메라만 바뀔 때는 false.
    pub viewport_map_dirty: bool,
    /// 캐시된 레이어 쿼드 (key, [x,y,w,h], z). 맵 데이터가 바뀔 때만 재구축.
    pub viewport_cached_layers: Vec<(GpuKey, [f32; 4], f32)>,
    pub viewport_cached_has_back: bool,
    pub viewport_size: (u32, u32),
    pub viewport_dragging: bool,
    pub viewport_last_pos: Option<egui::Pos2>,
    pub loading: bool,
    pub load_rx: Option<crossbeam_channel::Receiver<LoadMsg>>,
    pub status: String,
    pub search: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            archive: None,
            archive_path: None,
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
            string_linker: None,
            string_linker_status: "StringLinker 미로드".into(),
            map_tile_images: Vec::new(),
            map_obj_images: Vec::new(),
            map_status: "맵 ID 를 입력하고 'Load Map'".into(),
            map_images_rx: None,
            avatar_image: None,
            avatar_status: "Body 이미지를 로드하세요".into(),
            avatar_rendered: None,
            avatar_selected_slots: std::collections::HashSet::new(),
            avatar_slot_selection: [usize::MAX; 29],
            avatar_loaded_parts: Vec::new(),
            avatar_equipment_input: String::new(),
            avatar_body_skin: 2000,
            camera: Camera2D::new(512.0, 512.0),
            viewport_texture: None,
            viewport_dirty: true,
            viewport_map_dirty: true,
            viewport_cached_layers: Vec::new(),
            viewport_cached_has_back: false,
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
