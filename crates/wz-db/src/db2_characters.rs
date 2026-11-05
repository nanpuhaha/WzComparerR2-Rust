//! DB2 Character 장비 그리드 (`DB2Form.LoadCharacter` 일반 분기).
//!
//! `Character/<Dir>/<id>.img`마다 Eqp 이름/설명과 info의 주요 장비 스탯을
//! 행으로 만든다. 원본은 info 전체를 하나의 텍스트 열로 합치지만, 여기서는
//! 비교 가능한 핵심 필드를 개별 컬럼으로 노출한다.

use crate::linker::StringLinker;
use serde::{Deserialize, Serialize};
use wz_core::dirtree::DirTree;
use wz_core::image::{ImageValue, Node};
use wz_core::WzArchive;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Db2CharacterRow {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub h1: String,
    pub has_icon: bool,
    pub req_level: i32,
    pub req_job: i32,
    pub tuc: i32,
    pub islot: String,
    pub vslot: String,
    pub inc_str: i32,
    pub inc_dex: i32,
    pub inc_int: i32,
    pub inc_luk: i32,
    pub inc_pad: i32,
    pub inc_mad: i32,
    pub inc_pdd: i32,
    pub inc_mdd: i32,
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

/// 한 Character 장비 WZ 의 이미지들을 DB2 Character 행으로 만든다.
pub fn character_grid_rows(
    archive: &WzArchive,
    tree: &DirTree,
    linker: &StringLinker,
) -> Vec<Db2CharacterRow> {
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
        rows.push(Db2CharacterRow {
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
            vslot: info.map(|info| string(info, "vslot")).unwrap_or_default(),
            inc_str: info.map(|info| int(info, "incSTR")).unwrap_or(0),
            inc_dex: info.map(|info| int(info, "incDEX")).unwrap_or(0),
            inc_int: info.map(|info| int(info, "incINT")).unwrap_or(0),
            inc_luk: info.map(|info| int(info, "incLUK")).unwrap_or(0),
            inc_pad: info.map(|info| int(info, "incPAD")).unwrap_or(0),
            inc_mad: info.map(|info| int(info, "incMAD")).unwrap_or(0),
            inc_pdd: info.map(|info| int(info, "incPDD")).unwrap_or(0),
            inc_mdd: info.map(|info| int(info, "incMDD")).unwrap_or(0),
        });
    }
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_stats() {
        let mut node = Node::new("01040000.img");
        let mut info = Node::new("info");
        for (key, value) in [
            ("icon", ImageValue::Int(0)),
            ("reqLevel", ImageValue::Int(100)),
            ("tuc", ImageValue::Int(7)),
            ("incSTR", ImageValue::Int(5)),
            ("incPAD", ImageValue::Int(3)),
        ] {
            let mut c = Node::new(key);
            c.value = Some(value);
            info.children.push(c);
        }
        for (key, value) in [("islot", "Cp"), ("vslot", "CpH1H5")] {
            let mut c = Node::new(key);
            c.value = Some(ImageValue::Str(value.into()));
            info.children.push(c);
        }
        node.children.push(info);
        let info = child(&node, "info").unwrap();
        assert_eq!(int(info, "reqLevel"), 100);
        assert_eq!(int(info, "incSTR"), 5);
        assert_eq!(string(info, "islot"), "Cp");
    }
}
