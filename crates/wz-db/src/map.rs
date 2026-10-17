//! 메이플스토리 맵 씬 데이터 및 발판(Foothold) 계층 구조 파서 (`MapData.cs`, `FootholdManager.cs` 포트).
//!
//! 원본 `WzComparerR2.MapRender/MapData.cs` 및 `FootholdManager.cs`, `Patches2/*.cs` 대응.

use std::collections::HashMap;
use wz_core::image::Node;

/// 맵 가상 뷰포트 사각형 (VR: Virtual Rectangle).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MapVirtualRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// 맵 기본 메타데이터 (`info` 노드).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapInfo {
    pub map_id: Option<i32>,
    pub vrect: Option<MapVirtualRect>,
    pub bgm: Option<String>,
    pub map_mark: Option<String>,
    pub return_map: Option<i32>,
    pub forced_return: Option<i32>,
    pub town: bool,
    pub swim: bool,
    pub fly: bool,
}

/// 발판 (Foothold) 선분 데이터.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Foothold {
    pub id: i32,
    pub layer: usize,
    pub group: usize,
    pub x1: i32,
    pub y1: i32,
    pub x2: i32,
    pub y2: i32,
    pub prev: i32,
    pub next: i32,
    pub piece: i32,
}

impl Foothold {
    /// 수직 벽 여부 (`x1 >= x2`).
    pub fn is_wall(&self) -> bool {
        self.x1 >= self.x2
    }

    /// 수평 발판 여부 (`y1 == y2`).
    pub fn is_flat(&self) -> bool {
        self.y1 == self.y2
    }
}

/// 사다리 / 로프 종류.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LadderRopeKind {
    /// 사다리 (WZ `l = 1`).
    Ladder,
    /// 로프 (WZ `l = 0`).
    Rope,
}

/// 사다리 / 로프 데이터.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LadderRope {
    pub id: i32,
    pub x: i32,
    pub y1: i32,
    pub y2: i32,
    /// 사다리/로프 구분 플래그 (true: ladder, false: rope). WZ 속성명 `l`.
    pub l: bool,
    pub uf: i32,
    pub page: i32,
}

impl LadderRope {
    /// 사다리 또는 로프 종류를 반환한다.
    #[inline]
    pub fn kind(&self) -> LadderRopeKind {
        if self.l {
            LadderRopeKind::Ladder
        } else {
            LadderRopeKind::Rope
        }
    }
}

/// 포탈 데이터.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapPortal {
    pub id: i32,
    pub p_name: String,
    pub p_type: i32,
    pub x: i32,
    pub y: i32,
    pub to_map: Option<i32>,
    pub to_name: Option<String>,
    pub script: Option<String>,
}

/// 라이프 (NPC / Mob) 종류.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifeKind {
    /// NPC (WZ `type = "n"`).
    Npc,
    /// 몬스터 (WZ `type = "m"`).
    Mob,
    /// 기타 / 미분류.
    Unknown,
}

/// 라이프 (NPC / Mob) 배치 데이터.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapLife {
    pub id: String,
    /// WZ 원시 라이프 타입 문자열 ("n": NPC, "m": Mob).
    pub life_type: String,
    pub x: i32,
    pub y: i32,
    pub cy: i32,
    pub fh: i32,
    pub flip: bool,
    pub hide: bool,
}

impl MapLife {
    /// 라이프 종류(NPC, Mob 등)를 반환한다.
    #[inline]
    pub fn kind(&self) -> LifeKind {
        match self.life_type.as_str() {
            "n" | "N" => LifeKind::Npc,
            "m" | "M" => LifeKind::Mob,
            _ => LifeKind::Unknown,
        }
    }
}

/// 맵 배경 아이템 (`back/{id}`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BackItem {
    pub index: usize,
    /// 배경 세트 이름 (WZ 속성명 `bS` / background set).
    pub b_s: String,
    pub ani: i32,
    pub no: String,
    pub x: i32,
    pub y: i32,
    pub rx: i32,
    pub ry: i32,
    pub cx: i32,
    pub cy: i32,
    pub tile_mode: i32,
    pub flip: bool,
    pub front: bool,
    pub alpha: i32,
}

impl BackItem {
    /// 배경 세트 이름 (`bS` 속성)을 반환한다.
    #[inline]
    pub fn bg_set(&self) -> &str {
        &self.b_s
    }
}

/// 맵 오브젝트 아이템 (`{layer}/obj/{id}`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ObjItem {
    pub layer: usize,
    pub index: usize,
    /// 오브젝트 세트 이름 (WZ 속성명 `oS` / object set).
    pub o_s: String,
    pub l0: String,
    pub l1: String,
    pub l2: String,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub flip: bool,
}

impl ObjItem {
    /// 오브젝트 세트 이름 (`oS` 속성)을 반환한다.
    #[inline]
    pub fn obj_set(&self) -> &str {
        &self.o_s
    }
}

/// 맵 타일 아이템 (`{layer}/tile/{id}`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TileItem {
    pub layer: usize,
    pub index: usize,
    /// 타일 세트 이름 (WZ 레이어 info 속성명 `tS` / tile set).
    pub t_s: String,
    pub u: String,
    pub no: String,
    pub x: i32,
    pub y: i32,
}

impl TileItem {
    /// 타일 세트 이름 (`tS` 속성)을 반환한다.
    #[inline]
    pub fn tile_set(&self) -> &str {
        &self.t_s
    }
}

/// 발판 그룹 및 전체 발판 트리 (`FootholdManager.cs:20-53` 대응).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FootholdTree {
    pub layers: Vec<Vec<Foothold>>,
    pub bounds: Option<MapVirtualRect>,
}

impl FootholdTree {
    /// 발판 리스트로부터 계층형 FootholdTree 및 바운딩 사각형을 생성한다.
    pub fn build(footholds: &[Foothold]) -> Self {
        let mut layers = vec![Vec::new(); 8];
        let mut min_x = i32::MAX;
        let mut max_x = i32::MIN;
        let mut min_y = i32::MAX;
        let mut max_y = i32::MIN;
        let mut has_fh = false;

        for fh in footholds {
            if fh.layer < 8 {
                layers[fh.layer].push(fh.clone());
            }
            min_x = min_x.min(fh.x1.min(fh.x2));
            max_x = max_x.max(fh.x1.max(fh.x2));
            min_y = min_y.min(fh.y1.min(fh.y2));
            max_y = max_y.max(fh.y1.max(fh.y2));
            has_fh = true;
        }

        let bounds = if has_fh {
            Some(MapVirtualRect {
                left: min_x,
                top: min_y,
                right: max_x,
                bottom: max_y,
            })
        } else {
            None
        };

        Self { layers, bounds }
    }

    /// ID로 발판을 검색한다.
    pub fn get_by_id(&self, id: i32) -> Option<&Foothold> {
        for layer in &self.layers {
            for fh in layer {
                if fh.id == id {
                    return Some(fh);
                }
            }
        }
        None
    }
}

/// 전체 맵 물리 씬 데이터 컨테이너 (`MapData`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapData {
    pub info: MapInfo,
    pub footholds: Vec<Foothold>,
    pub footholds_by_id: HashMap<i32, Foothold>,
    pub foothold_tree: FootholdTree,
    pub ladders_ropes: Vec<LadderRope>,
    pub portals: Vec<MapPortal>,
    pub life: Vec<MapLife>,
    pub backgrounds: Vec<BackItem>,
    pub objects: Vec<ObjItem>,
    pub tiles: Vec<TileItem>,
}

impl MapData {
    /// Arena 노드 트리에서 맵 데이터를 파싱한다 (`ADR-2 Arena 일원화`).
    pub fn parse_from_arena(arena: &crate::Arena, root: crate::NodeId) -> Self {
        let mut data = Self::default();

        if let Some(info_id) = arena.find_child(root, "info") {
            data.info = parse_map_info_arena(arena, info_id);
        }

        if let Some(fh_id) = arena.find_child(root, "foothold") {
            parse_footholds_arena(arena, fh_id, &mut data);
        }

        if let Some(lr_id) = arena.find_child(root, "ladderRope") {
            parse_ladder_rope_arena(arena, lr_id, &mut data);
        }

        if let Some(p_id) = arena.find_child(root, "portal") {
            parse_portals_arena(arena, p_id, &mut data);
        }

        if let Some(life_id) = arena.find_child(root, "life") {
            parse_life_arena(arena, life_id, &mut data);
        }

        if let Some(back_id) = arena.find_child(root, "back") {
            parse_back_arena(arena, back_id, &mut data);
        }

        for layer in 0..8 {
            let layer_name = layer.to_string();
            if let Some(layer_id) = arena.find_child(root, &layer_name) {
                parse_layer_arena(arena, layer_id, layer, &mut data);
            }
        }

        data.foothold_tree = FootholdTree::build(&data.footholds);
        data
    }

    /// WZ 맵 노드 트리(`Map/Map/MapX/XXXXXXXXX.img` 루트)에서 맵 데이터를 파싱한다.
    pub fn parse_from_node(root: &Node) -> Self {
        let (arena, arena_root) = crate::Arena::from_wz_node(root);
        Self::parse_from_arena(&arena, arena_root)
    }
}

fn get_arena_int(arena: &crate::Arena, node_id: crate::NodeId, key: &str) -> Option<i32> {
    let child_id = arena.find_child(node_id, key)?;
    match arena.value(child_id)? {
        crate::NodeValue::Short(v) => Some(*v as i32),
        crate::NodeValue::Int(v) => Some(*v),
        crate::NodeValue::Long(v) => Some(*v as i32),
        crate::NodeValue::Uint(v) => Some(*v as i32),
        _ => None,
    }
}

fn get_arena_string(arena: &crate::Arena, node_id: crate::NodeId, key: &str) -> Option<String> {
    let child_id = arena.find_child(node_id, key)?;
    match arena.value(child_id)? {
        crate::NodeValue::Str(s) | crate::NodeValue::Uol(s) => Some(s.to_string()),
        _ => None,
    }
}

fn get_arena_bool(arena: &crate::Arena, node_id: crate::NodeId, key: &str) -> bool {
    let child_id = match arena.find_child(node_id, key) {
        Some(id) => id,
        None => return false,
    };
    match arena.value(child_id) {
        Some(crate::NodeValue::Bool(b)) => *b,
        Some(crate::NodeValue::Short(v)) => *v != 0,
        Some(crate::NodeValue::Int(v)) => *v != 0,
        _ => false,
    }
}

fn parse_map_info_arena(arena: &crate::Arena, node_id: crate::NodeId) -> MapInfo {
    let mut info = MapInfo {
        map_id: get_arena_int(arena, node_id, "mapID"),
        bgm: get_arena_string(arena, node_id, "bgm"),
        map_mark: get_arena_string(arena, node_id, "mapMark"),
        return_map: get_arena_int(arena, node_id, "returnMap"),
        forced_return: get_arena_int(arena, node_id, "forcedReturn"),
        town: get_arena_bool(arena, node_id, "town"),
        swim: get_arena_bool(arena, node_id, "swim"),
        fly: get_arena_bool(arena, node_id, "fly"),
        vrect: None,
    };

    let vr_left = get_arena_int(arena, node_id, "VRLeft");
    let vr_top = get_arena_int(arena, node_id, "VRTop");
    let vr_right = get_arena_int(arena, node_id, "VRRight");
    let vr_bottom = get_arena_int(arena, node_id, "VRBottom");

    if let (Some(l), Some(t), Some(r), Some(b)) = (vr_left, vr_top, vr_right, vr_bottom) {
        info.vrect = Some(MapVirtualRect {
            left: l,
            top: t,
            right: r,
            bottom: b,
        });
    }

    info
}

fn parse_footholds_arena(arena: &crate::Arena, fh_root: crate::NodeId, map: &mut MapData) {
    for &layer_id in arena.children(fh_root) {
        let layer_name = arena.name(layer_id).unwrap_or("0");
        let layer_idx = layer_name.parse::<usize>().unwrap_or(0);

        for &group_id in arena.children(layer_id) {
            let group_name = arena.name(group_id).unwrap_or("0");
            let group_idx = group_name.parse::<usize>().unwrap_or(0);

            for &item_id in arena.children(group_id) {
                let id_str = arena.name(item_id).unwrap_or("0");
                let id = id_str.parse::<i32>().unwrap_or(0);
                let x1 = get_arena_int(arena, item_id, "x1").unwrap_or(0);
                let y1 = get_arena_int(arena, item_id, "y1").unwrap_or(0);
                let x2 = get_arena_int(arena, item_id, "x2").unwrap_or(0);
                let y2 = get_arena_int(arena, item_id, "y2").unwrap_or(0);
                let prev = get_arena_int(arena, item_id, "prev").unwrap_or(0);
                let next = get_arena_int(arena, item_id, "next").unwrap_or(0);
                let piece = get_arena_int(arena, item_id, "piece").unwrap_or(0);

                let fh = Foothold {
                    id,
                    layer: layer_idx,
                    group: group_idx,
                    x1,
                    y1,
                    x2,
                    y2,
                    prev,
                    next,
                    piece,
                };
                map.footholds_by_id.insert(id, fh.clone());
                map.footholds.push(fh);
            }
        }
    }
}

fn parse_ladder_rope_arena(arena: &crate::Arena, lr_root: crate::NodeId, map: &mut MapData) {
    for &child_id in arena.children(lr_root) {
        let id_str = arena.name(child_id).unwrap_or("0");
        let id = id_str.parse::<i32>().unwrap_or(0);
        let x = get_arena_int(arena, child_id, "x").unwrap_or(0);
        let y1 = get_arena_int(arena, child_id, "y1").unwrap_or(0);
        let y2 = get_arena_int(arena, child_id, "y2").unwrap_or(0);
        let l = get_arena_int(arena, child_id, "l").unwrap_or(0) != 0;
        let uf = get_arena_int(arena, child_id, "uf").unwrap_or(0);
        let page = get_arena_int(arena, child_id, "page").unwrap_or(0);

        map.ladders_ropes.push(LadderRope {
            id,
            x,
            y1,
            y2,
            l,
            uf,
            page,
        });
    }
}

fn parse_portals_arena(arena: &crate::Arena, p_root: crate::NodeId, map: &mut MapData) {
    for &child_id in arena.children(p_root) {
        let id_str = arena.name(child_id).unwrap_or("0");
        let id = id_str.parse::<i32>().unwrap_or(0);
        let p_name = get_arena_string(arena, child_id, "pn").unwrap_or_default();
        let p_type = get_arena_int(arena, child_id, "pt").unwrap_or(0);
        let x = get_arena_int(arena, child_id, "x").unwrap_or(0);
        let y = get_arena_int(arena, child_id, "y").unwrap_or(0);
        let to_map = get_arena_int(arena, child_id, "tm");
        let to_name = get_arena_string(arena, child_id, "tn");
        let script = get_arena_string(arena, child_id, "script");

        map.portals.push(MapPortal {
            id,
            p_name,
            p_type,
            x,
            y,
            to_map,
            to_name,
            script,
        });
    }
}

fn parse_life_arena(arena: &crate::Arena, life_root: crate::NodeId, map: &mut MapData) {
    for &child_id in arena.children(life_root) {
        let id = get_arena_string(arena, child_id, "id").unwrap_or_default();
        let life_type = get_arena_string(arena, child_id, "type").unwrap_or_default();
        let x = get_arena_int(arena, child_id, "x").unwrap_or(0);
        let y = get_arena_int(arena, child_id, "y").unwrap_or(0);
        let cy = get_arena_int(arena, child_id, "cy").unwrap_or(0);
        let fh = get_arena_int(arena, child_id, "fh").unwrap_or(0);
        let flip = get_arena_bool(arena, child_id, "f");
        let hide = get_arena_bool(arena, child_id, "hide");

        map.life.push(MapLife {
            id,
            life_type,
            x,
            y,
            cy,
            fh,
            flip,
            hide,
        });
    }
}

fn parse_back_arena(arena: &crate::Arena, back_root: crate::NodeId, map: &mut MapData) {
    for &child_id in arena.children(back_root) {
        let idx_str = arena.name(child_id).unwrap_or("0");
        let index = idx_str.parse::<usize>().unwrap_or(0);
        let b_s = get_arena_string(arena, child_id, "bS").unwrap_or_default();
        let ani = get_arena_int(arena, child_id, "ani").unwrap_or(0);
        let no = get_arena_string(arena, child_id, "no").unwrap_or_default();
        let x = get_arena_int(arena, child_id, "x").unwrap_or(0);
        let y = get_arena_int(arena, child_id, "y").unwrap_or(0);
        let rx = get_arena_int(arena, child_id, "rx").unwrap_or(0);
        let ry = get_arena_int(arena, child_id, "ry").unwrap_or(0);
        let cx = get_arena_int(arena, child_id, "cx").unwrap_or(0);
        let cy = get_arena_int(arena, child_id, "cy").unwrap_or(0);
        let tile_mode = get_arena_int(arena, child_id, "type").unwrap_or(0);
        let flip = get_arena_bool(arena, child_id, "f");
        let front = get_arena_bool(arena, child_id, "front");
        let alpha = get_arena_int(arena, child_id, "a").unwrap_or(255);

        map.backgrounds.push(BackItem {
            index,
            b_s,
            ani,
            no,
            x,
            y,
            rx,
            ry,
            cx,
            cy,
            tile_mode,
            flip,
            front,
            alpha,
        });
    }
}

fn parse_layer_arena(
    arena: &crate::Arena,
    layer_node: crate::NodeId,
    layer_idx: usize,
    map: &mut MapData,
) {
    // 1. obj
    if let Some(obj_root) = arena.find_child(layer_node, "obj") {
        for &child_id in arena.children(obj_root) {
            let idx_str = arena.name(child_id).unwrap_or("0");
            let index = idx_str.parse::<usize>().unwrap_or(0);
            let o_s = get_arena_string(arena, child_id, "oS").unwrap_or_default();
            let l0 = get_arena_string(arena, child_id, "l0").unwrap_or_default();
            let l1 = get_arena_string(arena, child_id, "l1").unwrap_or_default();
            let l2 = get_arena_string(arena, child_id, "l2").unwrap_or_default();
            let x = get_arena_int(arena, child_id, "x").unwrap_or(0);
            let y = get_arena_int(arena, child_id, "y").unwrap_or(0);
            let z = get_arena_int(arena, child_id, "z").unwrap_or(0);
            let flip = get_arena_bool(arena, child_id, "f");

            map.objects.push(ObjItem {
                layer: layer_idx,
                index,
                o_s,
                l0,
                l1,
                l2,
                x,
                y,
                z,
                flip,
            });
        }
    }

    // 2. tile
    let t_s = arena
        .find_child(layer_node, "info")
        .and_then(|info_id| get_arena_string(arena, info_id, "tS"))
        .unwrap_or_default();

    if let Some(tile_root) = arena.find_child(layer_node, "tile") {
        for &child_id in arena.children(tile_root) {
            let idx_str = arena.name(child_id).unwrap_or("0");
            let index = idx_str.parse::<usize>().unwrap_or(0);
            let u = get_arena_string(arena, child_id, "u").unwrap_or_default();
            let no = get_arena_string(arena, child_id, "no").unwrap_or_default();
            let x = get_arena_int(arena, child_id, "x").unwrap_or(0);
            let y = get_arena_int(arena, child_id, "y").unwrap_or(0);

            map.tiles.push(TileItem {
                layer: layer_idx,
                index,
                t_s: t_s.clone(),
                u,
                no,
                x,
                y,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wz_core::image::ImageValue;

    #[test]
    fn test_map_scene_parsing_and_footholds() {
        let mut root = Node::new("100000000.img");

        // info
        let mut info = Node::new("info");
        let mut bgm = Node::new("bgm");
        bgm.value = Some(ImageValue::Str("Bgm00/Sleepywood".into()));
        let mut vr_l = Node::new("VRLeft");
        vr_l.value = Some(ImageValue::Int(-1000));
        let mut vr_r = Node::new("VRRight");
        vr_r.value = Some(ImageValue::Int(1000));
        let mut vr_t = Node::new("VRTop");
        vr_t.value = Some(ImageValue::Int(-800));
        let mut vr_b = Node::new("VRBottom");
        vr_b.value = Some(ImageValue::Int(600));
        info.children.push(bgm);
        info.children.push(vr_l);
        info.children.push(vr_r);
        info.children.push(vr_t);
        info.children.push(vr_b);
        root.children.push(info);

        // foothold/0/0/1
        let mut fh_root = Node::new("foothold");
        let mut layer0 = Node::new("0");
        let mut group0 = Node::new("0");
        let mut fh1 = Node::new("1");
        let mut x1 = Node::new("x1");
        x1.value = Some(ImageValue::Int(-500));
        let mut y1 = Node::new("y1");
        y1.value = Some(ImageValue::Int(100));
        let mut x2 = Node::new("x2");
        x2.value = Some(ImageValue::Int(500));
        let mut y2 = Node::new("y2");
        y2.value = Some(ImageValue::Int(100));
        fh1.children.push(x1);
        fh1.children.push(y1);
        fh1.children.push(x2);
        fh1.children.push(y2);
        group0.children.push(fh1);
        layer0.children.push(group0);
        fh_root.children.push(layer0);
        root.children.push(fh_root);

        let map_data = MapData::parse_from_node(&root);
        assert_eq!(map_data.info.bgm.as_deref(), Some("Bgm00/Sleepywood"));
        assert!(map_data.info.vrect.is_some());
        assert_eq!(map_data.info.vrect.unwrap().left, -1000);
        assert_eq!(map_data.footholds.len(), 1);
        assert_eq!(map_data.footholds[0].id, 1);
        assert!(map_data.footholds[0].is_flat());
        assert!(!map_data.footholds[0].is_wall());

        // FootholdTree 검증
        let tree = FootholdTree::build(&map_data.footholds);
        let bounds = tree.bounds.expect("bounds calculated");
        assert_eq!(bounds.left, -500);
        assert_eq!(bounds.right, 500);
        assert_eq!(bounds.top, 100);
        assert_eq!(bounds.bottom, 100);
        assert_eq!(tree.get_by_id(1).map(|f| f.id), Some(1));
        assert!(tree.get_by_id(999).is_none());
    }

    #[test]
    fn test_map_domain_types_and_accessors() {
        let lr_ladder = LadderRope {
            l: true,
            ..Default::default()
        };
        let lr_rope = LadderRope {
            l: false,
            ..Default::default()
        };
        assert_eq!(lr_ladder.kind(), LadderRopeKind::Ladder);
        assert_eq!(lr_rope.kind(), LadderRopeKind::Rope);

        let life_npc = MapLife {
            life_type: "n".into(),
            ..Default::default()
        };
        let life_mob = MapLife {
            life_type: "m".into(),
            ..Default::default()
        };
        let life_other = MapLife {
            life_type: "x".into(),
            ..Default::default()
        };
        assert_eq!(life_npc.kind(), LifeKind::Npc);
        assert_eq!(life_mob.kind(), LifeKind::Mob);
        assert_eq!(life_other.kind(), LifeKind::Unknown);

        let back = BackItem {
            b_s: "grassyCat".into(),
            ..Default::default()
        };
        assert_eq!(back.bg_set(), "grassyCat");

        let obj = ObjItem {
            o_s: "house".into(),
            ..Default::default()
        };
        assert_eq!(obj.obj_set(), "house");

        let tile = TileItem {
            t_s: "woodFloor".into(),
            ..Default::default()
        };
        assert_eq!(tile.tile_set(), "woodFloor");
    }
}
