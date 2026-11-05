//! DB2 Map 그리드 (`DB2Form.LoadMap`).
//!
//! `Map/Map/MapX/<id>.img` 이미지마다 `String/Map.img/<id>`의
//! `streetName`/`mapName`, 미니맵 캔버스 존재 여부, `info/link`를 행으로 만든다.

use crate::linker::StringLinker;
use serde::{Deserialize, Serialize};
use wz_core::dirtree::DirTree;
use wz_core::image::{ImageValue, Node};
use wz_core::WzArchive;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Db2MapRow {
    pub id: String,
    pub street_name: String,
    pub map_name: String,
    pub has_minimap: bool,
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

/// 한 Map WZ 의 이미지들을 DB2 Map 그리드 행으로 만든다.
pub fn map_grid_rows(archive: &WzArchive, tree: &DirTree, linker: &StringLinker) -> Vec<Db2MapRow> {
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
        let (street_name, map_name) = id
            .parse::<i32>()
            .ok()
            .and_then(|id| linker.string_map.get(&id))
            .map(|sr| {
                (
                    sr.street_name.clone().unwrap_or_default(),
                    sr.map_name.clone().unwrap_or_default(),
                )
            })
            .unwrap_or_default();
        rows.push(Db2MapRow {
            id: id.to_string(),
            street_name,
            map_name,
            has_minimap: child(&node, "miniMap")
                .and_then(|mini| child(mini, "canvas"))
                .is_some(),
            link: child(&node, "info").and_then(|info| int(info, "link")),
        });
    }
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    rows
}

/// 편의용: 아카이브를 열어 트리까지 읽고 행을 만든다.
pub fn map_grid_rows_from_path(
    path: &std::path::Path,
    linker: &StringLinker,
) -> Result<Vec<Db2MapRow>, wz_error::WzError> {
    let archive = WzArchive::open(path)?;
    let tree = archive.read_tree()?;
    Ok(map_grid_rows(&archive, &tree, linker))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wz_core::image::{ImageValue, Node};

    fn map_node(with_minimap: bool, link: Option<i32>) -> Node {
        let mut node = Node::new("000010000.img");
        let mut info = Node::new("info");
        if let Some(link) = link {
            let mut link_node = Node::new("link");
            link_node.value = Some(ImageValue::Int(link));
            info.children.push(link_node);
        }
        node.children.push(info);
        if with_minimap {
            let mut mini = Node::new("miniMap");
            mini.children.push(Node::new("canvas"));
            node.children.push(mini);
        }
        node
    }

    #[test]
    fn row_fields_are_extracted() {
        let node = map_node(true, Some(100000000));
        assert!(child(&node, "miniMap")
            .and_then(|mini| child(mini, "canvas"))
            .is_some());
        assert_eq!(
            child(&node, "info").and_then(|i| int(i, "link")),
            Some(100_000_000)
        );
    }
}
