//! `MapTooltipRenderer`의 텍스트 필드.
//!
//! 렌더러는 `Map.CreateFromNode`의 필드와 `StringLinker.StringMap`으로
//! 마을/맵 이름, 설명, BGM, 몹/ NPC 목록을 그린다. GDI 레이아웃은 제외한다.

use serde::{Deserialize, Serialize};
use wz_core::image::{ImageValue, Node};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapTooltipFields {
    pub id: i32,
    pub link: Option<i32>,
    pub barrier: i32,
    pub barrier_arc: i32,
    pub barrier_aut: i32,
    pub map_mark: Option<String>,
    pub bgm: Option<String>,
    pub mobs: Vec<i32>,
    pub npcs: Vec<i32>,
}

fn child<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    node.object_children()
        .iter()
        .find(|c| c.name.as_ref() == name)
}

fn as_int(node: Option<&Node>) -> Option<i32> {
    node.and_then(|n| match &n.value {
        Some(ImageValue::Int(v)) => Some(*v),
        Some(ImageValue::Short(v)) => Some(i32::from(*v)),
        Some(ImageValue::Long(v)) => Some(*v as i32),
        Some(ImageValue::Uint(v)) => Some(*v as i32),
        Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => s.parse().ok(),
        _ => None,
    })
}

fn as_string(node: Option<&Node>) -> Option<String> {
    node.and_then(|n| match &n.value {
        Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => Some(s.to_string()),
        _ => None,
    })
}

impl MapTooltipFields {
    pub fn from_node(node: &Node) -> Option<Self> {
        let id: i32 = node.name.as_ref().strip_suffix(".img")?.parse().ok()?;
        let mut map = MapTooltipFields {
            id,
            ..Default::default()
        };
        if let Some(info) = child(node, "info") {
            map.map_mark = as_string(child(info, "mapMark"));
            map.link = as_int(child(info, "link"));
            map.barrier = as_int(child(info, "barrier")).unwrap_or(0);
            map.barrier_arc = as_int(child(info, "barrierArc")).unwrap_or(0);
            map.barrier_aut = as_int(child(info, "barrierAut")).unwrap_or(0);
            map.bgm = as_string(child(info, "bgm"));
            let flying = child(info, "AFKmob")
                .and_then(|n| child(n, "flyingMob"))
                .and_then(|n| as_int(Some(n)))
                .unwrap_or(0);
            if flying != 0 {
                map.mobs.push(flying);
            }
        }

        // 이 클라이언트의 분할 WZ에서는 링크 대상이 다른 파일일 수 있어
        // 원본 `findNode == null` 경로와 같이 현재 노드를 사용한다.
        if let Some(life) = child(node, "life") {
            for entry in life.object_children() {
                let life_type = as_string(child(entry, "type"));
                let life_id = as_int(child(entry, "id")).unwrap_or(0);
                if life_id == 0 {
                    continue;
                }
                match life_type.as_deref() {
                    Some("n") if !map.npcs.contains(&life_id) => map.npcs.push(life_id),
                    Some("m") if !map.mobs.contains(&life_id) => map.mobs.push(life_id),
                    _ => {}
                }
            }
        }
        Some(map)
    }
}

/// 렌더러가 참조하는 문자열 묶음.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MapTooltipText {
    /// `GetMapName`의 두 줄 (street, mapName). 없으면 `(null)`.
    pub street: String,
    pub map_name: String,
    pub id_text: String,
    pub desc: Option<String>,
    pub bgm: Option<String>,
    /// `(id, name)`; 이름이 없으면 `(null)`.
    pub mobs: Vec<(i32, String)>,
    pub npcs: Vec<(i32, String)>,
}

impl MapTooltipFields {
    /// `link` 폴백을 포함한 `StringMap` 조회.
    pub fn text(
        &self,
        street: Option<String>,
        map_name: Option<String>,
        desc: Option<String>,
        mob_names: &dyn Fn(i32) -> Option<String>,
        npc_names: &dyn Fn(i32) -> Option<String>,
    ) -> MapTooltipText {
        MapTooltipText {
            street: street.unwrap_or_else(|| "(null)".into()),
            map_name: map_name.unwrap_or_else(|| "(null)".into()),
            id_text: format!("ID:{}", self.id),
            desc,
            bgm: self.bgm.clone().filter(|s| !s.is_empty()),
            mobs: self
                .mobs
                .iter()
                .map(|id| (*id, mob_names(*id).unwrap_or_else(|| "(null)".into())))
                .collect(),
            npcs: self
                .npcs
                .iter()
                .map(|id| (*id, npc_names(*id).unwrap_or_else(|| "(null)".into())))
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collects_info_and_life() {
        let mut node = Node::new("000010000.img");
        let mut info = Node::new("info");
        let mut bgm = Node::new("bgm");
        bgm.value = Some(ImageValue::Str("Bgm00/GoPicnic".into()));
        info.children.push(bgm);
        let mut barrier = Node::new("barrier");
        barrier.value = Some(ImageValue::Int(5));
        info.children.push(barrier);
        node.children.push(info);
        let mut life = Node::new("life");
        for (t, id) in [("m", 100100), ("n", 2000), ("m", 100100)] {
            let mut e = Node::new("0");
            let mut tn = Node::new("type");
            tn.value = Some(ImageValue::Str(t.into()));
            let mut idn = Node::new("id");
            idn.value = Some(ImageValue::Int(id));
            e.children.push(tn);
            e.children.push(idn);
            life.children.push(e);
        }
        node.children.push(life);
        let map = MapTooltipFields::from_node(&node).unwrap();
        assert_eq!(map.barrier, 5);
        assert_eq!(map.mobs, vec![100100]);
        assert_eq!(map.npcs, vec![2000]);
    }
}
