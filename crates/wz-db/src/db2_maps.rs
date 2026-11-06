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

/// 원본 `LoadMap(Part)`가 순회하는 `Map/Map` 하위 디렉터리.
///
/// 탭 17(`Map1`) = Map0..Map3, 탭 18(`Map2`) = Map4..Map8,
/// 탭 19(`Map3`) = Map9.
pub const MAP_PARTITIONS: [&[&str]; 3] = [
    &["Map0", "Map1", "Map2", "Map3"],
    &["Map4", "Map5", "Map6", "Map7", "Map8"],
    &["Map9"],
];

fn wz_files(dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut paths: Vec<_> = std::fs::read_dir(dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("wz"))
        .collect();
    paths.sort();
    paths
}

struct MapEntry {
    dir: String,
    own_minimap: bool,
    link: Option<i32>,
}

/// 원본 `LoadMap(Part)` 의미 그대로 한 탭의 행을 만든다.
///
/// `String/Map.img`의 이름을 쓰고, 미니맵은 자기 `miniMap/canvas`가 없으면
/// `info/link` 대상(`Map{링크 첫자리}/<link>.img`)의 캔버스로 채운다.
pub fn map_partition_rows(
    data_root: &std::path::Path,
    part: usize,
    linker: &StringLinker,
) -> Vec<Db2MapRow> {
    let Some(dirs) = MAP_PARTITIONS.get(part.wrapping_sub(1)) else {
        return Vec::new();
    };
    // 1패스: 모든 파티션의 맵을 읽어 (자체 미니맵, 링크)를 모은다.
    let mut entries: std::collections::HashMap<i32, MapEntry> = std::collections::HashMap::new();
    for dir in MAP_PARTITIONS.iter().flat_map(|part| part.iter()) {
        for path in wz_files(&data_root.join("Map/Map").join(dir)) {
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
                let Some(offset) = entry.offset else {
                    continue;
                };
                let Ok(node) = archive.parse_image_at(offset) else {
                    continue;
                };
                entries.insert(
                    id,
                    MapEntry {
                        dir: (*dir).to_string(),
                        own_minimap: child(&node, "miniMap")
                            .and_then(|mini| child(mini, "canvas"))
                            .is_some(),
                        link: child(&node, "info").and_then(|info| int(info, "link")),
                    },
                );
            }
        }
    }

    // 2패스: 요청한 탭의 행만 만들고 링크로 미니맵을 보완한다.
    let mut rows = Vec::new();
    for dir in *dirs {
        for path in wz_files(&data_root.join("Map/Map").join(dir)) {
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
                if id_text.is_empty() || !id_text.chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }
                let Ok(id) = id_text.parse::<i32>() else {
                    continue;
                };
                let Some(own) = entries.get(&id) else {
                    continue;
                };
                let (street_name, map_name) = linker
                    .string_map
                    .get(&id)
                    .map(|sr| {
                        (
                            sr.street_name.clone().unwrap_or_default(),
                            sr.map_name.clone().unwrap_or_default(),
                        )
                    })
                    .unwrap_or_default();
                let mut has_minimap = own.own_minimap;
                if !has_minimap {
                    if let Some(link) = own.link {
                        let first = link.to_string();
                        let target_dir = format!("Map{}", &first[..1]);
                        if let Some(target) = entries.get(&link) {
                            has_minimap = target.dir == target_dir && target.own_minimap;
                        }
                    }
                }
                rows.push(Db2MapRow {
                    id: id_text.to_string(),
                    street_name,
                    map_name,
                    has_minimap,
                    link: own.link,
                });
            }
        }
    }
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    rows
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
