//! DB2 Reactor 그리드 (`DB2Form.LoadReactor`).
//!
//! `Reactor/<id>.img`마다 아이콘 소스(`0/0`), `info` 존재, `info/link`를
//! 행으로 만든다. 원본 그리드는 ID/아이콘/info 덤프 3열이다.

use serde::{Deserialize, Serialize};
use wz_core::dirtree::DirTree;
use wz_core::image::{ImageValue, Node};
use wz_core::WzArchive;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Db2ReactorRow {
    pub id: String,
    pub has_icon: bool,
    pub has_info: bool,
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

/// 한 Reactor WZ 의 이미지들을 DB2 Reactor 그리드 행으로 만든다.
pub fn reactor_grid_rows(archive: &WzArchive, tree: &DirTree) -> Vec<Db2ReactorRow> {
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
        let info = child(&node, "info");
        let link = info.and_then(|info| int(info, "link"));
        rows.push(Db2ReactorRow {
            id: id.to_string(),
            has_icon: child(&node, "0")
                .and_then(|stage| child(stage, "0"))
                .is_some(),
            has_info: info.is_some(),
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

    #[test]
    fn extracts_icon_and_link_flags() {
        let mut node = Node::new("0100000.img");
        let mut stage = Node::new("0");
        stage.children.push(Node::new("0"));
        node.children.push(stage);
        let mut info = Node::new("info");
        let mut link = Node::new("link");
        link.value = Some(ImageValue::Int(1002009));
        info.children.push(link);
        node.children.push(info);

        assert!(child(&node, "0").and_then(|s| child(s, "0")).is_some());
        assert_eq!(
            child(&node, "info").and_then(|i| int(i, "link")),
            Some(1_002_009)
        );
    }
}
