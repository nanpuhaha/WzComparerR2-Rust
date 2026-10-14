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

pub mod arena;
pub mod calc;
pub mod chara_prop;
pub mod linker;
pub mod map;
pub mod node;
pub mod query;
pub mod resolve;

#[cfg(test)]
mod tests;

pub use arena::Arena;
pub use calc::{evaluate_expression, CalcError};
pub use chara_prop::CharaProp;
pub use linker::{extract_gear_stats, StringLinker, StringResult};
pub use map::{
    BackItem, Foothold, FootholdTree, LadderRope, MapData, MapInfo, MapLife, MapPortal,
    MapVirtualRect, ObjItem, TileItem,
};
pub use node::{NodeId, NodeValue, RawDataRef};
pub use query::{
    execute_query, node_value_to_string, NumericComparisonMode, QueryExecutionResult, QueryResult,
    QueryRule, QueryRuleKind, QueryValueType, StringComparisonMode, ValueRule, DEFAULT_MAX_RESULTS,
};
pub use resolve::{find_path, handle_uol, resolve_uol, resolve_uol_lossy, ResolveError};
