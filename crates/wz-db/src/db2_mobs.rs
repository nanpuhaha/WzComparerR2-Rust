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

/// 원본 `LoadMob(Part)`의 선행 숫자 필터.
///
/// 탭 20(`Mob`) = 8/9로 시작하지 않는 ID, 탭 21(`Mob001`) = `8`,
/// 탭 22(`Mob2`) = `9`.
pub fn mob_partition_accepts(part: usize, id: i32) -> bool {
    let first = (id / 1_000_000) % 10;
    match part {
        1 => first != 8 && first != 9,
        2 => first == 8,
        3 => first == 9,
        _ => false,
    }
}

fn all_wz_files(dir: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            all_wz_files(&path, out);
        } else if path.extension().and_then(|s| s.to_str()) == Some("wz") {
            out.push(path);
        }
    }
}

/// 원본 `LoadMob(Part)` 의미 그대로 한 탭의 행을 만든다.
///
/// `String/Mob.img`를 순회하며 `Mob/<id>.img`가 있고 선행 숫자 필터를
/// 통과하는 항목만 남긴다.
pub fn mob_partition_rows(
    data_root: &std::path::Path,
    part: usize,
    linker: &StringLinker,
) -> Vec<Db2MobRow> {
    let mut files = Vec::new();
    all_wz_files(&data_root.join("Mob"), &mut files);
    files.sort();

    let mut found: HashMap<i32, Db2MobRow> = HashMap::new();
    for path in files {
        let Ok(archive) = WzArchive::open(&path) else {
            continue;
        };
        let Ok(tree) = archive.read_tree() else {
            continue;
        };
        for entry in &tree.root.entries {
            if entry.node_type != 4 {
                continue;
            }
            let Some(name) = entry.name.as_deref() else {
                continue;
            };
            let Some(id_text) = name.strip_suffix(".img") else {
                continue;
            };
            let Ok(id) = id_text.parse::<i32>() else {
                continue;
            };
            if found.contains_key(&id) {
                continue;
            }
            let Some(offset) = entry.offset else {
                continue;
            };
            let Ok(node) = archive.parse_image_at(offset) else {
                continue;
            };
            found.insert(
                id,
                Db2MobRow {
                    id: format!("{id:07}"),
                    name: linker
                        .string_mob
                        .get(&id)
                        .map(|s| s.name.clone())
                        .unwrap_or_default(),
                    has_stand: child(&node, "stand").is_some(),
                    has_fly: child(&node, "fly").is_some(),
                    has_move: child(&node, "move").is_some(),
                    has_hit: child(&node, "hit1").is_some() || child(&node, "hit").is_some(),
                    has_die: child(&node, "die1").is_some() || child(&node, "die").is_some(),
                    has_thumbnail: child(&node, "info")
                        .and_then(|info| child(info, "thumbnail"))
                        .is_some(),
                    link: child(&node, "info").and_then(|info| int(info, "link")),
                },
            );
        }
    }

    let mut rows: Vec<Db2MobRow> = linker
        .string_mob
        .keys()
        .filter(|id| found.contains_key(id))
        .filter(|id| mob_partition_accepts(part, **id))
        .filter_map(|id| found.get(id).cloned())
        .collect();
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    rows
}

#[cfg(test)]
mod partition_tests {
    use super::*;

    #[test]
    fn partition_uses_leading_digit() {
        assert!(mob_partition_accepts(1, 100100));
        assert!(!mob_partition_accepts(1, 8645262));
        assert!(mob_partition_accepts(2, 8645262));
        assert!(mob_partition_accepts(3, 9832023));
        assert!(!mob_partition_accepts(3, 8645262));
        assert!(!mob_partition_accepts(0, 100100));
    }
}
