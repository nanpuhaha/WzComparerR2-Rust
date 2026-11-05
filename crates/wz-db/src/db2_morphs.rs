//! DB2 Morph 그리드 (`DB2Form.LoadMorph`).
//!
//! `Item/Consume/0221.img`의 각 아이템에 대해 이름/설명, `spec/morph`,
//! 아이콘 존재 여부, `Morph/<morph>.img/walk/0` 존재 여부를 행으로 만든다.

use crate::linker::StringLinker;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wz_core::dirtree::DirTree;
use wz_core::image::{ImageValue, Node};
use wz_core::WzArchive;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Db2MorphRow {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub morph_id: String,
    pub has_icon: bool,
    pub has_morph: bool,
}

fn child<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    node.object_children()
        .iter()
        .find(|c| c.name.as_ref() == name)
}

fn scalar_text(node: &Node) -> Option<String> {
    match &node.value {
        Some(ImageValue::Int(v)) => Some(v.to_string()),
        Some(ImageValue::Short(v)) => Some(v.to_string()),
        Some(ImageValue::Long(v)) => Some(v.to_string()),
        Some(ImageValue::Uint(v)) => Some(v.to_string()),
        Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => Some(s.to_string()),
        _ => None,
    }
}

fn morph_spec(item: &Node) -> String {
    let raw = child(item, "spec")
        .and_then(|spec| child(spec, "morph"))
        .and_then(scalar_text)
        .unwrap_or_default();
    format!("{raw:0>4}")
}

fn find_image_node(archive: &WzArchive, tree: &DirTree, image: &str) -> Option<Node> {
    let entry = tree
        .root
        .entries
        .iter()
        .find(|e| e.name.as_deref() == Some(image))?;
    archive.parse_image_at(entry.offset?).ok()
}

/// Morph 그리드 행을 만든다.
pub fn morph_grid_rows(
    consume_archive: &WzArchive,
    consume_tree: &DirTree,
    morph_archive: &WzArchive,
    morph_tree: &DirTree,
    linker: &StringLinker,
) -> Vec<Db2MorphRow> {
    let Some(items) = find_image_node(consume_archive, consume_tree, "0221.img") else {
        return Vec::new();
    };
    let mut morph_cache: HashMap<String, bool> = HashMap::new();
    let mut rows = Vec::new();
    for item in items.object_children() {
        let id = item.name.to_string();
        let Some(item_id) = id.parse::<i32>().ok() else {
            continue;
        };
        let sr = linker.string_item.get(&item_id);
        let morph_id = morph_spec(item);
        let has_morph = *morph_cache.entry(morph_id.clone()).or_insert_with(|| {
            let image = format!("{morph_id}.img");
            find_image_node(morph_archive, morph_tree, &image)
                .and_then(|node| child(&node, "walk").map(|walk| child(walk, "0").is_some()))
                .unwrap_or(false)
        });
        rows.push(Db2MorphRow {
            id,
            name: sr.map(|sr| sr.name.clone()).unwrap_or_default(),
            desc: sr.and_then(|sr| sr.desc.clone()).unwrap_or_default(),
            morph_id,
            has_icon: child(item, "info")
                .and_then(|info| child(info, "icon"))
                .is_some(),
            has_morph,
        });
    }
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn morph_id_is_zero_padded() {
        let mut item = Node::new("02210000");
        let mut spec = Node::new("spec");
        let mut morph = Node::new("morph");
        morph.value = Some(ImageValue::Int(5));
        spec.children.push(morph);
        item.children.push(spec);
        assert_eq!(morph_spec(&item), "0005");
        assert_eq!(morph_spec(&Node::new("02219999")), "0000");
    }
}
