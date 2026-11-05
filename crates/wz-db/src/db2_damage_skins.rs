//! DB2 DamageSkin 그리드 (`DB2Form.LoadDamageSkin`).
//!
//! `String/Consume.img`의 이름에 스킨 키워드가 있는 항목을 대상으로,
//! `Item/Consume/0243.img` / `0263.img`의 아이콘과 샘플 존재 여부를 행으로
//! 만든다. 원본과 같이 0263이 있으면 아이콘을 덮어쓴다.

use crate::linker::StringLinker;
use serde::{Deserialize, Serialize};
use wz_core::dirtree::DirTree;
use wz_core::image::Node;
use wz_core::WzArchive;

const ICON_IMAGES: [&str; 2] = ["0243.img", "0263.img"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Db2DamageSkinRow {
    pub id: String,
    pub name: String,
    pub desc: String,
    /// 마지막으로 발견한 아이콘 이미지(`0243`/`0263`).
    pub icon_image: Option<String>,
    pub has_sample: bool,
}

fn has_name_keyword(name: &str) -> bool {
    name.contains("스킨")
        || name.contains("Damage Skin")
        || name.contains("字型")
        || name.contains("ジスキン")
        || name.contains("伤害皮肤")
}

fn child<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    node.object_children()
        .iter()
        .find(|c| c.name.as_ref() == name)
}

fn parse_icon_images(archive: &WzArchive, tree: &DirTree) -> Vec<(&'static str, Node)> {
    let mut out = Vec::new();
    for image in ICON_IMAGES {
        let Some(entry) = tree
            .root
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some(image))
        else {
            continue;
        };
        let Some(offset) = entry.offset else {
            continue;
        };
        if let Ok(node) = archive.parse_image_at(offset) {
            out.push((image.trim_end_matches(".img"), node));
        }
    }
    out
}

/// DamageSkin 그리드 행을 만든다.
pub fn damage_skin_rows(
    archive: &WzArchive,
    tree: &DirTree,
    linker: &StringLinker,
) -> Vec<Db2DamageSkinRow> {
    let images = parse_icon_images(archive, tree);
    let mut rows = Vec::new();
    for (string_id, sr) in &linker.string_item {
        if !sr.full_path.contains("Consume.img") || !has_name_keyword(&sr.name) {
            continue;
        }
        let id = format!("0{string_id}");
        let mut icon_image = None;
        let mut has_sample = false;
        for (image_name, image_node) in &images {
            let Some(item) = child(image_node, &id) else {
                continue;
            };
            let Some(info) = child(item, "info") else {
                continue;
            };
            if child(info, "icon").is_some() {
                icon_image = Some((*image_name).to_string());
            }
            if child(info, "sample").is_some() {
                has_sample = true;
            }
        }
        rows.push(Db2DamageSkinRow {
            id,
            name: sr.name.clone(),
            desc: sr.desc.clone().unwrap_or_default(),
            icon_image,
            has_sample,
        });
    }
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyword_filter_matches_original_languages() {
        assert!(has_name_keyword("데미지 스킨 (블루)"));
        assert!(has_name_keyword("Damage Skin (Blue)"));
        assert!(has_name_keyword("ダメージジスキン"));
        assert!(!has_name_keyword("빨간 포션"));
    }
}
