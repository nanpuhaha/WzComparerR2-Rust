//! `wz-db` — 노드 arena, UOL/링크 해석, 질의 엔진, 문자열 링커.
//!
//! # 이 crate 가 하는 일
//!
//! `wz-core` 가 "바이트 → 노드 트리" 라면, 여기는 "노드 트리 → **참조가
//! 풀린** 트리" 다. 세 가지를 맡는다.
//!
//! 1. **arena** — 노드를 평탄 저장해 부모 이동을 O(1) 로 만들고, `NodeId`
//!    핸들로 메모리를 다룬다 (ADR-2).
//! 2. **해석** — UOL(`"../Other.img/icon"`) 체인을 푼다. 원본의 해석기 두 벌
//!    (`Db2Host.GetNode` / `DB2NodeExtension.GetNode`) 을 이 한 곳으로 모은다.
//! 3. **질의 및 문자열 링커** — 문자열/숫자/벡터 매처 (`WzQueryControl.cs:761-1229`) 및
//!    아이템/장비/스킬/맵 ID별 툴팁 문자열 링커 (`StringLinker.cs`).
//!
//! # 원본에서 고치는 것
//!
//! - **UOL 무한 루프** — `Wz_Node.ResolveUol` (`Wz_Node.cs:549-558`) 에는 방문
//!   집합이 없다. 순환 링크 하나면 멈추지 않는다. [`resolve::resolve_uol`] 은
//!   방문 집합을 갖고 순환을 오류로 낸다.
//! - **해석기 2벌** — 재귀형과 반복형이 서로 다른 규칙으로 경로를 푼다.
//!   한 벌로 통합한다.

pub mod achievement;
pub mod arena;
pub mod calc;
pub mod chara_prop;
pub mod db2;
pub mod db2_characters;
pub mod db2_damage_skins;
pub mod db2_item_tabs;
pub mod db2_items;
pub mod db2_maps;
pub mod db2_mobs;
pub mod db2_morphs;
pub mod db2_music;
pub mod db2_npcs;
pub mod db2_reactors;
pub mod db2_skills;
pub mod db2_taming_mobs;
pub mod gear_stats;
pub mod item_tooltip;
pub mod linker;
pub mod map;
pub mod node;
pub mod npc_quote;
pub mod particle;
pub mod query;
pub mod resolve;
pub mod skill_summary;
pub mod world_archive;

#[cfg(test)]
mod tests;

pub use achievement::{Achievement, AchievementReward};
pub use arena::Arena;
pub use calc::{evaluate_expression, CalcError};
pub use chara_prop::CharaProp;
pub use db2::{image_rows as db2_image_rows, Db2Row};
pub use db2_characters::{
    character_grid_rows as db2_character_grid_rows,
    character_grid_rows_for as db2_character_grid_rows_for, Db2CharacterRow,
};
pub use db2_damage_skins::{damage_skin_rows as db2_damage_skin_rows, Db2DamageSkinRow};
pub use db2_item_tabs::{
    item_tab_rows as db2_item_tab_rows, item_tab_tooltips as db2_item_tab_tooltips,
    load_string_image as db2_item_tab_string_image, Db2ItemTabRow,
};
pub use db2_items::{item_grid_rows as db2_item_grid_rows, Db2ItemRow};
pub use db2_maps::{map_grid_rows as db2_map_grid_rows, Db2MapRow};
pub use db2_mobs::{mob_grid_rows as db2_mob_grid_rows, Db2MobRow};
pub use db2_morphs::{morph_grid_rows as db2_morph_grid_rows, Db2MorphRow};
pub use db2_music::{music_grid_rows as db2_music_grid_rows, Db2MusicRow};
pub use db2_npcs::{npc_grid_rows as db2_npc_grid_rows, Db2NpcRow};
pub use db2_reactors::{reactor_grid_rows as db2_reactor_grid_rows, Db2ReactorRow};
pub use db2_skills::{skill_grid_rows as db2_skill_grid_rows, Db2SkillRow};
pub use db2_taming_mobs::{taming_mob_grid_rows as db2_taming_mob_grid_rows, Db2TamingMobRow};
pub use gear_stats::extract_gear_props;
pub use item_tooltip::{
    build_item_tooltip, item_attribute_lines, item_core_spec_string, ItemTooltipView,
};
pub use linker::{
    extract_gear_stats, LoadMode, StringDomain, StringLinker, StringResult, StringRoot, StringRoots,
};
pub use map::{
    BackItem, Foothold, FootholdTree, IlluminantCluster, LadderRope, LadderRopeKind, LifeKind,
    MapData, MapEvent, MapInfo, MapLife, MapLight, MapLight2D, MapParticle, MapPortal, MapReactor,
    MapTooltip, MapVirtualRect, MiniMapInfo, ObjItem, SkyWhale, TileItem,
};
pub use node::{NodeId, NodeValue, RawDataRef};
pub use npc_quote::{message as npc_quote_message, parse as npc_quote_parse, NpcQuote};
pub use particle::{parse_particle_desc, ParticleDesc, ParticleTexture};
pub use query::{
    execute_query, node_value_to_string, NumericComparisonMode, QueryExecutionResult, QueryResult,
    QueryRule, QueryRuleKind, QueryValueType, StringComparisonMode, ValueRule, DEFAULT_MAX_RESULTS,
};
pub use resolve::{find_path, handle_uol, resolve_uol, resolve_uol_lossy, ResolveError};
pub use skill_summary::parse_skill_summary;
pub use world_archive::{
    mob_lines as world_archive_mob_lines, npc_lines as world_archive_npc_lines,
    split_lines as world_archive_split_lines,
};
