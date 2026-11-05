//! DB2 Item 탭 그리드 (`DB2Form.LoadItem`).
//!
//! `Item/<Dir>` 아래 모든 WZ 이미지의 아이템을 행으로 만들고,
//! `String/<Dir>.img`의 이름/설명, `info/icon`(Pet은 `iconD`) 존재 여부를
//! 기록한다. Special 탭은 `Etc/CashPackage.img` 목록으로 필터한다.
//!
//! 원본은 `Iter.Text.IDString()`(= `int.Parse(...).ToString()`)으로 문자열
//! 노드를 찾고, **노드가 없을 때만** `"  "`를 쓴다. 노드가 있는데 값이 비어
//! 있으면 빈 문자열이 그대로 남는다. 또 `int.Parse`는 앞뒤 공백을 허용하므로
//! `"02439339 "` 같은 ID도 `2439339`로 조회된다.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use wz_core::dirtree::DirTree;
use wz_core::image::{ImageValue, Node};
use wz_core::WzArchive;

/// 원본 `GetValue2(..., "  ")`의 기본값.
const STRING_FALLBACK: &str = "  ";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Db2ItemTabRow {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub has_icon: bool,
}

fn child<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    node.object_children()
        .iter()
        .find(|c| c.name.as_ref() == name)
}

fn scalar(node: &Node) -> Option<String> {
    match &node.value {
        Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => Some(s.to_string()),
        Some(ImageValue::Int(v)) => Some(v.to_string()),
        Some(ImageValue::Short(v)) => Some(v.to_string()),
        Some(ImageValue::Long(v)) => Some(v.to_string()),
        Some(ImageValue::Uint(v)) => Some(v.to_string()),
        _ => None,
    }
}

/// `Node.GetValue2(path, "  ")`와 같은 의미.
fn value2(node: Option<&Node>, path: &[&str]) -> String {
    let mut current = node;
    for step in path {
        current = current.and_then(|n| child(n, step));
    }
    current
        .and_then(scalar)
        .unwrap_or_else(|| STRING_FALLBACK.to_string())
}

fn wz_files(dir: &Path) -> Vec<PathBuf> {
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

fn image_entries(tree: &DirTree) -> Vec<(String, u64)> {
    tree.root
        .entries
        .iter()
        .filter(|e| e.node_type == 4)
        .filter_map(|e| Some((e.name.as_ref()?.to_string(), e.offset?)))
        .collect()
}

/// `String/<dir>.img` 노드를 같은 디렉터리의 분할 WZ에서 찾아 파싱한다.
///
/// `Etc`는 `String/Etc.img/Etc`, `Install`은 `String/Ins.img`,
/// 나머지는 `String/<dir>.img`다.
pub fn load_string_image(string_wz: &Path, dir: &str) -> Option<Node> {
    let image_name = match dir {
        "Install" => "Ins.img",
        _ => return load_named_image(string_wz, &format!("{dir}.img"), dir == "Etc"),
    };
    load_named_image(string_wz, image_name, false)
}

fn load_named_image(string_wz: &Path, image_name: &str, etc_wrapper: bool) -> Option<Node> {
    let parent = string_wz.parent()?;
    for path in wz_files(parent) {
        let Ok(archive) = WzArchive::open(&path) else {
            continue;
        };
        let Ok(tree) = archive.read_tree() else {
            continue;
        };
        let Some((_, offset)) = image_entries(&tree)
            .into_iter()
            .find(|(name, _)| name == image_name)
        else {
            continue;
        };
        let node = archive.parse_image_at(offset).ok()?;
        if etc_wrapper {
            return node
                .object_children()
                .iter()
                .find(|c| c.name.as_ref() == "Etc")
                .cloned();
        }
        return Some(node);
    }
    None
}

fn cash_packages(data_root: &Path) -> HashSet<String> {
    let mut set = HashSet::new();
    for path in wz_files(&data_root.join("Etc")) {
        let Ok(archive) = WzArchive::open(&path) else {
            continue;
        };
        let Ok(tree) = archive.read_tree() else {
            continue;
        };
        let Some((_, offset)) = image_entries(&tree)
            .into_iter()
            .find(|(name, _)| name == "CashPackage.img")
        else {
            continue;
        };
        if let Ok(node) = archive.parse_image_at(offset) {
            for c in node.object_children() {
                set.insert(c.name.to_string());
            }
        }
    }
    set
}

/// `Item/<dir>` 전체를 DB2 Item 탭 행으로 만든다.
///
/// `string_image`는 [`load_string_image`]로 얻은 `String/<dir>.img` 노드다.
/// Special 탭은 문자열 이미지를 쓰지 않으므로 `None`이어도 된다.
pub fn item_tab_rows(
    data_root: &Path,
    dir: &str,
    string_image: Option<&Node>,
) -> Vec<Db2ItemTabRow> {
    let special = dir == "Special";
    let cash = if special {
        cash_packages(data_root)
    } else {
        HashSet::new()
    };
    let mut rows = Vec::new();
    for path in wz_files(&data_root.join("Item").join(dir)) {
        let Ok(archive) = WzArchive::open(&path) else {
            continue;
        };
        let Ok(tree) = archive.read_tree() else {
            continue;
        };
        for (image_name, offset) in image_entries(&tree) {
            if special && image_name != "0910.img" {
                continue;
            }
            let Ok(image) = archive.parse_image_at(offset) else {
                continue;
            };
            // Pet 은 이미지 자체가 펫 한 마리이고 아이콘은 info/iconD 다.
            if dir == "Pet" {
                let id = image_name.trim_end_matches(".img").to_string();
                rows.push(Db2ItemTabRow {
                    id: id.clone(),
                    name: value2(string_image, &[id.as_str(), "name"]),
                    desc: value2(string_image, &[id.as_str(), "desc"]),
                    has_icon: child(&image, "info")
                        .and_then(|info| child(info, "iconD"))
                        .is_some(),
                });
                continue;
            }

            for item in image.object_children() {
                if special && !cash.contains(item.name.as_ref()) {
                    continue;
                }
                let id = item.name.to_string();
                // 원본 `IDString()` = int.Parse(...).ToString() — 공백 허용.
                let Ok(id_num) = id.trim().parse::<i32>() else {
                    continue;
                };
                let id_string = id_num.to_string();
                let (name, desc) = if special {
                    // Special 은 아이템 노드 자체의 name/desc 를 쓴다.
                    (value2(Some(item), &["name"]), value2(Some(item), &["desc"]))
                } else {
                    (
                        value2(string_image, &[id_string.as_str(), "name"]),
                        value2(string_image, &[id_string.as_str(), "desc"]),
                    )
                };
                let has_icon = if special {
                    child(item, "icon").is_some()
                } else {
                    child(item, "info")
                        .and_then(|info| child(info, "icon"))
                        .is_some()
                };
                rows.push(Db2ItemTabRow {
                    id,
                    name,
                    desc,
                    has_icon,
                });
            }
        }
    }
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    rows.dedup_by(|a, b| a.id == b.id);
    rows
}

/// ItemTooltip 속성까지 포함한 탭 행.
pub fn item_tab_tooltips(
    data_root: &Path,
    dir: &str,
    string_image: Option<&Node>,
) -> Vec<crate::item_tooltip::ItemTooltipView> {
    let special = dir == "Special";
    let cash = if special {
        cash_packages(data_root)
    } else {
        HashSet::new()
    };
    let mut rows = Vec::new();
    for path in wz_files(&data_root.join("Item").join(dir)) {
        let Ok(archive) = WzArchive::open(&path) else {
            continue;
        };
        let Ok(tree) = archive.read_tree() else {
            continue;
        };
        for (image_name, offset) in image_entries(&tree) {
            if special && image_name != "0910.img" {
                continue;
            }
            let Ok(image) = archive.parse_image_at(offset) else {
                continue;
            };
            if dir == "Pet" {
                let id = image_name.trim_end_matches(".img").to_string();
                let Ok(id_num) = id.trim().parse::<i32>() else {
                    continue;
                };
                rows.push(crate::item_tooltip::ItemTooltipView {
                    id: id.clone(),
                    name: value2(string_image, &[id.as_str(), "name"]),
                    desc: value2(string_image, &[id.as_str(), "desc"]),
                    auto_desc: String::new(),
                    attributes: crate::item_tooltip::item_attribute_lines(&image, id_num),
                });
                continue;
            }
            for item in image.object_children() {
                if special && !cash.contains(item.name.as_ref()) {
                    continue;
                }
                let id = item.name.to_string();
                let Ok(id_num) = id.trim().parse::<i32>() else {
                    continue;
                };
                let id_string = id_num.to_string();
                let (name, desc) = if special {
                    (value2(Some(item), &["name"]), value2(Some(item), &["desc"]))
                } else {
                    (
                        value2(string_image, &[id_string.as_str(), "name"]),
                        value2(string_image, &[id_string.as_str(), "desc"]),
                    )
                };
                rows.push(crate::item_tooltip::ItemTooltipView {
                    id,
                    name,
                    desc,
                    auto_desc: String::new(),
                    attributes: crate::item_tooltip::item_attribute_lines(item, id_num),
                });
            }
        }
    }
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    rows.dedup_by(|a, b| a.id == b.id);
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn value2_uses_fallback_only_when_node_missing() {
        let mut root = Node::new("Consume.img");
        let mut id = Node::new("2439339");
        let mut name = Node::new("name");
        name.value = Some(ImageValue::Str("".into()));
        id.children.push(name);
        root.children.push(id);

        assert_eq!(value2(Some(&root), &["2439339", "name"]), "");
        assert_eq!(value2(Some(&root), &["0000000", "name"]), "  ");
    }
}
