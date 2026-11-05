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

/// GUI 렌더용 아바타 파트 (로더의 `AvatarPart` + 최종 좌표/z).
#[derive(Clone, Default)]
pub struct AvatarPartData {
    pub slot: String,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub position: (i32, i32),
    pub z_index: i32,
}

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
    pub avatar_loaded_parts: Vec<AvatarPartData>,
    pub avatar_equipment_input: String,
    pub avatar_body_skin: i32,
    /// 프리즘 (C# `PrismData`) — 호출자/UI 공급. WZ에는 저장되지 않는다.
    pub prism_enabled: bool,
    pub prism_color_type: i32,
    pub prism_hue: i32,
    pub prism_saturation: i32,
    pub prism_brightness: i32,
    pub prism_convert_pure_black: bool,
    /// Hair/Face 믹스 컬러 (C# `MixColor`/`MixOpacity`).
    pub hair_mix_enabled: bool,
    pub hair_mix_color: i32,
    pub hair_mix_opacity: i32,
    pub face_mix_enabled: bool,
    pub face_mix_color: i32,
    pub face_mix_opacity: i32,
    /// Effect 슬롯의 정적 프레임 인덱스.
    pub effect_frame: usize,
    /// `name:x,y;...` 형식의 커스텀 오리진 오버라이드.
    pub custom_origin_input: String,
    /// Effect 전체 프레임과 delay (자동 재생).
    pub effect_frames: Vec<(u32, u32, Vec<u8>)>,
    pub effect_delays: Vec<u32>,
    /// 프레임별 origin (`effect_frames` 와 같은 순서).
    pub effect_origins: Vec<(i32, i32)>,
    /// 현재 프레임의 origin — 렌더 시 effect 쿼드 위치에 반영한다.
    pub effect_origin: (i32, i32),
    pub effect_auto_play: bool,
    pub effect_last_time: f64,
    /// 체어 애니메이션 (rollingHouseChair 등).
    pub chair_frames: Vec<Vec<AvatarPartData>>,
    pub chair_delays: Vec<u32>,
    pub chair_frame: usize,
    pub chair_auto_play: bool,
    pub chair_last_time: f64,
    /// 체어 customChair 타입/이벤트 메타데이터 표시용.
    pub chair_type: Option<String>,
    pub chair_event_point: Option<(i32, i32, i32)>,
    /// `customChair` 의 평탄화된 메타데이터 (모든 UI 타입 표시용).
    pub chair_custom_fields: Vec<(String, String)>,
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
            prism_enabled: false,
            prism_color_type: 0,
            prism_hue: 0,
            prism_saturation: 100,
            prism_brightness: 100,
            prism_convert_pure_black: false,
            hair_mix_enabled: false,
            hair_mix_color: 0,
            hair_mix_opacity: 100,
            face_mix_enabled: false,
            face_mix_color: 0,
            face_mix_opacity: 100,
            effect_frame: 0,
            custom_origin_input: String::new(),
            effect_frames: Vec::new(),
            effect_delays: Vec::new(),
            effect_origins: Vec::new(),
            effect_origin: (0, 0),
            effect_auto_play: false,
            effect_last_time: 0.0,
            chair_frames: Vec::new(),
            chair_delays: Vec::new(),
            chair_frame: 0,
            chair_auto_play: false,
            chair_last_time: 0.0,
            chair_type: None,
            chair_event_point: None,
            chair_custom_fields: Vec::new(),
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
