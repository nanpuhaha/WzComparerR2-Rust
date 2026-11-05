//! 문자열 링커(`StringLinker.cs` 이식) — 아이템/장비/스킬/맵/몬스터/NPC ID별 이름·설명 인덱싱.
//!
//! 원본은 `WzComparerR2.Common\StringLinker.cs` (커밋 `bb98c25`) 의 이식이다.
//! 한 번의 로딩은 **다섯 개의 서로 다른 WZ 루트**를 받는다 (`StringLinker.cs:47`):
//!
//! | 인자 | WZ 파일 | 여기서 읽는 것 |
//! |---|---|---|
//! | `stringNode` | `String.wz` 루트 | `Pet/Cash/Ins/Consume`, `Etc`, `Eqp`, `Mob`, `Npc`, `Map`, `Skill`, `Familiar`, `FamiliarSkill`, `MonsterBook` |
//! | `itemNode`   | `Item.wz` 루트   | `Special/0910.img` |
//! | `etcNode`    | `Etc.wz` 루트    | `SetItemInfo.img`, `Achievement/AchievementData`, `worldArchive.img` |
//! | `questNode`  | `Quest.wz` 루트  | `QuestData`, `QuestInfo.img` |
//! | `reactorNode`| `Reactor.wz` 루트| 각 `.img` 의 `info` |
//!
//! 그래서 이 모듈도 진입점을 루트별로 나눈다 — [`StringLinker::load_string_wz`],
//! [`StringLinker::load_item_wz`], [`StringLinker::load_etc_wz`],
//! [`StringLinker::load_quest_wz`], [`StringLinker::load_reactor_wz`].
//!
//! # `string_root` 가 무엇인지 (혼동하기 쉬운 지점)
//!
//! [`StringLinker::load_string_wz`] 가 받는 노드는 **`.img` 엔트리를 직접 자식으로
//! 갖는 노드**다. 원본 `Load(Wz_File stringWz, ...)` 가 `stringWz.Node` 를 넘기는데
//! (`StringLinker.cs:44`) 그 노드의 자식이 `Consume.img` 같은 엔트리이기 때문이다.
//! 즉 "`String.wz` 라는 이름의 노드를 찾아 내려간다"가 아니라 **"String.wz 파일
//! 자체의 루트"** 다. 한 단계만 더 내려가면 아무 것도 잡지 못한다.
//! `string_linker_takes_the_wz_file_root_whose_children_are_img_entries` 가 고정한다.

use std::collections::BTreeMap;

use crate::arena::Arena;
use crate::node::{NodeId, NodeValue};
use crate::query::node_value_to_string;
use crate::resolve::{find_path, resolve_uol_lossy};

/// 단일 항목의 문자열 정보 (`StringResult` / `StringResultSkill`).
///
/// `StringResult.cs` 의 필드를 그대로 옮겼다. `skill_*` 계열은 원본에서
/// 파생 클래스(`StringResultSkill`)에 있었지만 여기서는 스킬이 아닌 항목에서
/// 항상 비어 있으므로 한 구조체에 둔다.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct StringResult {
    pub name: String,
    pub desc: Option<String>,
    pub auto_desc: Option<String>,
    pub street_name: Option<String>,
    pub map_name: Option<String>,
    pub pdesc: Option<String>,
    /// `Quest_DemandBase` — `Quest.wz` 의 `demand/base`.
    pub quest_demand_base: Option<String>,
    /// `SkillH` — `h` 또는 `h1..hn`.
    pub skill_h: Vec<String>,
    /// `SkillpH` — `ph`.
    pub skill_ph: Vec<String>,
    /// `SkillhcH` — `hch`.
    pub skill_hch: Vec<String>,
    /// `SkillExtraH` — KMST1196 의 `h_<level>` 노드. 레벨 오름차순.
    pub skill_extra_h: Vec<(i32, String)>,
    pub full_path: String,
    /// `StringResult` 의 인덱서(`this[string]`)가 쌓는 모든 스칼라 자식.
    pub values: BTreeMap<String, String>,
}

/// 로딩 모드 (`StringLinker.Load` 의 `update` 인자).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadMode {
    /// `Load` — 새 [`StringResult`] 를 만들어 **덮어쓴다**.
    Load,
    /// `Update` — 기존 결과를 이어받아 병합하고 `FullPath` 는 건드리지 않는다.
    Update,
}

impl LoadMode {
    fn is_update(self) -> bool {
        matches!(self, LoadMode::Update)
    }
}

/// `StringLinker` 의 도메인별 문자열 사전.
///
/// `get_by_id` 의 fallback 순서는 편의일 뿐이고, 오라클 op 는 도메인을 명시해야
/// 한다 (ADR-0006). 이 타입이 그 도메인 이름을 타입으로 만든다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringDomain {
    Item,
    Eqp,
    Skill,
    Map,
    Mob,
    Npc,
}

impl StringDomain {
    /// `get_by_id` 가 사용하는 편의 순서 (ADR-0006).
    pub const ALL: [Self; 6] = [
        Self::Item,
        Self::Eqp,
        Self::Skill,
        Self::Map,
        Self::Mob,
        Self::Npc,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Item => "item",
            Self::Eqp => "eqp",
            Self::Skill => "skill",
            Self::Map => "map",
            Self::Mob => "mob",
            Self::Npc => "npc",
        }
    }

    pub fn parse(text: &str) -> Option<Self> {
        match text.to_ascii_lowercase().as_str() {
            "item" => Some(Self::Item),
            "eqp" | "equip" => Some(Self::Eqp),
            "skill" => Some(Self::Skill),
            "map" => Some(Self::Map),
            "mob" => Some(Self::Mob),
            "npc" => Some(Self::Npc),
            _ => None,
        }
    }
}

/// 하나의 WZ 루트와 그 노드를 가리키는 borrowed view.
#[derive(Clone, Copy)]
pub struct StringRoot<'a> {
    pub arena: &'a Arena,
    pub node: NodeId,
}

/// [`StringLinker`] 가 읽는 다섯 루트. `string` 만 필수이고 나머지는 선택이다.
#[derive(Clone, Copy)]
pub struct StringRoots<'a> {
    pub string: StringRoot<'a>,
    pub item: Option<StringRoot<'a>>,
    pub etc: Option<StringRoot<'a>>,
    pub quest: Option<StringRoot<'a>>,
    pub reactor: Option<StringRoot<'a>>,
}

/// WZ 문자열 테이블 인덱서 (`StringLinker.cs`).
#[derive(Debug, Clone, Default)]
pub struct StringLinker {
    pub string_eqp: BTreeMap<i32, StringResult>,
    pub string_familiar_skill: BTreeMap<i32, StringResult>,
    pub string_item: BTreeMap<i32, StringResult>,
    pub string_map: BTreeMap<i32, StringResult>,
    pub string_mob: BTreeMap<i32, StringResult>,
    pub string_monster_book: BTreeMap<i32, StringResult>,
    pub string_npc: BTreeMap<i32, StringResult>,
    pub string_quest: BTreeMap<i32, StringResult>,
    pub string_reactor: BTreeMap<i32, StringResult>,
    pub string_skill: BTreeMap<i32, StringResult>,
    pub string_skill_str: BTreeMap<String, StringResult>,
    pub string_set_item: BTreeMap<i32, StringResult>,
    pub string_world_archive_mob: BTreeMap<i32, StringResult>,
    pub string_world_archive_npc: BTreeMap<i32, StringResult>,
    pub string_world_archive_mob_by_path: BTreeMap<String, StringResult>,
    pub string_world_archive_npc_by_path: BTreeMap<String, StringResult>,
}

/// `worldArchive.img` 엔트리 하나를 처리하는 값 묶음.
///
/// 예전에는 `load_world_archive_entry` 가 같은 값 일곱 개를 매번 인자로 받았다.
struct WorldArchiveEntry<'a> {
    arena: &'a Arena,
    kind: WorldArchiveKind,
    entry: NodeId,
    node_text: &'a str,
    sub_text: &'a str,
    kind_node: NodeId,
    mode: LoadMode,
}

impl StringLinker {
    pub fn new() -> Self {
        Self::default()
    }

    /// 모든 테이블을 비운다 (`StringLinker.cs:709-734`).
    pub fn clear(&mut self) {
        self.string_eqp.clear();
        self.string_familiar_skill.clear();
        self.string_item.clear();
        self.string_map.clear();
        self.string_mob.clear();
        self.string_monster_book.clear();
        self.string_npc.clear();
        self.string_quest.clear();
        self.string_reactor.clear();
        self.string_skill.clear();
        self.string_skill_str.clear();
        self.string_set_item.clear();
        self.string_world_archive_mob.clear();
        self.string_world_archive_npc.clear();
        self.string_world_archive_mob_by_path.clear();
        self.string_world_archive_npc_by_path.clear();
    }

    /// 다섯 루트를 한 번에 로드한다. `string` 만 필수이고, 나머지는 주어진
    /// 것만 병합한다. 호출자는 루트→로더 매핑을 몰라도 된다.
    pub fn load_roots(&mut self, roots: &StringRoots<'_>) -> bool {
        self.clear();
        self.load_string_wz(roots.string.arena, roots.string.node);
        if let Some(r) = roots.item {
            self.load_item_wz(r.arena, r.node);
        }
        if let Some(r) = roots.etc {
            self.load_etc_wz(r.arena, r.node);
        }
        if let Some(r) = roots.quest {
            self.load_quest_wz(r.arena, r.node);
        }
        if let Some(r) = roots.reactor {
            self.load_reactor_wz(r.arena, r.node);
        }
        self.has_values()
    }

    /// `StringLinker.HasValues` (`StringLinker.cs:736-743`).
    ///
    /// 원본은 `stringFamiliarSkill` / `stringMonsterBook` / `stringWorldArchive*`
    /// 를 **세지 않는다**. 그대로 옮겼다.
    pub fn has_values(&self) -> bool {
        !self.string_eqp.is_empty()
            || !self.string_item.is_empty()
            || !self.string_map.is_empty()
            || !self.string_mob.is_empty()
            || !self.string_npc.is_empty()
            || !self.string_skill.is_empty()
            || !self.string_set_item.is_empty()
            || !self.string_quest.is_empty()
            || !self.string_reactor.is_empty()
    }

    /// `String.wz` 루트를 처음부터 읽는다 (`StringLinker.Load`).
    ///
    /// `string_root` 의 **직접 자식**이 `Consume.img` 같은 `.img` 엔트리여야 한다.
    /// 기존 내용을 먼저 비운다.
    pub fn load_string_wz(&mut self, arena: &Arena, string_root: NodeId) -> bool {
        self.clear();
        self.load_string_wz_with(arena, string_root, LoadMode::Load)
    }

    /// `String.wz` 루트를 기존 결과 위에 병합한다 (`StringLinker.Update`).
    ///
    /// 비우지 않고, `FullPath` 도 덮어쓰지 않는다 (`StringLinker.cs:97` 등).
    pub fn update_string_wz(&mut self, arena: &Arena, string_root: NodeId) -> bool {
        self.load_string_wz_with(arena, string_root, LoadMode::Update)
    }

    fn load_string_wz_with(&mut self, arena: &Arena, string_root: NodeId, mode: LoadMode) -> bool {
        for &child in arena.children(string_root) {
            let Some(name) = arena.name(child) else {
                continue;
            };
            // `Wz_Node imgNode = node` — `Wz_Image` 가 아니면 `.img` 엔트리
            // 노드 자체를 순회한다 (GMS 포맷, `StringLinker.cs:70`).
            let img_root = arena.descend_image(child);
            self.load_image_with(arena, name, img_root, mode);
        }
        self.has_values()
    }

    /// `Item.wz` 루트 — `Special/0910.img` (`StringLinker.cs:416-450`).
    pub fn load_item_wz(&mut self, arena: &Arena, item_root: NodeId) -> bool {
        self.load_item_wz_with(arena, item_root, LoadMode::Load)
    }

    /// `Item.wz` 루트를 병합한다 (`StringLinker.Update` 의 itemNode).
    pub fn update_item_wz(&mut self, arena: &Arena, item_root: NodeId) -> bool {
        self.load_item_wz_with(arena, item_root, LoadMode::Update)
    }

    fn load_item_wz_with(&mut self, arena: &Arena, item_root: NodeId, mode: LoadMode) -> bool {
        let Some(special) = find_path(arena, item_root, "Special") else {
            return self.has_values();
        };
        for &node in arena.children(special) {
            if arena.name(node) == Some("0910.img") {
                let img_root = arena.descend_image(node);
                self.load_0910_img(arena, img_root, mode);
            }
        }
        self.has_values()
    }

    /// `Etc.wz` 루트 — `SetItemInfo.img` 와 `worldArchive.img`
    /// (`StringLinker.cs:452-485`, `:517-625`).
    pub fn load_etc_wz(&mut self, arena: &Arena, etc_root: NodeId) -> bool {
        self.load_etc_wz_with(arena, etc_root, LoadMode::Load)
    }

    /// `Etc.wz` 루트를 병합한다 (`StringLinker.Update` 의 etcNode).
    pub fn update_etc_wz(&mut self, arena: &Arena, etc_root: NodeId) -> bool {
        self.load_etc_wz_with(arena, etc_root, LoadMode::Update)
    }

    fn load_etc_wz_with(&mut self, arena: &Arena, etc_root: NodeId, mode: LoadMode) -> bool {
        for &node in arena.children(etc_root) {
            match arena.name(node) {
                Some("SetItemInfo.img") => {
                    let img_root = arena.descend_image(node);
                    self.load_set_item_info_img(arena, img_root, mode);
                }
                Some("worldArchive.img") => {
                    let img_root = arena.descend_image(node);
                    self.load_world_archive(arena, img_root, mode);
                }
                _ => {}
            }
        }
        self.has_values()
    }

    /// `Quest.wz` 루트 — `QuestData` 또는 `QuestInfo.img`
    /// (`StringLinker.cs:627-673`).
    pub fn load_quest_wz(&mut self, arena: &Arena, quest_root: NodeId) -> bool {
        self.load_quest_wz_with(arena, quest_root, LoadMode::Load)
    }

    /// `Quest.wz` 루트를 병합한다 (`StringLinker.Update` 의 questNode).
    pub fn update_quest_wz(&mut self, arena: &Arena, quest_root: NodeId) -> bool {
        self.load_quest_wz_with(arena, quest_root, LoadMode::Update)
    }

    fn load_quest_wz_with(&mut self, arena: &Arena, quest_root: NodeId, mode: LoadMode) -> bool {
        let mut data = find_path(arena, quest_root, "QuestData");
        let mut new_quest_dir = true;
        if data.is_none() {
            if let Some(info) = find_path(arena, quest_root, "QuestInfo.img") {
                data = Some(arena.descend_image(info));
                new_quest_dir = false;
            }
        }
        let Some(data) = data else {
            return self.has_values();
        };

        for &entry in arena.children(data) {
            let tree = arena.descend_image(entry);
            let info = if new_quest_dir {
                find_path(arena, tree, "QuestInfo")
            } else {
                Some(tree)
            };
            let Some(info) = info else { continue };
            let Some(tree_name) = arena.name(tree) else {
                continue;
            };
            let Ok(id) = parse_csharp_int(&tree_name.replace(".img", "")) else {
                continue;
            };
            let Some(link_node) = resolve_uol_lossy(arena, info) else {
                continue;
            };

            let mut sr = take(&mut self.string_quest, id, mode);
            set_default_string(arena, &mut sr.name, link_node, "name");
            if let Some(v) = get_default_string(arena, link_node, "0") {
                sr.desc = Some(v);
            }
            if let Some(v) = get_default_string(arena, link_node, "demand/base") {
                sr.quest_demand_base = Some(v);
            }
            set_full_path(&mut sr, mode, || {
                let prefix = if new_quest_dir { "QuestData\\" } else { "" };
                format!("{prefix}{}", node_path(arena, tree))
            });
            add_all_values(arena, &mut sr, link_node);
            self.string_quest.insert(id, sr);
        }
        self.has_values()
    }

    /// `Reactor.wz` 루트 — 각 `.img` 의 `info` (`StringLinker.cs:675-703`).
    pub fn load_reactor_wz(&mut self, arena: &Arena, reactor_root: NodeId) -> bool {
        self.load_reactor_wz_with(arena, reactor_root, LoadMode::Load)
    }

    /// `Reactor.wz` 루트를 병합한다 (`StringLinker.Update` 의 reactorNode).
    pub fn update_reactor_wz(&mut self, arena: &Arena, reactor_root: NodeId) -> bool {
        self.load_reactor_wz_with(arena, reactor_root, LoadMode::Update)
    }

    fn load_reactor_wz_with(
        &mut self,
        arena: &Arena,
        reactor_root: NodeId,
        mode: LoadMode,
    ) -> bool {
        for &entry in arena.children(reactor_root) {
            let tree = arena.descend_image(entry);
            let Some(info) = find_path(arena, tree, "info") else {
                continue;
            };
            let Some(tree_name) = arena.name(tree) else {
                continue;
            };
            let Ok(id) = parse_csharp_int(&tree_name.replace(".img", "")) else {
                continue;
            };
            let Some(link_node) = resolve_uol_lossy(arena, info) else {
                continue;
            };

            let mut sr = take(&mut self.string_reactor, id, mode);
            if let Some(v) = get_default_string(arena, link_node, "viewName")
                .or_else(|| get_default_string(arena, link_node, "name"))
                .or_else(|| get_default_string(arena, link_node, "info"))
            {
                sr.name = v;
            }
            set_full_path(&mut sr, mode, || {
                format!("Reactor\\{}", node_path(arena, tree))
            });
            // 원본은 reactor 에 `AddAllValue` 를 호출하지 않는다 (`:700` 주석).
            self.string_reactor.insert(id, sr);
        }
        self.has_values()
    }

    /// 단일 문자열 이미지(`Consume.img`, `Eqp.img`, `Skill.img` 등)를 로드한다.
    pub fn load_image(&mut self, arena: &Arena, image_name: &str, image_root: NodeId) {
        self.load_image_with(arena, image_name, image_root, LoadMode::Load);
    }

    /// 단일 문자열 이미지를 병합한다 (`StringLinker.Update` 경로).
    pub fn update_image(&mut self, arena: &Arena, image_name: &str, image_root: NodeId) {
        self.load_image_with(arena, image_name, image_root, LoadMode::Update);
    }

    fn load_image_with(
        &mut self,
        arena: &Arena,
        image_name: &str,
        image_root: NodeId,
        mode: LoadMode,
    ) {
        let root = arena.descend_image(image_root);
        match image_name {
            "Pet.img" | "Cash.img" | "Ins.img" | "Consume.img" => {
                for &tree in arena.children(root) {
                    let Some(text) = arena.name(tree) else {
                        continue;
                    };
                    let Ok(id) = parse_csharp_int(text) else {
                        continue;
                    };
                    let Some(link_node) = resolve_uol_lossy(arena, tree) else {
                        continue;
                    };
                    let mut sr = take(&mut self.string_item, id, mode);
                    set_default_string(arena, &mut sr.name, link_node, "name");
                    if let Some(v) = get_default_string(arena, link_node, "desc") {
                        sr.desc = Some(v);
                    }
                    if let Some(v) = get_default_string(arena, link_node, "autodesc") {
                        sr.auto_desc = Some(v);
                    }
                    set_full_path(&mut sr, mode, || format!("{image_name}/{text}"));
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
                        let Ok(id) = parse_csharp_int(text) else {
                            continue;
                        };
                        let Some(link_node) = resolve_uol_lossy(arena, tree) else {
                            continue;
                        };
                        let mut sr = take(&mut self.string_item, id, mode);
                        set_default_string(arena, &mut sr.name, link_node, "name");
                        if let Some(v) = get_default_string(arena, link_node, "desc") {
                            sr.desc = Some(v);
                        }
                        set_full_path(&mut sr, mode, || format!("{image_name}/{cat_name}/{text}"));
                        add_all_values(arena, &mut sr, link_node);
                        self.string_item.insert(id, sr);
                    }
                }
            }
            "Familiar.img" | "FamiliarSkill.img" => {
                for &tree0 in arena.children(root) {
                    if arena.name(tree0) != Some("skill") {
                        continue;
                    }
                    for &tree1 in arena.children(tree0) {
                        let Some(text) = arena.name(tree1) else {
                            continue;
                        };
                        let Ok(id) = parse_csharp_int(text) else {
                            continue;
                        };
                        let Some(link_node) = resolve_uol_lossy(arena, tree1) else {
                            continue;
                        };
                        // 원본은 update 여부와 무관하게 항상 새 결과다 (`:148-150`).
                        let mut sr = StringResult::default();
                        set_default_string(arena, &mut sr.name, link_node, "name");
                        if let Some(v) = get_default_string(arena, link_node, "desc") {
                            sr.desc = Some(v);
                        }
                        set_full_path(&mut sr, mode, || format!("{image_name}/skill/{text}"));
                        add_all_values(arena, &mut sr, link_node);
                        self.string_familiar_skill.insert(id, sr);
                    }
                }
            }
            "Mob.img" => {
                for &tree in arena.children(root) {
                    let Some(text) = arena.name(tree) else {
                        continue;
                    };
                    let Ok(id) = parse_csharp_int(text) else {
                        continue;
                    };
                    let Some(link_node) = resolve_uol_lossy(arena, tree) else {
                        continue;
                    };
                    let mut sr = take(&mut self.string_mob, id, mode);
                    set_default_string(arena, &mut sr.name, link_node, "name");
                    set_full_path(&mut sr, mode, || format!("{image_name}/{text}"));
                    add_all_values(arena, &mut sr, link_node);
                    self.string_mob.insert(id, sr);
                }
            }
            "MonsterBook.img" => {
                for &tree in arena.children(root) {
                    let Some(text) = arena.name(tree) else {
                        continue;
                    };
                    let Ok(id) = parse_csharp_int(text) else {
                        continue;
                    };
                    // `Mob.img` 이 먼저 로드되어 있어야 이름을 빌 수 있다.
                    let Some(mob) = self.string_mob.get(&id) else {
                        continue;
                    };
                    // `FindNodeByPath("episode")` — UOL 해석 없이 그대로 찾는다.
                    let Some(message_node) = find_path(arena, tree, "episode") else {
                        continue;
                    };
                    // 원본은 `messageNode.Value.ToString()` 이라 값이 없으면
                    // NullReferenceException 이다. 여기서는 건너뛴다.
                    let Some(desc) = node_value_to_string(arena.value(message_node)) else {
                        continue;
                    };
                    let mut sr = take(&mut self.string_monster_book, id, mode);
                    sr.name = mob.name.clone();
                    sr.desc = Some(desc);
                    self.string_monster_book.insert(id, sr);
                }
            }
            "Npc.img" => {
                for &tree in arena.children(root) {
                    let Some(text) = arena.name(tree) else {
                        continue;
                    };
                    let Ok(id) = parse_csharp_int(text) else {
                        continue;
                    };
                    let Some(link_node) = resolve_uol_lossy(arena, tree) else {
                        continue;
                    };
                    let mut sr = take(&mut self.string_npc, id, mode);
                    set_default_string(arena, &mut sr.name, link_node, "name");
                    if let Some(v) = get_default_string(arena, link_node, "func") {
                        sr.desc = Some(v);
                    }
                    set_full_path(&mut sr, mode, || format!("{image_name}/{text}"));
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
                        let Ok(id) = parse_csharp_int(text) else {
                            continue;
                        };
                        let Some(link_node) = resolve_uol_lossy(arena, tree) else {
                            continue;
                        };
                        let mut sr = take(&mut self.string_map, id, mode);
                        // `string.Format` 은 null 을 빈 문자열로 찍으므로 이름은
                        // **항상** 덮어쓴다 (`StringLinker.cs:269-271`). 다만
                        // update 모드에서 패치가 한쪽 필드만 주는 경우, 먼저
                        // 기존 값을 유지한 뒤 합성해야 `name` 과
                        // `street_name`/`map_name` 이 어긋나지 않는다.
                        if let Some(v) = get_default_string(arena, link_node, "streetName") {
                            sr.street_name = Some(v);
                        }
                        if let Some(v) = get_default_string(arena, link_node, "mapName") {
                            sr.map_name = Some(v);
                        }
                        sr.name = format!(
                            "{} : {}",
                            sr.street_name.as_deref().unwrap_or_default(),
                            sr.map_name.as_deref().unwrap_or_default()
                        );
                        if let Some(v) = get_default_string(arena, link_node, "mapDesc") {
                            sr.desc = Some(v);
                        }
                        set_full_path(&mut sr, mode, || format!("{image_name}/{c0}/{text}"));
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
                    // `StringLinker.cs:295-308` — update 모드는 이름 키를 먼저 보고,
                    // 없던 때만 숫자 ID 사전을 본다. (`try` 안의 두 대입 순서 그대로.)
                    let existing = self.string_skill_str.get(text).cloned().or_else(|| {
                        skill_numeric_id(text).and_then(|id| self.string_skill.get(&id).cloned())
                    });

                    let mut sr = match (mode, existing) {
                        (LoadMode::Update, Some(sr)) => sr,
                        _ => StringResult::default(),
                    };

                    set_default_string(arena, &mut sr.name, link_node, "name");
                    if let Some(v) = get_default_string(arena, link_node, "desc") {
                        sr.desc = Some(v);
                    }
                    if let Some(v) = get_default_string(arena, link_node, "pdesc") {
                        sr.pdesc = Some(v);
                    }

                    // `StringLinker.cs:314-319` — `SkillH.Add(h)` 는 h 가 null 이어도 원소를
                    // 추가한다. 그래서 마지막 원소가 null 이면, 바로 아래의
                    // `if (SkillH.Last() == null)` 이 true 가 되어 그 원소를
                    // 지우고 `h1..hn` 루프로 넘어간다.
                    let h = get_default_string(arena, link_node, "h");
                    if mode.is_update() && h.is_some() {
                        sr.skill_h.clear();
                    }
                    let mut cleared_levels = false;
                    match h {
                        Some(h) => sr.skill_h.push(h),
                        None => {
                            // `h1..hn` — 비어 있으면 멈춘다 (`:342-353`).
                            // update 모드에서는 **첫 비어 있지 않은 값에서 한 번만**
                            // 기존 목록을 비운다 (`cleared` 플래그).
                            for hi in numbered_levels(arena, link_node) {
                                if mode.is_update() && !cleared_levels {
                                    sr.skill_h.clear();
                                    cleared_levels = true;
                                }

                                sr.skill_h.push(hi);
                            }
                        }
                    }

                    let ph = get_default_string(arena, link_node, "ph");
                    merge_single(&mut sr.skill_ph, ph, mode);

                    let hch = get_default_string(arena, link_node, "hch");
                    merge_single(&mut sr.skill_hch, hch, mode);

                    collect_extra_h(arena, &mut sr, link_node);
                    set_full_path(&mut sr, mode, || format!("{image_name}/{text}"));
                    add_all_values(arena, &mut sr, link_node);

                    if let Some(id) = skill_numeric_id(text) {
                        self.string_skill.insert(id, sr.clone());
                    }
                    self.string_skill_str.insert(text.to_string(), sr);
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
                            let Ok(id) = parse_csharp_int(text) else {
                                continue;
                            };
                            let Some(link_node) = resolve_uol_lossy(arena, tree) else {
                                continue;
                            };
                            let mut sr = take(&mut self.string_eqp, id, mode);
                            set_default_string(arena, &mut sr.name, link_node, "name");
                            if let Some(v) = get_default_string(arena, link_node, "desc") {
                                sr.desc = Some(v);
                            }
                            set_full_path(&mut sr, mode, || {
                                format!("{image_name}/{c0}/{c1}/{text}")
                            });
                            add_all_values(arena, &mut sr, link_node);
                            self.string_eqp.insert(id, sr);
                        }
                    }
                }
            }
            "SetItemInfo.img" => self.load_set_item_info_img(arena, root, mode),
            "0910.img" => self.load_0910_img(arena, root, mode),
            _ => {}
        }
    }

    /// `Etc.wz/SetItemInfo.img` — 이름은 `setItemName` 에서만 온다
    /// (`StringLinker.cs:476`). `desc` 는 읽지 않는다.
    fn load_set_item_info_img(&mut self, arena: &Arena, root: NodeId, mode: LoadMode) {
        for &tree in arena.children(root) {
            let Some(text) = arena.name(tree) else {
                continue;
            };
            let Ok(id) = parse_csharp_int(text) else {
                continue;
            };
            let Some(link_node) = resolve_uol_lossy(arena, tree) else {
                continue;
            };
            let mut sr = take(&mut self.string_set_item, id, mode);
            set_default_string(arena, &mut sr.name, link_node, "setItemName");
            set_full_path(&mut sr, mode, || format!("SetItemInfo.img/{text}"));
            add_all_values(arena, &mut sr, link_node);
            self.string_set_item.insert(id, sr);
        }
    }

    /// `Item.wz/Special/0910.img` — `Consume.img` 와 같지만 `autodesc` 는
    /// 읽지 않는다 (`StringLinker.cs:440-441`).
    fn load_0910_img(&mut self, arena: &Arena, root: NodeId, mode: LoadMode) {
        for &tree in arena.children(root) {
            let Some(text) = arena.name(tree) else {
                continue;
            };
            let Ok(id) = parse_csharp_int(text) else {
                continue;
            };
            let Some(link_node) = resolve_uol_lossy(arena, tree) else {
                continue;
            };
            let mut sr = take(&mut self.string_item, id, mode);
            set_default_string(arena, &mut sr.name, link_node, "name");
            if let Some(v) = get_default_string(arena, link_node, "desc") {
                sr.desc = Some(v);
            }
            set_full_path(&mut sr, mode, || format!("0910.img/{text}"));
            add_all_values(arena, &mut sr, link_node);
            self.string_item.insert(id, sr);
        }
    }

    /// `Etc.wz/worldArchive.img` (`StringLinker.cs:517-625`).
    ///
    /// ```text
    /// collectionInfo/<채널>/<숫자>/mob|<npc>/<엔트리>
    /// ```
    ///
    /// `worldName` / `worldDesc` 는 원본이 `case` 로 명시적으로 건너뛴다.
    /// 엔트리의 이름은 `Mob.img` / `Npc.img` 에서 빌고, 설명이 비면 `"(null)"`.
    fn load_world_archive(&mut self, arena: &Arena, root: NodeId, mode: LoadMode) {
        let Some(info) = find_path(arena, root, "collectionInfo") else {
            return;
        };
        for &node in arena.children(info) {
            let Some(node_text) = arena.name(node) else {
                continue;
            };
            for &sub in arena.children(node) {
                let Some(sub_text) = arena.name(sub) else {
                    continue;
                };
                match sub_text {
                    // 원본 `case "worldName": case "worldDesc": break;`
                    "worldName" | "worldDesc" => continue,
                    _ => {}
                }
                if parse_csharp_int(sub_text).is_err() {
                    continue;
                }
                for &sub2 in arena.children(sub) {
                    let kind = match arena.name(sub2) {
                        Some("mob") => WorldArchiveKind::Mob,
                        Some("npc") => WorldArchiveKind::Npc,
                        _ => continue,
                    };
                    for &entry in arena.children(sub2) {
                        self.load_world_archive_entry(WorldArchiveEntry {
                            arena,
                            kind,
                            entry,
                            node_text,
                            sub_text,
                            kind_node: sub2,
                            mode,
                        });
                    }
                }
            }
        }
    }

    fn load_world_archive_entry(&mut self, entry: WorldArchiveEntry<'_>) {
        let WorldArchiveEntry {
            arena,
            kind,
            entry,
            node_text,
            sub_text,
            kind_node,
            mode,
        } = entry;
        let Some(entry_text) = arena.name(entry) else {
            return;
        };
        let kind_text = arena.name(kind_node).unwrap_or_default();

        let ids = world_archive_ids(arena, entry);
        // `desc` 가 없거나 비면 원본은 `"(null)"` 을 넣는다.
        let desc = find_path(arena, entry, "desc")
            .and_then(|n| node_value_to_string(arena.value(n)))
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "(null)".to_string());

        let path = format!("{node_text}_{sub_text}_{kind_text}_{entry_text}");

        let mut sr = match (mode, kind) {
            (LoadMode::Update, WorldArchiveKind::Mob) => self
                .string_world_archive_mob_by_path
                .get(&path)
                .cloned()
                .unwrap_or_default(),
            (LoadMode::Update, WorldArchiveKind::Npc) => self
                .string_world_archive_npc_by_path
                .get(&path)
                .cloned()
                .unwrap_or_default(),
            _ => StringResult::default(),
        };
        sr.desc = Some(desc);

        // 원본은 같은 `StringResult` 를 경로 사전과 ID 사전에 **공유**한다
        // (`StringLinker.cs:565`, `:602`). 그래서 `Name` 는 열거된 ID 중
        // `stringMob` / `stringNpc` 에 **실제로 있는 마지막 것**의 이름이 된다.
        // 하나도 없으면 배정 자체가 일어나지 않으므로 기존 값이 남는다.
        let target_map = match kind {
            WorldArchiveKind::Mob => &self.string_mob,
            WorldArchiveKind::Npc => &self.string_npc,
        };
        let assigned_name = ids.iter().rev().find_map(|id| target_map.get(id));
        if let Some(target) = assigned_name {
            sr.name = target.name.clone();
        }

        let (path_map, id_map) = match kind {
            WorldArchiveKind::Mob => (
                &mut self.string_world_archive_mob_by_path,
                &mut self.string_world_archive_mob,
            ),
            WorldArchiveKind::Npc => (
                &mut self.string_world_archive_npc_by_path,
                &mut self.string_world_archive_npc,
            ),
        };

        path_map.insert(path, sr.clone());
        for id in ids {
            if target_map.contains_key(&id) {
                id_map.insert(id, sr.clone());
            }
        }
    }

    /// 도메인을 명시해 문자열을 조회한다 (ADR-0006).
    ///
    /// `get_by_id` 의 fallback 순서에 기대지 않으려면 이 메서드를 쓴다.
    pub fn get(&self, domain: StringDomain, id: i32) -> Option<&StringResult> {
        match domain {
            StringDomain::Item => self.string_item.get(&id),
            StringDomain::Eqp => self.string_eqp.get(&id),
            StringDomain::Skill => self.string_skill.get(&id),
            StringDomain::Map => self.string_map.get(&id),
            StringDomain::Mob => self.string_mob.get(&id),
            StringDomain::Npc => self.string_npc.get(&id),
        }
    }

    /// ID로 등록된 문자열을 `item → eqp → skill → map → mob → npc` 순으로 조회한다.
    ///
    /// 원본에 이 순서를 정한 단일 함수는 없다 — 렌더러마다 필요한 사전 하나만
    /// 본다 (`ItemTooltipRender.cs:64`, `GearTooltipRender.cs:144`,
    /// `MapTooltipRenderer.cs:395`). 이 순서는 그 관습을 하나로 모은 **편의**이며
    /// (ADR-0006), 도메인이 정해진 오라클 op 는 [`Self::get`] 을 써야 한다.
    pub fn get_by_id(&self, id: i32) -> Option<&StringResult> {
        StringDomain::ALL
            .iter()
            .find_map(|&domain| self.get(domain, id))
    }

    /// 하나의 `StringResult` 를 오라클 `tooltip` 텍스트로 만든다.
    pub fn format_tooltip_result(sr: &StringResult) -> String {
        match sr.desc.as_deref() {
            Some(desc) if !desc.is_empty() => format!("{}\n{}", sr.name, desc),
            _ => sr.name.clone(),
        }
    }

    /// 도메인을 명시한 툴팁 조회. 도메인별 사전을 직접 본다.
    pub fn format_tooltip_in(&self, domain: StringDomain, id: i32) -> Option<String> {
        self.get(domain, id).map(Self::format_tooltip_result)
    }

    /// 오라클 `tooltip` 검증용 텍스트: `name`, 그리고 `desc` 가 있으면
    /// 개행으로 이어붙인 것. `desc` 가 비면 `name` 만 돌려준다.
    ///
    /// # KNOWN GAP — C# 에 대응하는 **텍스트** 툴팁이 없다
    ///
    /// 원본의 툴팁은 `Bitmap` 이다 (`ItemTooltipRender.cs:52-117`):
    /// 이름 → 속성 태그 → 아이콘 → 요구레벨 → `sr.Desc + sr.AutoDesc`
    /// → 거래 가능 여부를 GDI `DrawString` 으로 좌표에 얹는다. 돌려주는 문자열이
    /// 없어서 바이트 단위 대조 대상이 될 수 없다. 그러니 여기서 더 풍부한 형식을
    /// **발명하지 않는다** — 그러면 대조가 불가능해질 뿐이다.
    /// C# GUI 와 달라지는 지점은 `Desc + AutoDesc` 연결과 `"(null)"` 이름 대체다.
    pub fn format_tooltip(&self, id: i32) -> Option<String> {
        self.get_by_id(id).map(Self::format_tooltip_result)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorldArchiveKind {
    Mob,
    Npc,
}

/// `StringLinker.cs:342-353` 의 `h1`, `h2`, … 루프.
fn numbered_levels(arena: &Arena, node: NodeId) -> Vec<String> {
    let mut out = Vec::new();
    for i in 1.. {
        let key = format!("h{i}");
        match get_default_string(arena, node, &key) {
            Some(v) if !v.is_empty() => out.push(v),
            _ => break,
        }
    }
    out
}

/// `StringLinker.cs:356-367` — KMST1196 의 `h_<level>` 노드를 모은다.
fn collect_extra_h(arena: &Arena, sr: &mut StringResult, link_node: NodeId) {
    for &child in arena.children(link_node) {
        let Some(name) = arena.name(child) else {
            continue;
        };
        let Some(rest) = name.strip_prefix("h_") else {
            continue;
        };
        let Ok(level) = parse_csharp_int(rest) else {
            continue;
        };
        if level <= 0 {
            continue;
        }
        let Some(text) = node_value_to_string(arena.value(child)) else {
            continue;
        };
        sr.skill_extra_h.retain(|(l, _)| *l != level);
        sr.skill_extra_h.push((level, text));
    }
    if sr.skill_extra_h.len() > 1 {
        sr.skill_extra_h.sort_by_key(|(level, _)| *level);
    }
}

/// `worldArchive.img` 엔트리의 `id` 아래 정수들을 모은다 (`StringLinker.cs:544-548`).
///
/// 값이 0 이면 건너뛴다.
fn world_archive_ids(arena: &Arena, entry: NodeId) -> Vec<i32> {
    let Some(id_node) = find_path(arena, entry, "id") else {
        return Vec::new();
    };
    arena
        .children(id_node)
        .iter()
        .filter_map(|&c| match arena.value(c) {
            Some(NodeValue::Short(v)) => Some(i32::from(*v)),
            Some(NodeValue::Int(v)) => Some(*v),
            Some(NodeValue::Long(v)) => i32::try_from(*v).ok(),
            Some(NodeValue::Uint(v)) => i32::try_from(*v).ok(),
            _ => None,
        })
        .filter(|&v| v != 0)
        .collect()
}

/// C# `Int32.TryParse` 의 기본 `NumberStyles.Integer` 처럼 앞뒤 공백과
/// 선택적 부호를 허용한다. 숫자가 아니면 `Err`.
fn parse_csharp_int(text: &str) -> Result<i32, ()> {
    text.trim().parse::<i32>().map_err(|_| ())
}

/// `StringLinker.cs:373` — 이름 길이가 7 이상이고 숫자로 읽혀야 한다.
fn skill_numeric_id(text: &str) -> Option<i32> {
    if text.len() < 7 {
        return None;
    }
    parse_csharp_int(text).ok()
}

/// `Load` 는 새 결과로 덮어쓰고, `Update` 는 기존 결과를 이어받는다.
fn take(map: &mut BTreeMap<i32, StringResult>, id: i32, mode: LoadMode) -> StringResult {
    match mode {
        LoadMode::Load => StringResult::default(),
        LoadMode::Update => map.remove(&id).unwrap_or_default(),
    }
}

/// 단일 문자열 필드를 `Load`/`Update` 규칙에 맞게 병합한다.
///
/// `Update` 는 새 값이 있으면 기존 값을 비우고 교체하고, 없으면 유지한다.
/// `Load` 는 항상 append 한다. (`StringLinker.cs` 의 `if (!update) … else …`)
fn merge_single(slot: &mut Vec<String>, value: Option<String>, mode: LoadMode) {
    match (mode, value) {
        (LoadMode::Update, Some(v)) => {
            slot.clear();
            slot.push(v);
        }
        (LoadMode::Update, None) => {}
        (LoadMode::Load, Some(v)) => slot.push(v),
        (LoadMode::Load, None) => {}
    }
}

/// `if (!update) strResult.FullPath = ...` — update 모드는 경로를 건드리지 않는다.
fn set_full_path<F>(sr: &mut StringResult, mode: LoadMode, make: F)
where
    F: FnOnce() -> String,
{
    if !mode.is_update() {
        sr.full_path = make();
    }
}

/// `GetDefaultString` (`StringLinker.cs:764-768`).
///
/// 노드가 **있으면** 값을 문자열로 돌려준다. 값이 없거나 스칼라가 아니면
/// C# 의 `Convert.ToString(null)` (= `""`) 과 같은 빈 문자열이다.
/// 노드의 루트→자기 경로를 `\` 로 잇는다. C# `Wz_Node.FullPath` 대응.
fn node_path(arena: &Arena, node: NodeId) -> String {
    let mut parts = Vec::new();
    let mut cur = Some(node);
    while let Some(id) = cur {
        if let Some(name) = arena.name(id) {
            if !name.is_empty() {
                parts.push(name.to_string());
            }
        }
        cur = arena.parent(id);
    }
    parts.reverse();
    parts.join("\\")
}

fn get_default_string(arena: &Arena, node: NodeId, search_path: &str) -> Option<String> {
    let target = find_path(arena, node, search_path)?;
    Some(node_value_to_string(arena.value(target)).unwrap_or_default())
}

/// `strResult.Name = GetDefaultString(...) ?? strResult.Name ?? string.Empty`
fn set_default_string(arena: &Arena, name: &mut String, link_node: NodeId, search_path: &str) {
    if let Some(v) = get_default_string(arena, link_node, search_path) {
        *name = v;
    }
}

/// `AddAllValue` (`StringLinker.cs:770-779`) — 스칼라 자식을 전부 평탄화한다.
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
pub fn extract_gear_stats(arena: &Arena, gear_root: NodeId) -> Vec<(String, String)> {
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

#[cfg(test)]
mod unit_tests {
    use super::*;

    #[test]
    fn parse_csharp_int_accepts_surrounding_whitespace() {
        // C# Int32.TryParse 의 기본 NumberStyles.Integer 와 같은 동작.
        assert_eq!(parse_csharp_int(" 100 "), Ok(100));
        assert_eq!(parse_csharp_int("-7"), Ok(-7));
        assert_eq!(parse_csharp_int("1.5"), Err(()));
    }
}

#[cfg(test)]
mod domain_tests {
    use super::*;

    #[test]
    fn string_domain_parse_is_case_insensitive_and_explicit() {
        assert_eq!(StringDomain::parse("ITEM"), Some(StringDomain::Item));
        assert_eq!(StringDomain::parse("equip"), Some(StringDomain::Eqp));
        assert_eq!(StringDomain::parse("mob"), Some(StringDomain::Mob));
        assert_eq!(StringDomain::parse("unknown"), None);
    }
}
