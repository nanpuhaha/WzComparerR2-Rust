//! 월드맵 (`Map/WorldMap/<name>.img`) 파싱/표시.
//!
//! 원본 `UIWorldMap` 의 데이터 모델 중 BaseImg + MapList 중심의 읽기 전용 뷰.

#[derive(Debug, Clone, Default)]
pub struct WorldMapSpot {
    pub x: f32,
    pub y: f32,
    pub spot_type: i32,
    pub title: Option<String>,
    pub desc: Option<String>,
    pub map_nos: Vec<i32>,
}

#[derive(Debug, Clone, Default)]
pub struct WorldMapQuest {
    pub name: String,
    pub base: Option<(u32, u32, Vec<u8>)>,
    pub open_maps: Vec<i32>,
    pub close_maps: Vec<i32>,
    pub is_default: bool,
    pub quest: i32,
    pub state: i32,
}

#[derive(Debug, Clone, Default)]
pub struct WorldMapFog {
    pub name: String,
    pub image: Option<(u32, u32, Vec<u8>)>,
    pub quest: i32,
    pub state: i32,
}

#[derive(Debug, Clone, Default)]
pub struct WorldMapView {
    pub name: String,
    pub display_name: String,
    pub base: Option<(u32, u32, Vec<u8>)>,
    /// 툴팁/원점 보정용 (원본 BaseImg origin).
    #[allow(dead_code)]
    pub base_origin: (i32, i32),
    pub base_spot: (i32, i32),
    pub spots: Vec<WorldMapSpot>,
    pub quests: Vec<WorldMapQuest>,
    pub fogs: Vec<WorldMapFog>,
}

/// `WorldMap` 디렉터리의 이미지 이름 목록.
pub fn list_worldmaps(root: &std::path::Path) -> Vec<String> {
    let base = root.join("Map").join("WorldMap");
    let mut names = Vec::new();
    for entry in std::fs::read_dir(base).ok().into_iter().flatten().flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wz") {
            continue;
        }
        let Ok(archive) = wz_core::WzArchive::open(&path) else {
            continue;
        };
        let Ok(tree) = archive.read_tree() else {
            continue;
        };
        for entry in &tree.root.entries {
            if entry.node_type == 4 {
                if let Some(name) = &entry.name {
                    if name.ends_with(".img") {
                        names.push(name.trim_end_matches(".img").to_string());
                    }
                }
            }
        }
    }
    names.sort();
    names.dedup();
    names
}
