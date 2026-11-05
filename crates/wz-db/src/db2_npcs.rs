//! DB2 NPC 그리드 (`DB2Form.LoadNpc`).
//!
//! `Npc/<id>.img`마다 `String/Npc.img/<id>/name`, `stand/0`/`default/0`
//! 아이콘 소스, `info/link`를 행으로 만든다.

use crate::linker::StringLinker;
use serde::{Deserialize, Serialize};
use wz_core::dirtree::DirTree;
use wz_core::image::{ImageValue, Node};
use wz_core::WzArchive;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Db2NpcRow {
    pub id: String,
    pub name: String,
    pub has_stand_0: bool,
    pub has_default_0: bool,
    pub has_link: bool,
    pub link: Option<i32>,
}

fn child<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    node.object_children()
        .iter()
        .find(|c| c.name.as_ref() == name)
}

fn int(node: &Node, key: &str) -> Option<i32> {
    child(node, key).and_then(|c| match &c.value {
        Some(ImageValue::Int(v)) => Some(*v),
        Some(ImageValue::Short(v)) => Some(i32::from(*v)),
        Some(ImageValue::Long(v)) => Some(*v as i32),
        Some(ImageValue::Uint(v)) => Some(*v as i32),
        Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => s.parse().ok(),
        _ => None,
    })
}

/// 한 NPC WZ 의 이미지들을 DB2 NPC 그리드 행으로 만든다.
pub fn npc_grid_rows(archive: &WzArchive, tree: &DirTree, linker: &StringLinker) -> Vec<Db2NpcRow> {
    let mut rows = Vec::new();
    for entry in &tree.root.entries {
        if entry.node_type != 4 {
            continue;
        }
        let Some(name) = entry.name.as_deref() else {
            continue;
        };
        let Some(id) = name.strip_suffix(".img") else {
            continue;
        };
        if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let Some(offset) = entry.offset else {
            continue;
        };
        let Ok(node) = archive.parse_image_at(offset) else {
            continue;
        };
        let link = child(&node, "info").and_then(|info| int(info, "link"));
        rows.push(Db2NpcRow {
            id: id.to_string(),
            name: id
                .parse::<i32>()
                .ok()
                .and_then(|n| linker.string_npc.get(&n))
                .map(|sr| sr.name.clone())
                .unwrap_or_default(),
            has_stand_0: child(&node, "stand")
                .and_then(|stand| child(stand, "0"))
                .is_some(),
            has_default_0: child(&node, "default")
                .and_then(|default| child(default, "0"))
                .is_some(),
            has_link: link.is_some(),
            link,
        });
    }
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use wz_core::image::ImageValue;

    #[test]
    fn extracts_link_and_icon_sources() {
        let mut node = Node::new("1541000.img");
        let mut stand = Node::new("stand");
        stand.children.push(Node::new("0"));
        node.children.push(stand);
        let mut info = Node::new("info");
        let mut link = Node::new("link");
        link.value = Some(ImageValue::Str("1541001".into()));
        info.children.push(link);
        node.children.push(info);

        assert!(child(&node, "stand").and_then(|s| child(s, "0")).is_some());
        assert_eq!(
            child(&node, "info").and_then(|i| int(i, "link")),
            Some(1_541_001)
        );
    }
}
