//! `SetItem.CreateFromNode` — 세트 아이템 데이터 모델.
//!
//! GDI와 무관한 노드 파싱/그룹 결합만 옮겼다. 세트 효과의 `Option`은 잠재능력
//! 노드를 참조하므로 `option_root`(ItemOption.img)가 필요하다.

use std::collections::BTreeMap;
use wz_core::image::{ImageValue, Node};

fn child<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    node.object_children()
        .iter()
        .find(|c| c.name.as_ref() == name)
}

pub fn node_i64(node: &Node) -> Option<i64> {
    match &node.value {
        Some(ImageValue::Int(v)) => Some(i64::from(*v)),
        Some(ImageValue::Short(v)) => Some(i64::from(*v)),
        Some(ImageValue::Long(v)) => Some(*v),
        Some(ImageValue::Uint(v)) => Some(i64::from(*v)),
        Some(ImageValue::Float(v)) => Some(f64::from(*v) as i64),
        Some(ImageValue::Double(v)) => Some(*v as i64),
        Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => s.parse().ok(),
        _ => None,
    }
}

pub fn node_string(node: &Node) -> Option<String> {
    match &node.value {
        Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => Some(s.to_string()),
        Some(ImageValue::Int(v)) => Some(v.to_string()),
        Some(ImageValue::Short(v)) => Some(v.to_string()),
        Some(ImageValue::Long(v)) => Some(v.to_string()),
        Some(ImageValue::Uint(v)) => Some(v.to_string()),
        Some(ImageValue::Float(v)) => Some(v.to_string()),
        Some(ImageValue::Double(v)) => Some(v.to_string()),
        _ => None,
    }
}

/// 세트 효과 `Option`이 가리키는 잠재능력.
pub type PotentialRef = wz_db::potential::PotentialOption;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetItemOptionToMob {
    pub mobs: Vec<i32>,
    pub mob_name: Option<String>,
    pub props: BTreeMap<String, i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetItemActiveSkill {
    pub skill_id: i32,
    pub level: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetItemBonusByTime {
    pub term_start: i32,
    pub props: BTreeMap<String, i64>,
}

/// `SetItemEffect.Props`의 값.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetProp {
    Int(i64),
    Option(Vec<PotentialRef>),
    OptionToMob(Vec<SetItemOptionToMob>),
    ActiveSkill(Vec<SetItemActiveSkill>),
    BonusByTime(Vec<SetItemBonusByTime>),
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetItemEffect {
    pub props: BTreeMap<String, SetProp>,
    pub enabled: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetItemIDPart {
    pub item_ids: BTreeMap<i32, bool>,
    pub represent_name: Option<String>,
    pub type_name: Option<String>,
    pub by_gender: bool,
}

impl SetItemIDPart {
    fn single(item_id: i32) -> Self {
        let mut part = Self::default();
        part.item_ids.insert(item_id, false);
        part
    }

    fn from_ids(ids: &[i32]) -> Self {
        let mut part = Self::default();
        for id in ids {
            part.item_ids.insert(*id, false);
        }
        part
    }

    pub fn enabled(&self) -> bool {
        self.item_ids.values().any(|v| *v)
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetItemIDList {
    pub parts: Vec<(i32, SetItemIDPart)>,
}

impl SetItemIDList {
    pub fn add(&mut self, part_id: i32, part: SetItemIDPart) {
        self.parts.push((part_id, part));
    }

    pub fn remove(&mut self, part_id: i32) {
        self.parts.retain(|(id, _)| *id != part_id);
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SetItem {
    pub set_item_id: i32,
    pub complete_count: i32,
    pub parts: bool,
    pub expand_tool_tip: bool,
    pub item_ids: SetItemIDList,
    pub set_item_name: Option<String>,
    pub effects: BTreeMap<i32, SetItemEffect>,
}

/// `Enum.TryParse<GearPropType>` — 원본 enum 이름 집합.
fn is_gear_prop_type(name: &str) -> bool {
    matches!(
        name,
        "incSTR"
            | "incDEX"
            | "incINT"
            | "incLUK"
            | "incAllStat"
            | "incMHP"
            | "incMMP"
            | "incMDF"
            | "incPAD"
            | "incMAD"
            | "incPDD"
            | "incMDD"
            | "incACC"
            | "incEVA"
            | "incSpeed"
            | "incJump"
            | "incCraft"
            | "incDAMr"
            | "incCr"
            | "incCDr"
            | "incEXPr"
            | "incBDR"
            | "incIMDR"
            | "incSTRr"
            | "incDEXr"
            | "incINTr"
            | "incLUKr"
            | "incMHPr"
            | "incMMPr"
            | "incPADr"
            | "incMADr"
            | "incPDDr"
            | "incMDDr"
            | "incACCr"
            | "incEVAr"
            | "incARC"
            | "incAUT"
            | "incCHUC"
            | "damR"
            | "bdR"
            | "imdR"
            | "nbdR"
            | "statR"
            | "incPQEXPr"
            | "knockback"
            | "attackSpeed"
            | "tradeBlock"
            | "equipTradeBlock"
            | "only"
            | "onlyEquip"
            | "accountSharable"
            | "sharableOnce"
            | "notExtend"
            | "accountSharableAfterExchange"
            | "noPrism"
            | "mintable"
            | "tradeAvailable"
            | "accountShareTag"
            | "noPotential"
            | "fixedPotential"
            | "superiorEqp"
            | "nActivatedSocket"
            | "jokerToSetItem"
            | "abilityTimeLimited"
            | "blockGoldHammer"
            | "blockAdditionalPotential"
            | "noLookChange"
            | "reissueBan"
            | "timeLimited"
            | "reduceReq"
            | "limitBreak"
            | "Etuc"
            | "CuttableCount"
            | "cash"
            | "quest"
            | "pquest"
            | "useTradeBlock"
            | "exchangeableOnce"
            | "multiPet"
            | "permanent"
            | "Option"
            | "OptionToMob"
            | "activeSkill"
            | "bonusByTime"
            | "face"
            | "partJob"
            | "exceptToadsHammer"
            | "exceptTransmission"
            | "protoType"
            | "setItemID"
            | "setItemName"
            | "setItemCaption"
            | "jokerPossible"
            | "setItemOptionLevel"
            | "reqLevel"
            | "reqLEVEL"
            | "weight"
            | "string"
    )
}

fn parse_gear_prop_int(node: &Node) -> Option<(String, i64)> {
    let name = node.name.to_string();
    if !is_gear_prop_type(&name) {
        return None;
    }
    Some((name, node_i64(node).unwrap_or(0)))
}

/// `SetItem.CreateFromNode(setItemNode, optionNode)`.
pub fn create_from_node(set_node: &Node, option_root: Option<&Node>) -> Option<SetItem> {
    let mut set_item = SetItem {
        set_item_id: set_node.name.as_ref().parse().unwrap_or(0),
        ..Default::default()
    };
    let mut desc: Vec<(String, String)> = Vec::new();

    for sub in set_node.object_children() {
        match sub.name.as_ref() {
            "setItemName" => set_item.set_item_name = node_string(sub),
            "completeCount" => set_item.complete_count = node_i64(sub).unwrap_or(0) as i32,
            "parts" => set_item.parts = node_i64(sub).unwrap_or(0) != 0,
            "expandToolTip" => set_item.expand_tool_tip = node_i64(sub).unwrap_or(0) != 0,
            "ItemID" => {
                for item_node in sub.object_children() {
                    let Ok(idx) = item_node.name.as_ref().parse::<i32>() else {
                        continue;
                    };
                    if item_node.object_children().is_empty() {
                        let item_id = node_i64(item_node).unwrap_or(0) as i32;
                        set_item.item_ids.add(idx, SetItemIDPart::single(item_id));
                    } else {
                        let mut part = SetItemIDPart::default();
                        for item_node2 in item_node.object_children() {
                            match item_node2.name.as_ref() {
                                "representName" => part.represent_name = node_string(item_node2),
                                "typeName" => part.type_name = node_string(item_node2),
                                "byGender" => {
                                    part.by_gender = node_i64(item_node2).unwrap_or(0) != 0
                                }
                                other => {
                                    if other.parse::<i32>().map(|i| i >= 0).unwrap_or(false) {
                                        let id = node_i64(item_node2).unwrap_or(0) as i32;
                                        part.item_ids.insert(id, false);
                                    }
                                }
                            }
                        }
                        set_item.item_ids.add(idx, part);
                    }
                }
            }
            "Effect" => {
                for effect_node in sub.object_children() {
                    let count = effect_node.name.as_ref().parse::<i32>().unwrap_or(0);
                    let mut effect = SetItemEffect::default();
                    for prop_node in effect_node.object_children() {
                        match prop_node.name.as_ref() {
                            "Option" => {
                                if let Some(option_root) = option_root {
                                    let mut potens = Vec::new();
                                    for p_node in prop_node.object_children() {
                                        let option_id =
                                            child(p_node, "option").and_then(node_i64).unwrap_or(0)
                                                as i32;
                                        let opt_text = format!("{option_id:06}");
                                        let Some(opn) = child(option_root, &opt_text) else {
                                            continue;
                                        };
                                        let level =
                                            child(p_node, "level").and_then(node_i64).unwrap_or(0)
                                                as i32;
                                        if let Some(potential) =
                                            wz_db::potential_from_node(opn, level)
                                        {
                                            potens.push(potential);
                                        }
                                    }
                                    effect
                                        .props
                                        .insert("Option".to_string(), SetProp::Option(potens));
                                }
                            }
                            "OptionToMob" => {
                                let mut list = Vec::new();
                                let mut i = 1;
                                while let Some(opt_node) = child(prop_node, &i.to_string()) {
                                    let mut option = SetItemOptionToMob::default();
                                    for p_node in opt_node.object_children() {
                                        match p_node.name.as_ref() {
                                            "mob" => {
                                                for mob_node in p_node.object_children() {
                                                    option.mobs.push(
                                                        node_i64(mob_node).unwrap_or(0) as i32,
                                                    );
                                                }
                                            }
                                            "mobName" => option.mob_name = node_string(p_node),
                                            _ => {
                                                if let Some((key, value)) =
                                                    parse_gear_prop_int(p_node)
                                                {
                                                    option.props.insert(key, value);
                                                }
                                            }
                                        }
                                    }
                                    list.push(option);
                                    i += 1;
                                }
                                effect
                                    .props
                                    .insert("OptionToMob".to_string(), SetProp::OptionToMob(list));
                            }
                            "activeSkill" => {
                                let mut list = Vec::new();
                                let mut i = 0;
                                while let Some(opt_node) = child(prop_node, &i.to_string()) {
                                    let mut skill = SetItemActiveSkill::default();
                                    for p_node in opt_node.object_children() {
                                        match p_node.name.as_ref() {
                                            "id" => {
                                                skill.skill_id =
                                                    node_i64(p_node).unwrap_or(0) as i32
                                            }
                                            "level" => {
                                                skill.level = node_i64(p_node).unwrap_or(0) as i32
                                            }
                                            _ => {}
                                        }
                                    }
                                    list.push(skill);
                                    i += 1;
                                }
                                effect
                                    .props
                                    .insert("activeSkill".to_string(), SetProp::ActiveSkill(list));
                            }
                            "bonusByTime" => {
                                let mut list = Vec::new();
                                let mut i = 0;
                                while let Some(opt_node) = child(prop_node, &i.to_string()) {
                                    let mut bonus = SetItemBonusByTime::default();
                                    for p_node in opt_node.object_children() {
                                        match p_node.name.as_ref() {
                                            "termStart" => {
                                                bonus.term_start =
                                                    node_i64(p_node).unwrap_or(0) as i32
                                            }
                                            _ => {
                                                if let Some((key, value)) =
                                                    parse_gear_prop_int(p_node)
                                                {
                                                    bonus.props.insert(key, value);
                                                }
                                            }
                                        }
                                    }
                                    list.push(bonus);
                                    i += 1;
                                }
                                effect
                                    .props
                                    .insert("bonusByTime".to_string(), SetProp::BonusByTime(list));
                            }
                            _ => {
                                if let Some((key, value)) = parse_gear_prop_int(prop_node) {
                                    effect.props.insert(key, SetProp::Int(value));
                                }
                            }
                        }
                    }
                    set_item.effects.insert(count, effect);
                }
            }
            "Desc" => {
                for desc_node in sub.object_children() {
                    if let Some(value) = node_string(desc_node) {
                        desc.push((desc_node.name.to_string(), value));
                    }
                }
            }
            _ => {}
        }
    }

    // 추가 그룹 결합
    for (key, value) in desc {
        let (combine, type_name) = match key.as_str() {
            "weapon" => (
                combine_part(&mut set_item, |id| {
                    wz_db::is_weapon(wz_db::get_gear_type(id))
                }),
                wz_db::gear_type_string("weapon"),
            ),
            "subweapon" => (
                combine_part(&mut set_item, |id| {
                    wz_db::is_sub_weapon(wz_db::get_gear_type(id))
                }),
                wz_db::gear_type_string("subWeapon"),
            ),
            "pocket" => (
                combine_part(&mut set_item, |id| {
                    wz_db::get_gear_type(id) == wz_db::gear_type_value("pocket").unwrap()
                }),
                wz_db::gear_type_string("pocket"),
            ),
            "emblem" => (
                combine_part(&mut set_item, |id| {
                    wz_db::get_gear_type(id) == wz_db::gear_type_value("emblem").unwrap()
                }),
                wz_db::gear_type_string("emblem"),
            ),
            _ => (None, None),
        };
        if let Some(part_id) = combine {
            if let Some((_, part)) = set_item
                .item_ids
                .parts
                .iter_mut()
                .find(|(id, _)| *id == part_id)
            {
                part.represent_name = Some(value);
                part.type_name = type_name.map(|s| s.to_string());
            }
        }
    }

    Some(set_item)
}

/// `SetItem.CombinePart` — 조건에 맞는 장비 ID를 하나의 파트로 합친다.
fn combine_part(set_item: &mut SetItem, predicate: impl Fn(i32) -> bool) -> Option<i32> {
    let mut item_ids: Vec<i32> = Vec::new();
    let mut removed: Vec<i32> = Vec::new();
    let mut idx: Option<i32> = None;
    for (part_id, part) in &set_item.item_ids.parts {
        let mut add = false;
        for item_id in part.item_ids.keys() {
            if predicate(*item_id) {
                item_ids.push(*item_id);
                add = true;
            }
        }
        if add && !removed.contains(part_id) {
            removed.push(*part_id);
        }
        idx = Some(match idx {
            None => *part_id,
            Some(current) => current.max(*part_id),
        });
    }
    if item_ids.is_empty() {
        return None;
    }
    // `idx`는 파트가 하나도 없으면 None이므로 그 경우 0으로 둔다.
    let new_part = SetItemIDPart::from_ids(&item_ids);
    for part_id in &removed {
        set_item.item_ids.remove(*part_id);
    }
    let new_id = idx.unwrap_or(0) + 1;
    set_item.item_ids.add(new_id, new_part);
    Some(new_id)
}

/// 비교/CLI용 JSON. C# `SetItem` 구조 덤프와 같은 모양이다.
pub fn set_item_json(set: &SetItem) -> serde_json::Value {
    let item_ids: Vec<serde_json::Value> = set
        .item_ids
        .parts
        .iter()
        .map(|(part_id, part)| {
            serde_json::json!({
                "part_id": part_id,
                "represent_name": part.represent_name,
                "type_name": part.type_name,
                "by_gender": part.by_gender,
                "item_ids": part.item_ids.keys().copied().collect::<Vec<_>>(),
            })
        })
        .collect();
    let effects: Vec<serde_json::Value> = set
        .effects
        .iter()
        .map(|(count, effect)| {
            let mut props = serde_json::Map::new();
            for (key, value) in &effect.props {
                let json = match value {
                    SetProp::Int(v) => serde_json::json!(v),
                    SetProp::Option(list) => serde_json::Value::Array(
                        list.iter()
                            .map(|p| {
                                serde_json::json!({
                                    "code": p.code,
                                    "option_type": p.option_type,
                                    "req_level": p.req_level,
                                    "weight": p.weight,
                                    "summary": p.summary,
                                })
                            })
                            .collect(),
                    ),
                    SetProp::OptionToMob(list) => serde_json::Value::Array(
                        list.iter()
                            .map(|o| {
                                serde_json::json!({
                                    "mobs": o.mobs,
                                    "mob_name": o.mob_name,
                                    "props": o.props,
                                })
                            })
                            .collect(),
                    ),
                    SetProp::ActiveSkill(list) => serde_json::Value::Array(
                        list.iter()
                            .map(
                                |s| serde_json::json!({ "skill_id": s.skill_id, "level": s.level }),
                            )
                            .collect(),
                    ),
                    SetProp::BonusByTime(list) => serde_json::Value::Array(
                        list.iter()
                            .map(|b| {
                                serde_json::json!({
                                    "term_start": b.term_start,
                                    "props": b.props,
                                })
                            })
                            .collect(),
                    ),
                };
                props.insert(key.clone(), json);
            }
            serde_json::json!({
                "count": count,
                "enabled": effect.enabled,
                "props": props,
            })
        })
        .collect();
    serde_json::json!({
        "set_item_id": set.set_item_id,
        "set_item_name": set.set_item_name,
        "complete_count": set.complete_count,
        "parts": set.parts,
        "expand_tool_tip": set.expand_tool_tip,
        "item_ids": item_ids,
        "effects": effects,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wz_core::image::{ImageValue, Node};

    fn leaf(name: &str, value: i32) -> Node {
        let mut node = Node::new(std::sync::Arc::<str>::from(name));
        node.value = Some(ImageValue::Int(value));
        node
    }

    fn text(name: &str, value: &str) -> Node {
        let mut node = Node::new(std::sync::Arc::<str>::from(name));
        node.value = Some(ImageValue::Str(value.into()));
        node
    }

    fn parent(name: &str, children: Vec<Node>) -> Node {
        let mut node = Node::new(std::sync::Arc::<str>::from(name));
        node.children = children;
        node
    }

    #[test]
    fn parses_ids_and_effects() {
        let item_id = parent(
            "ItemID",
            vec![
                parent("1", vec![leaf("0", 1003154)]),
                parent(
                    "2",
                    vec![
                        text("representName", "무기"),
                        text("typeName", "무기"),
                        leaf("byGender", 1),
                        leaf("0", 1302149),
                    ],
                ),
            ],
        );
        let effect = parent(
            "Effect",
            vec![parent("1", vec![leaf("incSTR", 5), leaf("incDEX", 3)])],
        );
        let root = parent(
            "6",
            vec![
                text("setItemName", "테스트 세트"),
                leaf("completeCount", 2),
                leaf("parts", 1),
                leaf("expandToolTip", 0),
                item_id,
                effect,
            ],
        );
        let set = create_from_node(&root, None).expect("set");
        assert_eq!(set.set_item_id, 6);
        assert_eq!(set.set_item_name.as_deref(), Some("테스트 세트"));
        assert_eq!(set.complete_count, 2);
        assert!(set.parts);
        assert!(!set.expand_tool_tip);
        assert_eq!(set.item_ids.parts.len(), 2);
        assert_eq!(
            set.item_ids.parts[0]
                .1
                .item_ids
                .keys()
                .copied()
                .collect::<Vec<_>>(),
            vec![1003154]
        );
        assert!(set.item_ids.parts[1].1.by_gender);
        assert_eq!(set.effects[&1].props["incSTR"], SetProp::Int(5));
        assert_eq!(set.effects[&1].props["incDEX"], SetProp::Int(3));
    }

    #[test]
    fn desc_combines_weapon_parts() {
        let item_id = parent(
            "ItemID",
            vec![
                parent("1", vec![leaf("0", 1003154)]),
                parent("2", vec![leaf("0", 1302149)]),
            ],
        );
        let desc = parent("Desc", vec![text("weapon", "무기 중 택 1")]);
        let root = parent("7", vec![item_id, desc]);
        let set = create_from_node(&root, None).expect("set");
        // 무기 파트가 하나로 합쳐지고, 남은 파트 ID는 최대+1이 된다.
        let parts: Vec<_> = set.item_ids.parts.iter().map(|(id, _)| *id).collect();
        assert_eq!(parts, vec![1, 3]);
        let combined = &set.item_ids.parts[1].1;
        assert_eq!(
            combined.item_ids.keys().copied().collect::<Vec<_>>(),
            vec![1302149]
        );
        assert_eq!(combined.represent_name.as_deref(), Some("무기 중 택 1"));
        assert_eq!(combined.type_name.as_deref(), Some("무기"));
    }

    #[test]
    fn option_refs_use_option_root() {
        let option_root = parent(
            "ItemOption.img",
            vec![parent(
                "003091",
                vec![
                    parent("info", vec![text("string", "STR +#incSTR")]),
                    parent("level", vec![parent("1", vec![leaf("incSTR", 1)])]),
                ],
            )],
        );
        let option_effect = parent(
            "1",
            vec![parent(
                "Option",
                vec![parent("0", vec![leaf("option", 3091), leaf("level", 1)])],
            )],
        );
        let root = parent("8", vec![parent("Effect", vec![option_effect])]);
        let set = create_from_node(&root, Some(&option_root)).expect("set");
        match &set.effects[&1].props["Option"] {
            SetProp::Option(list) => {
                assert_eq!(list.len(), 1);
                assert_eq!(list[0].code, 3091);
                assert_eq!(list[0].summary.as_deref(), Some("STR +1"));
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}
