//! DB2 Item 그리드 (`DB2Form.LoadItem`).
//!
//! `Item/<dir>/<image>.img`의 각 아이템 노드에 대해
//! `String/<dir>.img/<id>/name|desc`를 붙여 그리드 행을 만든다.

use crate::linker::StringLinker;
use serde::{Deserialize, Serialize};
use wz_core::image::Node;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Db2ItemRow {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub has_icon: bool,
}

fn item_children(node: &Node) -> impl Iterator<Item = &Node> {
    node.object_children()
        .iter()
        .filter(|c| c.name.as_ref().parse::<i32>().is_ok())
}

fn has_icon(node: &Node) -> bool {
    node.object_children()
        .iter()
        .find(|c| c.name.as_ref() == "info")
        .and_then(|info| {
            info.object_children()
                .iter()
                .find(|c| c.name.as_ref() == "icon")
        })
        .is_some()
}

/// 이미지 루트의 아이템들을 DB2 Item 그리드 행으로 만든다.
pub fn item_grid_rows(root: &Node, linker: &StringLinker) -> Vec<Db2ItemRow> {
    let mut rows: Vec<Db2ItemRow> = item_children(root)
        .map(|item| {
            let id = item.name.to_string();
            let parsed = id.parse::<i32>().ok();
            let string = parsed.and_then(|id| linker.string_item.get(&id));
            Db2ItemRow {
                id,
                name: string
                    .map(|s| s.name.clone())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "  ".into()),
                desc: string
                    .and_then(|s| s.desc.clone())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| "  ".into()),
                has_icon: has_icon(item),
            }
        })
        .collect();
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use wz_core::image::ImageValue;

    #[test]
    fn builds_rows_with_string_fallback() {
        let mut root = Node::new("0200.img");
        let mut item = Node::new("02000000");
        let mut info = Node::new("info");
        let mut icon = Node::new("icon");
        icon.value = Some(ImageValue::Int(1));
        info.children.push(icon);
        item.children.push(info);
        root.children.push(item);

        let mut linker = StringLinker::new();
        let mut sr = crate::linker::StringResult {
            name: "빨간 포션".into(),
            desc: Some("HP 회복".into()),
            ..Default::default()
        };
        sr.full_path = "String/Consume.img/02000000".into();
        linker.string_item.insert(2_000_000, sr);

        let rows = item_grid_rows(&root, &linker);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "빨간 포션");
        assert_eq!(rows[0].desc, "HP 회복");
        assert!(rows[0].has_icon);
    }

    #[test]
    fn missing_string_uses_db2_placeholder() {
        let mut root = Node::new("0200.img");
        root.children.push(Node::new("02000099"));
        let rows = item_grid_rows(&root, &StringLinker::new());
        assert_eq!(rows[0].name, "  ");
        assert_eq!(rows[0].desc, "  ");
        assert!(!rows[0].has_icon);
    }
}
