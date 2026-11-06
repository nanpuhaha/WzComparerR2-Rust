//! NPC/Reactor 툴팁의 텍스트 라인.
//!
//! - `NpcTooltipRenderer`: 이름, `ID:n`, `등장위치 :`, ` 맵이름(mapID)`, 없으면 ` 불명`
//! - `ReactorTooltipRenderer`: `이름:`, `클립: n개`, `액션:`
//!
//! GDI 레이아웃/이미지는 제외하고 문자열만 맞춘다.

use serde::{Deserialize, Serialize};
use std::sync::Arc;
use wz_core::image::{ImageValue, Node};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NpcTooltip {
    pub id: i32,
    pub name: String,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReactorTooltip {
    pub id: i32,
    pub name: Option<String>,
    pub action: Option<String>,
    pub clip_count: i32,
    pub lines: Vec<String>,
}

fn child<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    node.object_children()
        .iter()
        .find(|c| c.name.as_ref() == name)
}

fn scalar(node: &Node) -> Option<String> {
    match &node.value {
        Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => Some(s.to_string()),
        _ => None,
    }
}

/// 7자리 mob/npc id (`8144006.img`).
pub fn seven_digit_id(node: &Node) -> Option<i32> {
    node.name.as_ref().strip_suffix(".img")?.parse().ok()
}

fn map_name_path(_maps: &dyn Fn(i32) -> Option<String>, id: i32) -> String {
    _maps(id).unwrap_or_else(|| "null".to_string())
}

/// `NpcTooltipRenderer` 텍스트.
///
/// `locations`는 `Etc/NpcLocation.img/<id>`의 자식 `(원본 텍스트, 파싱된 mapID)`다.
pub fn npc_lines(
    name: Option<&str>,
    id: i32,
    locations: &[(String, Option<i32>)],
    map_name: &dyn Fn(i32) -> Option<String>,
) -> Vec<String> {
    let mut lines = vec![
        name.unwrap_or("(null)").to_string(),
        format!("ID:{id}"),
        "등장위치 :".to_string(),
    ];
    for (raw, map_id) in locations {
        let resolved = map_id
            .map(|mid| map_name_path(map_name, mid))
            .unwrap_or_else(|| "null".to_string());
        lines.push(format!(" {resolved}({raw})"));
    }
    if locations.is_empty() {
        lines.push(" 불명".to_string());
    }
    lines
}

/// `Reactor.CreateFromNode`의 텍스트 필드 + 렌더러 라인.
pub fn reactor_from_node(node: &Node) -> Option<ReactorTooltip> {
    let id: i32 = node.name.as_ref().strip_suffix(".img")?.parse().ok()?;
    let mut name = None;
    let mut action = None;
    if let Some(info) = child(node, "info") {
        name = child(info, "viewName")
            .and_then(scalar)
            .or_else(|| child(info, "name").and_then(scalar))
            .or_else(|| child(info, "info").and_then(scalar));
    }
    if let Some(a) = child(node, "action") {
        action = scalar(a);
    }
    let mut clip_count = 0;
    while child(node, &clip_count.to_string()).is_some() {
        clip_count += 1;
    }
    let mut lines = vec![
        format!(
            "이름: {}",
            name.clone()
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "(null)".into())
        ),
        format!("클립: {clip_count}개"),
    ];
    if let Some(action) = action.as_ref().filter(|s| !s.is_empty()) {
        lines.push(format!("액션: {action}"));
    }
    Some(ReactorTooltip {
        id,
        name,
        action,
        clip_count,
        lines,
    })
}

/// `Etc/NpcLocation.img`에서 특정 NPC의 위치 목록을 만든다.
pub fn npc_locations(image: &Node, id: i32) -> Vec<(String, Option<i32>)> {
    let Some(node) = child(image, &id.to_string()) else {
        return Vec::new();
    };
    node.object_children()
        .iter()
        .map(|c| (c.name.to_string(), c.name.parse::<i32>().ok()))
        .collect()
}

/// `Arc<str>` 이름을 붙인 이미지 노드.
pub fn named_node(mut node: Node, name: &str) -> Node {
    node.name = Arc::from(name);
    node
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npc_lines_include_unknown_location() {
        let map = |id: i32| (id == 100000000).then(|| "헤네시스".to_string());
        assert_eq!(
            npc_lines(Some("리트"), 1000, &[], &map),
            vec!["리트", "ID:1000", "등장위치 :", " 불명"]
        );
        assert_eq!(
            npc_lines(None, 7, &[("100000000".into(), Some(100000000))], &map),
            vec!["(null)", "ID:7", "등장위치 :", " 헤네시스(100000000)"]
        );
    }

    #[test]
    fn reactor_counts_clips() {
        let mut node = Node::new("0100000.img");
        node.children.push(Node::new("0"));
        node.children.push(Node::new("1"));
        let mut info = Node::new("info");
        let mut view = Node::new("viewName");
        view.value = Some(ImageValue::Str("상자".into()));
        info.children.push(view);
        node.children.push(info);
        let r = reactor_from_node(&node).unwrap();
        assert_eq!(r.clip_count, 2);
        assert_eq!(r.lines, vec!["이름: 상자", "클립: 2개"]);
    }
}
