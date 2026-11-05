//! DB2 Mob 그리드 (`DB2Form.LoadMob`).
//!
//! 원본은 `String/Mob.img`의 ID를 순회하며 `Mob/<id>.img`가 있는 항목만
//! 그리드에 넣고, `stand/0` → `fly/0` 순으로 아이콘을 고른다. 여기서는
//! 주어진 Mob WZ(분할 `_Canvas` 포함)에 존재하는 이미지 중 이름이 있는
//! 항목을 같은 규칙으로 행으로 만든다.

use crate::linker::StringLinker;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wz_core::dirtree::DirTree;
use wz_core::image::{ImageValue, Node};
use wz_core::WzArchive;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Db2MobRow {
    pub id: String,
    pub name: String,
    pub has_stand: bool,
    pub has_fly: bool,
    pub has_move: bool,
    pub has_hit: bool,
    pub has_die: bool,
    pub has_thumbnail: bool,
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

/// 한 Mob WZ 의 이미지들을 DB2 Mob 그리드 행으로 만든다.
pub fn mob_grid_rows(archive: &WzArchive, tree: &DirTree, linker: &StringLinker) -> Vec<Db2MobRow> {
    let mut images: HashMap<&str, u64> = HashMap::new();
    for entry in &tree.root.entries {
        if entry.node_type != 4 {
            continue;
        }
        if let (Some(name), Some(offset)) = (entry.name.as_deref(), entry.offset) {
            if name.ends_with(".img") {
                images.insert(name, offset);
            }
        }
    }

    let mut rows = Vec::new();
    for (id, string) in &linker.string_mob {
        let name = format!("{id:07}.img");
        let Some(offset) = images.get(name.as_str()) else {
            continue;
        };
        let Ok(node) = archive.parse_image_at(*offset) else {
            continue;
        };
        rows.push(Db2MobRow {
            id: format!("{id:07}"),
            name: string.name.clone(),
            has_stand: child(&node, "stand").is_some(),
            has_fly: child(&node, "fly").is_some(),
            has_move: child(&node, "move").is_some(),
            has_hit: child(&node, "hit1").is_some() || child(&node, "hit").is_some(),
            has_die: child(&node, "die1").is_some() || child(&node, "die").is_some(),
            has_thumbnail: child(&node, "info")
                .and_then(|info| child(info, "thumbnail"))
                .is_some(),
            link: child(&node, "info").and_then(|info| int(info, "link")),
        });
    }
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use wz_core::image::ImageValue;

    fn node() -> Node {
        let mut node = Node::new("0100100.img");
        node.children.push(Node::new("stand"));
        node.children.push(Node::new("move"));
        let mut info = Node::new("info");
        let mut link = Node::new("link");
        link.value = Some(ImageValue::Int(100100));
        info.children.push(link);
        node.children.push(info);
        node
    }

    #[test]
    fn extracts_flags_and_link() {
        let node = node();
        assert!(child(&node, "stand").is_some());
        assert!(child(&node, "fly").is_none());
        assert_eq!(
            child(&node, "info").and_then(|i| int(i, "link")),
            Some(100100)
        );
    }
}
