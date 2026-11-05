//! CharaSim 스탯 집계 — 장비 기본값 + 세트 효과 + 잠재 옵션.
//!
//! C# `CharaSim/Gear.cs`, `SetItem.cs`, `Potential.cs`의 데이터 모델을
//! 정적 프리뷰에 필요한 범위로 옮긴 것이다. 스타포스는 C#과 동일하게
//! 최대 별 개수/표시만 계산하고, 강화 수치 자체는 WZ에 없으므로 바꾸지 않는다.

use std::collections::{BTreeMap, BTreeSet};

use wz_core::image::{ImageValue, Node};
use wz_core::WzArchive;

use crate::loader::{item_id_to_slot, slot_dir_name};

/// 호출자가 공급하는 잠재/스타포스 값.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StatOptions {
    /// `(ItemOption code, level)`.
    pub potentials: Vec<(i32, i32)>,
    /// `(item_id, star)` — 표시용. 스탯 강화치는 적용하지 않는다.
    pub starforce: Vec<(i32, i32)>,
}

/// 세트 효과 적용 상태.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetStatus {
    pub name: String,
    pub count: usize,
    pub complete: usize,
}

/// 집계된 캐릭터 스탯.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CharacterStats {
    pub props: BTreeMap<String, i64>,
    pub sets: Vec<SetStatus>,
    /// `(item_id, max_star)`.
    pub max_star: Vec<(i32, i32)>,
}

impl CharacterStats {
    pub fn get(&self, key: &str) -> i64 {
        self.props.get(key).copied().unwrap_or(0)
    }

    pub fn summary(&self) -> String {
        let keys = [
            ("incSTR", "STR"),
            ("incDEX", "DEX"),
            ("incINT", "INT"),
            ("incLUK", "LUK"),
            ("incMHP", "HP"),
            ("incMMP", "MP"),
            ("incPAD", "PAD"),
            ("incMAD", "MAD"),
            ("incPDD", "PDD"),
            ("incMDD", "MDD"),
            ("incACC", "ACC"),
            ("incEVA", "EVA"),
        ];
        keys.iter()
            .filter_map(|(key, label)| {
                let value = self.get(key);
                (value != 0).then(|| format!("{label} +{value}"))
            })
            .collect::<Vec<_>>()
            .join(", ")
    }
}

fn scalar(node: &Node) -> Option<i64> {
    match &node.value {
        Some(ImageValue::Int(v)) => Some(i64::from(*v)),
        Some(ImageValue::Short(v)) => Some(i64::from(*v)),
        Some(ImageValue::Long(v)) => Some(*v),
        Some(ImageValue::Uint(v)) => Some(i64::from(*v)),
        Some(ImageValue::Str(v)) => v.parse().ok(),
        _ => None,
    }
}

fn child<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    node.object_children()
        .iter()
        .find(|c| c.name.as_ref() == name)
}

fn path<'a>(node: &'a Node, path: &str) -> Option<&'a Node> {
    let mut cur = node;
    for seg in path.split('/').filter(|s| !s.is_empty()) {
        cur = child(cur, seg)?;
    }
    Some(cur)
}

fn add_prop(props: &mut BTreeMap<String, i64>, key: &str, value: i64) {
    match key {
        "incAllStat" => {
            for stat in ["incSTR", "incDEX", "incINT", "incLUK"] {
                *props.entry(stat.to_string()).or_default() += value;
            }
        }
        _ => {
            *props.entry(key.to_string()).or_default() += value;
        }
    }
}

fn add_node_props(props: &mut BTreeMap<String, i64>, node: &Node) {
    for child in node.object_children() {
        if let Some(value) = scalar(child) {
            add_prop(props, child.name.as_ref(), value);
        }
    }
}

fn find_image_node(data_root: &std::path::Path, slot: &str, item_id: i32) -> Option<Node> {
    let dir = slot_dir_name(slot).or_else(|| item_id_to_slot(item_id))?;
    let base = data_root.join("Character").join(dir);
    let image = format!("{item_id:08}.img");
    for entry in std::fs::read_dir(base).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wz") {
            continue;
        }
        let Ok(archive) = WzArchive::open(&path) else {
            continue;
        };
        let Ok(tree) = archive.read_tree() else {
            continue;
        };
        let Some(entry) = tree
            .root
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some(image.as_str()))
        else {
            continue;
        };
        let Some(offset) = entry.offset else {
            continue;
        };
        if let Ok(node) = archive.parse_image_at(offset) {
            return Some(node);
        }
    }
    None
}

fn find_etc_image(data_root: &std::path::Path, image_name: &str) -> Option<Node> {
    let base = data_root.join("Etc");
    for entry in std::fs::read_dir(base).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wz") {
            continue;
        }
        let Ok(archive) = WzArchive::open(&path) else {
            continue;
        };
        let Ok(tree) = archive.read_tree() else {
            continue;
        };
        let Some(entry) = tree
            .root
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some(image_name))
        else {
            continue;
        };
        let Some(offset) = entry.offset else {
            continue;
        };
        if let Ok(node) = archive.parse_image_at(offset) {
            return Some(node);
        }
    }
    None
}

fn find_item_image(data_root: &std::path::Path, image_name: &str) -> Option<Node> {
    let base = data_root.join("Item");
    for entry in std::fs::read_dir(base).ok()?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|s| s.to_str()) != Some("wz") {
            continue;
        }
        let Ok(archive) = WzArchive::open(&path) else {
            continue;
        };
        let Ok(tree) = archive.read_tree() else {
            continue;
        };
        let Some(entry) = tree
            .root
            .entries
            .iter()
            .find(|e| e.name.as_deref() == Some(image_name))
        else {
            continue;
        };
        let Some(offset) = entry.offset else {
            continue;
        };
        if let Ok(node) = archive.parse_image_at(offset) {
            return Some(node);
        }
    }
    None
}

fn set_item_ids(set: &Node) -> BTreeSet<i32> {
    let mut ids = BTreeSet::new();
    let Some(item_ids) = child(set, "ItemID") else {
        return ids;
    };
    fn walk(node: &Node, ids: &mut BTreeSet<i32>) {
        if let Some(value) = scalar(node) {
            if value > 0 && value < i32::MAX as i64 {
                ids.insert(value as i32);
            }
            return;
        }
        for child in node.object_children() {
            walk(child, ids);
        }
    }
    walk(item_ids, &mut ids);
    ids
}

fn apply_sets(
    data_root: &std::path::Path,
    equipped: &BTreeSet<i32>,
    props: &mut BTreeMap<String, i64>,
) -> Vec<SetStatus> {
    let Some(root) = find_etc_image(data_root, "SetItemInfo.img") else {
        return Vec::new();
    };
    let mut statuses = Vec::new();
    for set in root.object_children() {
        let ids = set_item_ids(set);
        let count = ids.iter().filter(|id| equipped.contains(id)).count();
        if count == 0 {
            continue;
        }
        let name = child(set, "setItemName")
            .and_then(|n| match &n.value {
                Some(ImageValue::Str(v)) => Some(v.to_string()),
                _ => None,
            })
            .unwrap_or_else(|| set.name.to_string());
        let complete = child(set, "completeCount")
            .and_then(scalar)
            .unwrap_or(0)
            .max(0) as usize;
        if let Some(effect) = child(set, "Effect") {
            for effect_node in effect.object_children() {
                let Ok(threshold) = effect_node.name.parse::<usize>() else {
                    continue;
                };
                if threshold <= count {
                    add_node_props(props, effect_node);
                }
            }
        }
        statuses.push(SetStatus {
            name,
            count,
            complete,
        });
    }
    statuses
}

fn apply_potentials(
    data_root: &std::path::Path,
    potentials: &[(i32, i32)],
    props: &mut BTreeMap<String, i64>,
) {
    if potentials.is_empty() {
        return;
    }
    let Some(root) = find_item_image(data_root, "ItemOption.img") else {
        return;
    };
    for (code, level) in potentials {
        let Some(option) = child(&root, &format!("{code:06}")) else {
            continue;
        };
        let Some(level_node) = path(option, &format!("level/{level}")) else {
            continue;
        };
        add_node_props(props, level_node);
    }
}

fn max_star(node: &Node) -> i32 {
    let info = child(node, "info").unwrap_or(node);
    let req_level = child(info, "reqLevel").and_then(scalar).unwrap_or(0) as i32;
    let tuc = child(info, "tuc").and_then(scalar).unwrap_or(0);
    let cash = child(info, "cash").and_then(scalar).unwrap_or(0) != 0;
    let only_upgrade = child(info, "onlyUpgrade").and_then(scalar).unwrap_or(0) != 0;
    let superior = child(info, "superiorEqp").and_then(scalar).unwrap_or(0) != 0;
    if tuc <= 0 || cash || only_upgrade {
        return 0;
    }
    const STAR: &[[i32; 3]] = &[
        [0, 5, 3],
        [95, 8, 5],
        [108, 10, 8],
        [118, 15, 10],
        [128, 20, 12],
        [138, 30, 15],
    ];
    let mut value = 0;
    for row in STAR {
        if req_level >= row[0] {
            value = if superior { row[2] } else { row[1] };
        } else {
            break;
        }
    }
    value
}

/// 장비 목록의 기본 스탯 + 세트 + 잠재 + 최대 스타를 계산한다.
pub fn compute_character_stats(
    data_root: &std::path::Path,
    equipment: &[(String, i32)],
    options: &StatOptions,
) -> CharacterStats {
    let mut props = BTreeMap::new();
    let mut equipped = BTreeSet::new();
    let mut stars = Vec::new();
    for (slot, item_id) in equipment {
        if matches!(slot.as_str(), "Chair" | "Taming" | "Effect") {
            continue;
        }
        equipped.insert(*item_id);
        let Some(node) = find_image_node(data_root, slot, *item_id) else {
            continue;
        };
        add_node_props(&mut props, child(&node, "info").unwrap_or(&node));
        let star = max_star(&node);
        if star > 0 {
            stars.push((*item_id, star));
        }
    }
    let sets = apply_sets(data_root, &equipped, &mut props);
    apply_potentials(data_root, &options.potentials, &mut props);
    for (item_id, star) in &options.starforce {
        if *star > 0 {
            if let Some(entry) = stars.iter_mut().find(|(id, _)| id == item_id) {
                entry.1 = entry.1.max(*star);
            }
        }
    }
    CharacterStats {
        props,
        sets,
        max_star: stars,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn data_root() -> Option<std::path::PathBuf> {
        std::env::var_os("WZ_DATA_ROOT")
            .map(std::path::PathBuf::from)
            .filter(|p| p.exists())
    }

    #[test]
    fn base_equipment_stats_and_star_max() {
        let Some(root) = data_root() else { return };
        let equipment = vec![("Cap".to_string(), 1000000), ("Coat".to_string(), 1040000)];
        let stats = compute_character_stats(&root, &equipment, &StatOptions::default());
        assert!(!stats.props.is_empty());
        assert!(stats.props.contains_key("reqLevel") || stats.props.contains_key("price"));
    }

    #[test]
    fn set_item_effect_applies() {
        let Some(root) = data_root() else { return };
        let equipment = vec![("Ring1".to_string(), 1114302)];
        let stats = compute_character_stats(&root, &equipment, &StatOptions::default());
        assert!(!stats.sets.is_empty());
        assert!(stats.get("incSTR") > 0);
    }

    #[test]
    fn potential_option_applies() {
        let Some(root) = data_root() else { return };
        let stats = compute_character_stats(
            &root,
            &[],
            &StatOptions {
                potentials: vec![(1, 1)],
                ..Default::default()
            },
        );
        assert_eq!(stats.get("incSTR"), 1);
    }
}
