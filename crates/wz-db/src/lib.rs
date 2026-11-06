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
pub mod chara_tooltip;
pub mod common_controls;
pub mod controls_input;
pub mod cosmetic_eulo;
pub mod damage_skin;
pub mod db2;
pub mod db2_bin;
pub mod db2_characters;
pub mod db2_damage_skins;
pub mod db2_familiar;
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
pub mod dotnet_metadata;
pub mod draw_math;
pub mod format_parser;
pub mod gear_stats;
pub mod gear_type_names;
pub mod item_tooltip;
pub mod item_tooltip_extra;
pub mod item_tooltip_layout;
pub mod linker;
pub mod main_window;
pub mod map;
pub mod map_tooltip;
pub mod node;
pub mod npc_quote;
pub mod particle;
pub mod pixel_ops;
pub mod plugin_events;
pub mod plugin_find;
pub mod plugin_ribbon;
pub mod query;
pub mod quest_tooltip;
pub mod resolve;
pub mod skill_summary;
pub mod skill_tooltip;
pub mod sprite_compose;
pub mod text_layout;
pub mod world_archive;

#[cfg(test)]
mod tests;

pub use achievement::{Achievement, AchievementReward};
pub use arena::Arena;
pub use calc::{evaluate_expression, CalcError};
pub use chara_prop::CharaProp;
pub use chara_tooltip::{npc_lines, npc_locations, reactor_from_node, NpcTooltip, ReactorTooltip};
pub use common_controls::{
    add_item_times, clamp_scale, create_timeline, get_process_from_timeline, gif_timeline,
    max_length, playing_indices, should_reset, update_time_offset, KeyFrame,
};
pub use controls_input::{
    drag_position, hit_items, item_rect, outside_drag_box, rect_contains, round_half_even,
    wheel_scale, HitItem,
};
pub use cosmetic_eulo::{
    cosmetic_eulo_text, get_cosmetic_gender, get_gear_type, get_gender, is_sub_weapon,
    replace_cosmetic_eulo,
};
pub use damage_skin::{DamageSkin, DamageSkinSection};
pub use db2::{image_rows as db2_image_rows, Db2Row};
pub use db2_bin::{BinCell, BinError, Db2BinGrid};
pub use db2_characters::{
    character_grid_rows as db2_character_grid_rows,
    character_grid_rows_for as db2_character_grid_rows_for, Db2CharacterRow,
};
pub use db2_damage_skins::{damage_skin_rows as db2_damage_skin_rows, Db2DamageSkinRow};
pub use db2_familiar::{familiar_rows, FamiliarLookups, FamiliarRow};
pub use db2_item_tabs::{
    item_tab_rows as db2_item_tab_rows, item_tab_tooltips as db2_item_tab_tooltips,
    load_string_image as db2_item_tab_string_image, Db2ItemTabRow,
};
pub use db2_items::{item_grid_rows as db2_item_grid_rows, Db2ItemRow};
pub use db2_maps::{
    map_grid_rows as db2_map_grid_rows, map_partition_rows as db2_map_partition_rows, Db2MapRow,
    MAP_PARTITIONS,
};
pub use db2_mobs::{
    mob_grid_rows as db2_mob_grid_rows, mob_partition_accepts,
    mob_partition_rows as db2_mob_partition_rows, Db2MobRow,
};
pub use db2_morphs::{morph_grid_rows as db2_morph_grid_rows, Db2MorphRow};
pub use db2_music::{music_grid_rows as db2_music_grid_rows, Db2MusicRow};
pub use db2_npcs::{npc_grid_rows as db2_npc_grid_rows, Db2NpcRow};
pub use db2_reactors::{reactor_grid_rows as db2_reactor_grid_rows, Db2ReactorRow};
pub use db2_skills::{skill_grid_rows as db2_skill_grid_rows, Db2SkillRow};
pub use db2_taming_mobs::{taming_mob_grid_rows as db2_taming_mob_grid_rows, Db2TamingMobRow};
pub use dotnet_metadata::{
    plugin_entries, plugin_types, read_assembly, DotnetAssembly, PluginEntryInfo, PublicType,
};
pub use draw_math::{
    border_path, flipped_origin, frame_sprite_call, gear_detail_number_glyphs, icon_border_path,
    multiply_rgb, non_premultiplied_hidef, opaque_blend, path_rects, rotation_bound,
    skill_dotline_tiles, sprite_destination, sprite_destination_scaled, src_alpha_mask,
    BlendFactor, BlendFactors, BlendFunction, GlyphDraw, SpriteCall,
};
pub use format_parser::{parse as parse_format, DocElement, Span};
pub use gear_stats::extract_gear_props;
pub use gear_type_names::gear_type_string;
pub use item_tooltip::{
    build_item_tooltip, extra_job_req_string, extra_job_req_string_list, get_attack_speed_string,
    get_gear_grade_string, get_mob_category_name, get_mob_skill_name, item_attribute_lines,
    item_category_name, item_core_spec_string, skill_req_amount, to_cjk_number_expr,
    to_thousands_number_expr, ItemTooltipView,
};
pub use item_tooltip_extra::{
    cant_account_sharable_tooltip, cash_tag_lines, exp_restriction_lines, exp_to_next_level,
    expire_date_lines, incline_line, item_int_props, pet_command_lines, point_cost_line,
    replace_desc_tags, replace_desc_tags_full, trade_available_line, RecipeRequirement, RecipeSpec,
};
pub use item_tooltip_layout::{
    layout_item, line_height, wrapped_lines, ItemLayout, ItemLayoutInput,
};
pub use linker::{
    extract_gear_stats, LoadMode, StringDomain, StringLinker, StringResult, StringRoot, StringRoots,
};
pub use main_window::{plugin_on_load, Command, CommandRegistry, OnLoadOutcome, Tab, TabHost};
pub use map::{
    BackItem, Foothold, FootholdTree, IlluminantCluster, LadderRope, LadderRopeKind, LifeKind,
    MapData, MapEvent, MapInfo, MapLife, MapLight, MapLight2D, MapParticle, MapPortal, MapReactor,
    MapTooltip, MapVirtualRect, MiniMapInfo, ObjItem, SkyWhale, TileItem,
};
pub use map_tooltip::{MapTooltipFields, MapTooltipText};
pub use node::{NodeId, NodeValue, RawDataRef};
pub use npc_quote::{message as npc_quote_message, parse as npc_quote_parse, NpcQuote};
pub use particle::{parse_particle_desc, ParticleDesc, ParticleTexture};
pub use pixel_ops::{blit_opaque_argb, compose_icon_block, enlarge_2x, IconBlock};
pub use plugin_events::{EventBus, InvalidRegion, InvalidateState, PluginEvent, Rect};
pub use plugin_find::{plugin_files, FoundNode, PluginMetadata, PluginWzSet};
pub use plugin_ribbon::{RibbonBar, RibbonError, RibbonModel, RibbonPanel};
pub use query::{
    execute_query, node_value_to_string, NumericComparisonMode, QueryExecutionResult, QueryResult,
    QueryRule, QueryRuleKind, QueryValueType, StringComparisonMode, ValueRule, DEFAULT_MAX_RESULTS,
};
pub use quest_tooltip::{QuestReward, QuestTooltip};
pub use resolve::{find_path, handle_uol, resolve_uol, resolve_uol_lossy, ResolveError};
pub use skill_summary::parse_skill_summary;
pub use skill_tooltip::SkillTooltip;
pub use sprite_compose::{compose_sprite, extract_rect, ComposeError};
pub use text_layout::{draw_plain_text, Align, LineRect};
pub use world_archive::{
    mob_lines as world_archive_mob_lines, npc_lines as world_archive_npc_lines,
    split_lines as world_archive_split_lines,
};
