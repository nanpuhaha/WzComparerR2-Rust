//! DB2 TamingMob(Character/TamingMob) 그리드 (`DB2Form.LoadCharacter`의 TamingMob 분기).
//!
//! `Character/TamingMob/<id>.img`마다 Eqp/Taming 이름/설명과 info의 주요
//! 장비 필드를 행으로 만든다. 원본은 info 전체를 하나의 텍스트 열로 합치지만,
//! 여기서는 비교 가능한 핵심 필드를 개별 컬럼으로 노출한다.

use crate::linker::StringLinker;
use serde::{Deserialize, Serialize};
use wz_core::dirtree::DirTree;
use wz_core::image::{ImageValue, Node};
use wz_core::WzArchive;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Db2TamingMobRow {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub h1: String,
    pub has_icon: bool,
    pub req_level: i32,
    pub req_job: i32,
    pub tuc: i32,
    pub islot: String,
    pub taming_mob: i32,
}

fn child<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    node.object_children()
        .iter()
        .find(|c| c.name.as_ref() == name)
}

fn int(node: &Node, key: &str) -> i32 {
    child(node, key)
        .and_then(|c| match &c.value {
            Some(ImageValue::Int(v)) => Some(*v),
            Some(ImageValue::Short(v)) => Some(i32::from(*v)),
            Some(ImageValue::Long(v)) => Some(*v as i32),
            Some(ImageValue::Uint(v)) => Some(*v as i32),
            Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => s.parse().ok(),
            _ => None,
        })
        .unwrap_or(0)
}

fn string(node: &Node, key: &str) -> String {
    child(node, key)
        .and_then(|c| match &c.value {
            Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => Some(s.to_string()),
            _ => None,
        })
        .unwrap_or_default()
}

/// 한 TamingMob WZ 의 이미지들을 DB2 TamingMob 행으로 만든다.
pub fn taming_mob_grid_rows(
    archive: &WzArchive,
    tree: &DirTree,
    linker: &StringLinker,
) -> Vec<Db2TamingMobRow> {
    let mut rows = Vec::new();
    for entry in &tree.root.entries {
        if entry.node_type != 4 {
            continue;
        }
        let Some(name) = entry.name.as_deref() else {
            continue;
        };
        if !name.starts_with('0') {
            continue;
        }
        let Some(id) = name.strip_suffix(".img") else {
            continue;
        };
        let Ok(id_num) = id.parse::<i32>() else {
            continue;
        };
        let Some(offset) = entry.offset else {
            continue;
        };
        let Ok(node) = archive.parse_image_at(offset) else {
            continue;
        };
        let info = child(&node, "info");
        let sr = linker.string_eqp.get(&id_num);
        rows.push(Db2TamingMobRow {
            id: id.to_string(),
            name: sr.map(|sr| sr.name.clone()).unwrap_or_default(),
            desc: sr.and_then(|sr| sr.desc.clone()).unwrap_or_default(),
            h1: sr
                .and_then(|sr| sr.values.get("h1").cloned())
                .unwrap_or_default(),
            has_icon: info.and_then(|info| child(info, "icon")).is_some(),
            req_level: info.map(|info| int(info, "reqLevel")).unwrap_or(0),
            req_job: info.map(|info| int(info, "reqJob")).unwrap_or(0),
            tuc: info.map(|info| int(info, "tuc")).unwrap_or(0),
            islot: info.map(|info| string(info, "islot")).unwrap_or_default(),
            taming_mob: info.map(|info| int(info, "tamingMob")).unwrap_or(0),
        });
    }
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_info_fields() {
        let mut node = Node::new("01902000.img");
        let mut info = Node::new("info");
        for (key, value) in [
            ("icon", ImageValue::Int(0)),
            ("reqLevel", ImageValue::Int(60)),
            ("reqJob", ImageValue::Int(0)),
            ("tuc", ImageValue::Int(0)),
            ("tamingMob", ImageValue::Int(1)),
        ] {
            let mut c = Node::new(key);
            c.value = Some(value);
            info.children.push(c);
        }
        let mut islot = Node::new("islot");
        islot.value = Some(ImageValue::Str("Tm".into()));
        info.children.push(islot);
        node.children.push(info);

        let info = child(&node, "info").unwrap();
        assert_eq!(int(info, "reqLevel"), 60);
        assert_eq!(int(info, "tamingMob"), 1);
        assert_eq!(string(info, "islot"), "Tm");
    }
}
