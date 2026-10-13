//! 메이플스토리 맵 씬 데이터 및 발판(Foothold) 계층 구조 파서 (`MapData.cs`, `FootholdManager.cs` 포트).
//!
//! 원본 `WzComparerR2.MapRender/MapData.cs` 및 `FootholdManager.cs`, `Patches2/*.cs` 대응.

use std::collections::HashMap;
use wz_core::image::{ImageValue, Node};

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

/// 사다리 / 로프 데이터.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LadderRope {
    pub id: i32,
    pub x: i32,
    pub y1: i32,
    pub y2: i32,
    pub l: bool, // true: ladder, false: rope
    pub uf: i32,
    pub page: i32,
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

/// 라이프 (NPC / Mob) 배치 데이터.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapLife {
    pub id: String,
    pub life_type: String, // "n": NPC, "m": Mob
    pub x: i32,
    pub y: i32,
    pub cy: i32,
    pub fh: i32,
    pub flip: bool,
    pub hide: bool,
}

/// 전체 맵 물리 씬 데이터 컨테이너 (`MapData`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MapData {
    pub info: MapInfo,
    pub footholds: Vec<Foothold>,
    pub footholds_by_id: HashMap<i32, Foothold>,
    pub ladders_ropes: Vec<LadderRope>,
    pub portals: Vec<MapPortal>,
    pub life: Vec<MapLife>,
}

impl MapData {
    /// WZ 맵 노드 트리(`Map/Map/MapX/XXXXXXXXX.img` 루트)에서 맵 데이터를 파싱한다.
    pub fn parse_from_node(root: &Node) -> Self {
        let mut data = Self::default();

        // 1. info 노드 파싱
        if let Some(info_node) = find_child_node(root, "info") {
            data.info = parse_map_info(info_node);
        }

        // 2. foothold 노드 파싱 (`foothold/{layer}/{group}/{id}`)
        if let Some(fh_node) = find_child_node(root, "foothold") {
            parse_footholds(fh_node, &mut data);
        }

        // 3. ladderRope 노드 파싱 (`ladderRope/{id}`)
        if let Some(lr_node) = find_child_node(root, "ladderRope") {
            parse_ladder_rope(lr_node, &mut data);
        }

        // 4. portal 노드 파싱 (`portal/{id}`)
        if let Some(p_node) = find_child_node(root, "portal") {
            parse_portals(p_node, &mut data);
        }

        // 5. life 노드 파싱 (`life/{id}`)
        if let Some(life_node) = find_child_node(root, "life") {
            parse_life(life_node, &mut data);
        }

        data
    }
}

fn find_child_node<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    if let Some(ImageValue::Property(p)) = &node.value {
        if let Some(c) = p.children.iter().find(|c| c.name.as_ref() == name) {
            return Some(c);
        }
    }
    node.children.iter().find(|c| c.name.as_ref() == name)
}

fn get_node_int(node: &Node, key: &str) -> Option<i32> {
    let target = find_child_node(node, key)?;
    match target.value.as_ref()? {
        ImageValue::Short(v) => Some(*v as i32),
        ImageValue::Int(v) => Some(*v),
        ImageValue::Long(v) => Some(*v as i32),
        ImageValue::Uint(v) => Some(*v as i32),
        _ => None,
    }
}

fn get_node_string(node: &Node, key: &str) -> Option<String> {
    let target = find_child_node(node, key)?;
    match target.value.as_ref()? {
        ImageValue::Str(s) | ImageValue::Uol(s) => Some(s.to_string()),
        _ => None,
    }
}

fn get_node_bool(node: &Node, key: &str) -> bool {
    let target = match find_child_node(node, key) {
        Some(t) => t,
        None => return false,
    };
    match target.value.as_ref() {
        Some(ImageValue::Bool(b)) => *b,
        Some(ImageValue::Short(v)) => *v != 0,
        Some(ImageValue::Int(v)) => *v != 0,
        _ => false,
    }
}

fn parse_map_info(node: &Node) -> MapInfo {
    let mut info = MapInfo {
        map_id: get_node_int(node, "mapID"),
        bgm: get_node_string(node, "bgm"),
        map_mark: get_node_string(node, "mapMark"),
        return_map: get_node_int(node, "returnMap"),
        forced_return: get_node_int(node, "forcedReturn"),
        town: get_node_bool(node, "town"),
        swim: get_node_bool(node, "swim"),
        fly: get_node_bool(node, "fly"),
        vrect: None,
    };

    let vr_left = get_node_int(node, "VRLeft");
    let vr_top = get_node_int(node, "VRTop");
    let vr_right = get_node_int(node, "VRRight");
    let vr_bottom = get_node_int(node, "VRBottom");

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

fn parse_footholds(fh_root: &Node, map: &mut MapData) {
    let layer_children = get_all_children(fh_root);
    for layer_node in layer_children {
        let layer_idx = layer_node.name.parse::<usize>().unwrap_or(0);
        let group_children = get_all_children(layer_node);
        for group_node in group_children {
            let group_idx = group_node.name.parse::<usize>().unwrap_or(0);
            let fh_children = get_all_children(group_node);
            for item_node in fh_children {
                let id = item_node.name.parse::<i32>().unwrap_or(0);
                let x1 = get_node_int(item_node, "x1").unwrap_or(0);
                let y1 = get_node_int(item_node, "y1").unwrap_or(0);
                let x2 = get_node_int(item_node, "x2").unwrap_or(0);
                let y2 = get_node_int(item_node, "y2").unwrap_or(0);
                let prev = get_node_int(item_node, "prev").unwrap_or(0);
                let next = get_node_int(item_node, "next").unwrap_or(0);
                let piece = get_node_int(item_node, "piece").unwrap_or(0);

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

fn parse_ladder_rope(lr_root: &Node, map: &mut MapData) {
    for child in get_all_children(lr_root) {
        let id = child.name.parse::<i32>().unwrap_or(0);
        let x = get_node_int(child, "x").unwrap_or(0);
        let y1 = get_node_int(child, "y1").unwrap_or(0);
        let y2 = get_node_int(child, "y2").unwrap_or(0);
        let l = get_node_int(child, "l").unwrap_or(0) != 0;
        let uf = get_node_int(child, "uf").unwrap_or(0);
        let page = get_node_int(child, "page").unwrap_or(0);

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

fn parse_portals(p_root: &Node, map: &mut MapData) {
    for child in get_all_children(p_root) {
        let id = child.name.parse::<i32>().unwrap_or(0);
        let p_name = get_node_string(child, "pn").unwrap_or_default();
        let p_type = get_node_int(child, "pt").unwrap_or(0);
        let x = get_node_int(child, "x").unwrap_or(0);
        let y = get_node_int(child, "y").unwrap_or(0);
        let to_map = get_node_int(child, "tm");
        let to_name = get_node_string(child, "tn");
        let script = get_node_string(child, "script");

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

fn parse_life(life_root: &Node, map: &mut MapData) {
    for child in get_all_children(life_root) {
        let id = get_node_string(child, "id").unwrap_or_default();
        let life_type = get_node_string(child, "type").unwrap_or_default();
        let x = get_node_int(child, "x").unwrap_or(0);
        let y = get_node_int(child, "y").unwrap_or(0);
        let cy = get_node_int(child, "cy").unwrap_or(0);
        let fh = get_node_int(child, "fh").unwrap_or(0);
        let flip = get_node_bool(child, "f");
        let hide = get_node_bool(child, "hide");

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

fn get_all_children(node: &Node) -> Vec<&Node> {
    let mut res = Vec::new();
    if let Some(ImageValue::Property(p)) = &node.value {
        for c in &p.children {
            res.push(c);
        }
    }
    for c in &node.children {
        res.push(c);
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }
}
