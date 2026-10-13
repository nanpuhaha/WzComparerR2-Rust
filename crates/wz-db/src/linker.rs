//! 문자열 링커(`StringLinker.cs` 이식) — 아이템/장비/스킬/맵/몬스터/NPC ID별 이름·설명 인덱싱.
//!
//! `String.wz` 내부의 `Consume.img`, `Cash.img`, `Ins.img`, `Pet.img`, `Etc.img`,
//! `Eqp.img`, `Skill.img`, `Map.img`, `Mob.img`, `Npc.img` 트리를 순회하며
//! UOL 참조를 자동 해석(`resolve_uol_lossy`)하고 ID별 [`StringResult`] 사전을 구축한다.

use std::collections::BTreeMap;

use crate::arena::Arena;
use crate::node::NodeId;
use crate::query::node_value_to_string;
use crate::resolve::{find_path, resolve_uol_lossy};

/// 단일 항목의 문자열 정보 (`StringResult` / `StringResultSkill`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StringResult {
    pub name: String,
    pub desc: Option<String>,
    pub auto_desc: Option<String>,
    pub street_name: Option<String>,
    pub map_name: Option<String>,
    pub pdesc: Option<String>,
    pub skill_h: Vec<String>,
    pub full_path: String,
    pub values: BTreeMap<String, String>,
}

/// WZ 문자열 테이블 인덱서 (`StringLinker.cs`).
#[derive(Debug, Clone, Default)]
pub struct StringLinker {
    pub string_eqp: BTreeMap<i32, StringResult>,
    pub string_item: BTreeMap<i32, StringResult>,
    pub string_map: BTreeMap<i32, StringResult>,
    pub string_mob: BTreeMap<i32, StringResult>,
    pub string_npc: BTreeMap<i32, StringResult>,
    pub string_skill: BTreeMap<i32, StringResult>,
    pub string_skill_str: BTreeMap<String, StringResult>,
    pub string_set_item: BTreeMap<i32, StringResult>,
}

impl StringLinker {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn clear(&mut self) {
        self.string_eqp.clear();
        self.string_item.clear();
        self.string_map.clear();
        self.string_mob.clear();
        self.string_npc.clear();
        self.string_skill.clear();
        self.string_skill_str.clear();
        self.string_set_item.clear();
    }

    pub fn has_values(&self) -> bool {
        !self.string_eqp.is_empty()
            || !self.string_item.is_empty()
            || !self.string_map.is_empty()
            || !self.string_mob.is_empty()
            || !self.string_npc.is_empty()
            || !self.string_skill.is_empty()
            || !self.string_set_item.is_empty()
    }

    /// `String.wz` 루트 노드(하위에 `Consume.img`, `Eqp.img` 등을 가진 노드)에서 문자열을 로드한다.
    pub fn load_string_wz(&mut self, arena: &Arena, string_root: NodeId) -> bool {
        for &child in arena.children(string_root) {
            let Some(name) = arena.name(child) else {
                continue;
            };
            let img_root = arena.descend_image(child);
            self.load_image(arena, name, img_root);
        }
        self.has_values()
    }

    /// 단일 문자열 이미지(`Consume.img`, `Eqp.img`, `Skill.img` 등)를 로드한다.
    pub fn load_image(&mut self, arena: &Arena, image_name: &str, image_root: NodeId) {
        let root = arena.descend_image(image_root);
        match image_name {
            "Pet.img" | "Cash.img" | "Ins.img" | "Consume.img" => {
                for &tree in arena.children(root) {
                    let Some(text) = arena.name(tree) else {
                        continue;
                    };
                    let Ok(id) = text.trim().parse::<i32>() else {
                        continue;
                    };
                    let Some(link_node) = resolve_uol_lossy(arena, tree) else {
                        continue;
                    };
                    let mut sr = self.string_item.remove(&id).unwrap_or_default();
                    if let Some(name) = get_default_string(arena, link_node, "name") {
                        sr.name = name;
                    }
                    if let Some(desc) = get_default_string(arena, link_node, "desc") {
                        sr.desc = Some(desc);
                    }
                    if let Some(autodesc) = get_default_string(arena, link_node, "autodesc") {
                        sr.auto_desc = Some(autodesc);
                    }
                    sr.full_path = format!("{image_name}/{text}");
                    add_all_values(arena, &mut sr, link_node);
                    self.string_item.insert(id, sr);
                }
            }
            "Etc.img" => {
                for &tree0 in arena.children(root) {
                    let cat_name = arena.name(tree0).unwrap_or_default();
                    for &tree in arena.children(tree0) {
                        let Some(text) = arena.name(tree) else {
                            continue;
                        };
                        let Ok(id) = text.trim().parse::<i32>() else {
                            continue;
                        };
                        let Some(link_node) = resolve_uol_lossy(arena, tree) else {
                            continue;
                        };
                        let mut sr = self.string_item.remove(&id).unwrap_or_default();
                        if let Some(name) = get_default_string(arena, link_node, "name") {
                            sr.name = name;
                        }
                        if let Some(desc) = get_default_string(arena, link_node, "desc") {
                            sr.desc = Some(desc);
                        }
                        sr.full_path = format!("{image_name}/{cat_name}/{text}");
                        add_all_values(arena, &mut sr, link_node);
                        self.string_item.insert(id, sr);
                    }
                }
            }
            "Eqp.img" => {
                for &tree0 in arena.children(root) {
                    let c0 = arena.name(tree0).unwrap_or_default();
                    for &tree1 in arena.children(tree0) {
                        let c1 = arena.name(tree1).unwrap_or_default();
                        for &tree in arena.children(tree1) {
                            let Some(text) = arena.name(tree) else {
                                continue;
                            };
                            let Ok(id) = text.trim().parse::<i32>() else {
                                continue;
                            };
                            let Some(link_node) = resolve_uol_lossy(arena, tree) else {
                                continue;
                            };
                            let mut sr = self.string_eqp.remove(&id).unwrap_or_default();
                            if let Some(name) = get_default_string(arena, link_node, "name") {
                                sr.name = name;
                            }
                            if let Some(desc) = get_default_string(arena, link_node, "desc") {
                                sr.desc = Some(desc);
                            }
                            sr.full_path = format!("{image_name}/{c0}/{c1}/{text}");
                            add_all_values(arena, &mut sr, link_node);
                            self.string_eqp.insert(id, sr);
                        }
                    }
                }
            }
            "Mob.img" => {
                for &tree in arena.children(root) {
                    let Some(text) = arena.name(tree) else {
                        continue;
                    };
                    let Ok(id) = text.trim().parse::<i32>() else {
                        continue;
                    };
                    let Some(link_node) = resolve_uol_lossy(arena, tree) else {
                        continue;
                    };
                    let mut sr = self.string_mob.remove(&id).unwrap_or_default();
                    if let Some(name) = get_default_string(arena, link_node, "name") {
                        sr.name = name;
                    }
                    sr.full_path = format!("{image_name}/{text}");
                    add_all_values(arena, &mut sr, link_node);
                    self.string_mob.insert(id, sr);
                }
            }
            "Npc.img" => {
                for &tree in arena.children(root) {
                    let Some(text) = arena.name(tree) else {
                        continue;
                    };
                    let Ok(id) = text.trim().parse::<i32>() else {
                        continue;
                    };
                    let Some(link_node) = resolve_uol_lossy(arena, tree) else {
                        continue;
                    };
                    let mut sr = self.string_npc.remove(&id).unwrap_or_default();
                    if let Some(name) = get_default_string(arena, link_node, "name") {
                        sr.name = name;
                    }
                    if let Some(func) = get_default_string(arena, link_node, "func") {
                        sr.desc = Some(func);
                    }
                    sr.full_path = format!("{image_name}/{text}");
                    add_all_values(arena, &mut sr, link_node);
                    self.string_npc.insert(id, sr);
                }
            }
            "Map.img" => {
                for &tree0 in arena.children(root) {
                    let c0 = arena.name(tree0).unwrap_or_default();
                    for &tree in arena.children(tree0) {
                        let Some(text) = arena.name(tree) else {
                            continue;
                        };
                        let Ok(id) = text.trim().parse::<i32>() else {
                            continue;
                        };
                        let Some(link_node) = resolve_uol_lossy(arena, tree) else {
                            continue;
                        };
                        let mut sr = self.string_map.remove(&id).unwrap_or_default();
                        let street = get_default_string(arena, link_node, "streetName");
                        let map = get_default_string(arena, link_node, "mapName");
                        sr.name = format!(
                            "{} : {}",
                            street.as_deref().unwrap_or_default(),
                            map.as_deref().unwrap_or_default()
                        );
                        sr.street_name = street;
                        sr.map_name = map;
                        if let Some(map_desc) = get_default_string(arena, link_node, "mapDesc") {
                            sr.desc = Some(map_desc);
                        }
                        sr.full_path = format!("{image_name}/{c0}/{text}");
                        add_all_values(arena, &mut sr, link_node);
                        self.string_map.insert(id, sr);
                    }
                }
            }
            "Skill.img" => {
                for &tree in arena.children(root) {
                    let Some(text) = arena.name(tree) else {
                        continue;
                    };
                    let Some(link_node) = resolve_uol_lossy(arena, tree) else {
                        continue;
                    };
                    let mut sr = StringResult::default();
                    if let Some(name) = get_default_string(arena, link_node, "name") {
                        sr.name = name;
                    }
                    sr.desc = get_default_string(arena, link_node, "desc");
                    sr.pdesc = get_default_string(arena, link_node, "pdesc");
                    if let Some(h) = get_default_string(arena, link_node, "h") {
                        sr.skill_h.push(h);
                    } else {
                        let mut i = 1;
                        while let Some(hi) = get_default_string(arena, link_node, &format!("h{i}"))
                        {
                            if hi.is_empty() {
                                break;
                            }
                            sr.skill_h.push(hi);
                            i += 1;
                        }
                    }
                    sr.full_path = format!("{image_name}/{text}");
                    add_all_values(arena, &mut sr, link_node);
                    if text.len() >= 7 {
                        if let Ok(id) = text.parse::<i32>() {
                            self.string_skill.insert(id, sr.clone());
                        }
                    }
                    self.string_skill_str.insert(text.to_string(), sr);
                }
            }
            _ => {}
        }
    }

    /// ID로 등록된 아이템/장비/스킬/맵/몬스터/NPC 문자열을 우선순위에 따라 조회한다.
    pub fn get_by_id(&self, id: i32) -> Option<&StringResult> {
        self.string_item
            .get(&id)
            .or_else(|| self.string_eqp.get(&id))
            .or_else(|| self.string_skill.get(&id))
            .or_else(|| self.string_map.get(&id))
            .or_else(|| self.string_mob.get(&id))
            .or_else(|| self.string_npc.get(&id))
    }

    /// 오라클 `tooltip` 검증용 텍스트(`name` 및 `desc` 개행 결합)를 반환한다.
    pub fn format_tooltip(&self, id: i32) -> Option<String> {
        let sr = self.get_by_id(id)?;
        match sr.desc.as_deref() {
            Some(desc) if !desc.is_empty() => Some(format!("{}\n{}", sr.name, desc)),
            _ => Some(sr.name.clone()),
        }
    }
}

fn get_default_string(arena: &Arena, node: NodeId, search_path: &str) -> Option<String> {
    let target = find_path(arena, node, search_path)?;
    node_value_to_string(arena.value(target))
}

fn add_all_values(arena: &Arena, sr: &mut StringResult, node: NodeId) {
    for &child in arena.children(node) {
        if let (Some(name), Some(val_str)) =
            (arena.name(child), node_value_to_string(arena.value(child)))
        {
            sr.values.insert(name.to_string(), val_str);
        }
    }
}

/// 장비(Gear) 이미지의 `info` 노드에서 스탯 프로퍼티 맵을 자연 정렬하여 추출한다.
pub fn extract_gear_stats(
    arena: &Arena,
    gear_root: NodeId,
) -> Vec<(String, String)> {
    use wz_core::text::sort_natural;

    let target_node = arena
        .children(gear_root)
        .iter()
        .copied()
        .find(|&c| arena.name(c) == Some("info"))
        .unwrap_or(gear_root);

    let mut entries = Vec::new();
    for &child in arena.children(target_node) {
        if let (Some(name), Some(val_str)) =
            (arena.name(child), node_value_to_string(arena.value(child)))
        {
            entries.push((name.to_string(), val_str));
        }
    }

    sort_natural(&mut entries, |(k, _)| k.as_str());
    entries
}
