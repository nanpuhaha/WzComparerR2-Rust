# C# 솔루션 전체 파일 커버리지 감사

원본: `/Users/jwseo/GitHub/WzComparerR2-seotbeo` (macOS, .NET 10).
LOC는 `*.cs`에서 `obj/`, `bin/`을 제외한 줄 수다.

상태: ✅ 구현+검증, 🟡 부분, ⬜ 미구현(UI/플랫폼 전용 포함).

## 프로젝트 요약

| 프로젝트 | 파일 | LOC | Rust 대응 | 상태 |
|---|---:|---:|---|---|
| WzComparerR2.WzLib | 54 | 13,513 | `wz-core`, `wz-image`, `wz-pool` | ✅ (핵심) |
| WzComparerR2.Common | 146 | 23,950 | `wz-db`, `wz-avatar`, `wz-render` | 🟡 |
| WzComparerR2.MapRender | 124 | 29,291 | `wz-db::map`, `wz-gui`, `wz-render` | 🟡 |
| WzComparerR2.Avatar | 13 | 7,055 | `wz-avatar` | ✅ |
| WzComparerR2 (main) | 150 | 61,035 | `wz-cli`, `wz-gui` | 🟡 |
| WzComparerR2.DB2 | 12 | 6,292 | `wz db2`, `wz db2-icons`, `wz db2-items`, `wz db2-maps`, `wz db2-mobs`, `wz db2-skills`, `wz db2-npcs`, `wz db2-reactors`, `wz db2-music`, `wz db2-damage-skins`, `wz db2-morphs`, `wz db2-taming-mobs`, `wz db2-characters`, GUI tables | 🟡 |
| CharaSimResource | 2 | 7,490 | — (WinForms 리소스) | ⬜ |
| WzComparerR2.Network | 28 | 1,954 | — | ⬜ |
| WzComparerR2.LuaConsole | 10 | 1,451 | — | ⬜ |
| WzComparerR2.PluginBase | 10 | 693 | — | ⬜ |
| WzComparerR2.Updater | 3 | 276 | `wz patch` (부분) | 🟡 |

## 디렉터리별

### WzComparerR2 (main)

| 디렉터리 | 파일 | LOC | 대응 | 상태 |
|---|---:|---:|---|---|
| (root) WinForms 메인/비교 창 | 48 | 26,281 | `wz-gui` 최소 PoC | ⬜/🟡 |
| CharaSimControl | 40 | 21,299 | gear tooltip(base/set/potential/starforce), Skill `SummaryParser` 요약, ItemTooltip 속성 태그, bitmap | 🟡 |
| AvatarCommon | 12 | 4,086 | `wz-avatar` | ✅ |
| Comparer | 6 | 3,080 | `wz-diff` + `virtual_node` | 🟡 |
| Patcher | 16 | 2,547 | `wz-core::patch`, `wz patch` | ✅ core/CLI, UI ⬜ |
| CharaSim | 5 | 1,957 | `wz-avatar::stats`, `gear_tooltip`, `skill_summary` | 🟡 |
| Config | 16 | 1,086 | — | ⬜ |
| SoundPlayer | 5 | 400 | `wz-gui::sound_player` | ✅ |

### WzComparerR2.Common

| 디렉터리 | 파일 | LOC | 대응 | 상태 |
|---|---:|---:|---|---|
| CharaSim | 54 | 10,121 | `wz-avatar::stats` | 🟡 (Gear/Set/Potential/MaxStar) |
| (root) | 15 | 2,832 | `wz-db` | 🟡 |
| Rendering | 25 | 2,726 | `wz-render`, `wz-image` | 🟡 (light/water/sprite) |
| Animation | 15 | 2,415 | `wz-avatar` frame loader | 🟡 |
| Controls | 16 | 2,138 | — | ⬜ |
| OpenAPI | 4 | 2,086 | — | ⬜ |
| Text | 4 | 707 | `wz-core::text` | ✅ |
| Encoders | 6 | 534 | `wz-image` | ✅ (PNG/BC7) |
| Config | 6 | 355 | — | ⬜ |

### WzComparerR2.MapRender

| 디렉터리 | 파일 | LOC | 대응 | 상태 |
|---|---:|---:|---|---|
| (root) MapData/Mesh/Scene | 53 | 16,268 | `map.rs`, `wz-gui` | 🟡 |
| UI | 31 | 8,753 | minimap/worldmap 패널 일부 | 🟡 |
| Patches2 | 20 | 1,573 | background/life/reactor/portal/particle | ✅ |
| Patches | 10 | 855 | 대부분 Patches2로 대체 | 🟡 |
| Effects | 3 | 685 | `water.rs`, `light.rs` | ✅/🟡 |
| Animation | 4 | 410 | `wz-avatar`/frame | 🟡 |

### WzComparerR2.WzLib

| 디렉터리 | 파일 | LOC | 대응 | 상태 |
|---|---:|---:|---|---|
| (root) | 30 | 6,152 | `wz-core` | ✅ |
| Compatibility | 9 | 3,185 | MS/PKG2 일부 | 🟡 |
| Utilities | 12 | 3,005 | `wz-pool`/`wz-core` | ✅ |
| Cryptography | 2 | 1,135 | `wz-core::crypto` | ✅ |

## 가장 큰 미구현/부분 모듈

1. `WzComparerR2` root WinForms UI (26,281 LOC) — Rust egui PoC만 존재.
2. `CharaSimControl` (21,299 LOC) — 비트맵 툴팁 일부만.
3. `WzComparerR2.Common/CharaSim` (10,121 LOC) — Gear/Set/Potential/MaxStar만.
4. `MapRender/UI` (8,753 LOC) — Minimap/WorldMap 일부.
5. `WzComparerR2.DB2` (6,292 LOC) — node table + icon/image viewer + Item/Map/
   Mob/Skill/NPC/Reactor/Music/DamageSkin/Morph/TamingMob/Character grid 구현
   (`wz db2`, `wz db2-icons`, `wz db2-items`, `wz db2-maps`, `wz db2-mobs`,
   `wz db2-skills`, `wz db2-npcs`, `wz db2-reactors`, `wz db2-music`,
   `wz db2-damage-skins`, `wz db2-morphs`, `wz db2-taming-mobs`,
   `wz db2-characters`, GUI Catalog). Character는 Coat(682), Cap(3166),
   Shoes(1703), Accessory(2369), Longcoat(2493), Glove(661), Pants(579),
   Shield(142), Cape(1139), PetEquip(511), Weapon(5655), Hair(9004),
   Face(7424), Mechanic(23) 열네 카테고리를 실데이터 비교했고, 나머지 15개
   전용 그리드와 전체 화면 이미지 뷰어 UI는 미구현.
6. `WzComparerR2.Common/Controls` (2,138 LOC), `OpenAPI` (2,086 LOC) — 미구현.
7. `LuaConsole` (1,451), `Network` (1,954), `PluginBase` (693) — 미구현.

## 이번 턴 추가 구현

- `wz-diff::virtual_node`: 원본 `WzVirtualNode.Combine` 병합 규칙
  (동일 이름 디렉터리 재귀 병합, 값/디렉터리 충돌 오류).
- C# `WzLib` 실행 하네스 `tools/csharp-worldmap`:
  - WorldMap QuestLimit/Fog fixture JSON 일치
  - 실제 WorldMap 85 images / QuestLimit 0 / Fog 0
  - `000010000.img` C# 4721 nodes, Rust oracle 4722 = outer Directory wrapper.
    per-node path multiset는 wrapper 제거 후 4721 = 4721로 일치.
- DB2 `01302000.img`: C# WzLib 270 rows == Rust `wz db2` 270 rows
  (path/type/value/children 완전 일치).
- DB2 Item grid `0200.img`: C# `StringLinker` 530 rows == Rust `wz db2-items`
  530 rows (id/name/desc/has_icon 완전 일치).
- DB2 Map grid `Map0_000.wz`: C# `StringLinker.StringMap` 87 rows == Rust
  `wz db2-maps` 87 rows (id/street/map/minimap/link 완전 일치).
- DB2 Mob grid `_Canvas_011.wz`: C# `StringLinker.StringMob` 55 rows == Rust
  `wz db2-mobs` 55 rows (id/name/stand/fly/move/hit/die/thumbnail/link 완전 일치).
- DB2 Skill grid `_Canvas_000.wz`: C# `StringLinker.StringSkill/StringSkill2`
  165 rows == Rust `wz db2-skills` 165 rows
  (id/row_type/book/name/desc/icon/common/h_count 완전 일치).
- DB2 NPC grid `_Canvas_000.wz`: C# `StringLinker.StringNpc` 943 rows == Rust
  `wz db2-npcs` 943 rows (id/name/stand/default/link 완전 일치).
- DB2 Reactor grid `_Canvas_000.wz`: C# WzLib 441 rows == Rust
  `wz db2-reactors` 441 rows (id/icon/info/link 완전 일치).
- DB2 Music grid `Sound_000.wz`: C# WzLib 40 rows == Rust `wz db2-music`
  40 rows (path/data_len/duration 완전 일치).
- DB2 DamageSkin grid `Consume_000.wz`: C# `StringLinker.StringItem` 952 rows
  == Rust `wz db2-damage-skins` 952 rows
  (id/name/desc/icon_image/has_sample 완전 일치).
- DB2 Morph grid `Consume_000.wz` + `Morph_000.wz`: C# 234 rows == Rust
  `wz db2-morphs` 234 rows
  (id/name/desc/morph_id/has_icon/has_morph 완전 일치).
- DB2 TamingMob grid `TamingMob_000.wz`: C# `StringLinker.StringEqp`
  1265 rows == Rust `wz db2-taming-mobs` 1265 rows
  (id/name/desc/h1/icon/reqLevel/reqJob/tuc/islot/tamingMob 완전 일치).
- DB2 Character grid `Coat_000.wz`: C# `StringLinker.StringEqp` 682 rows ==
  Rust `wz db2-characters` 682 rows
  (id/name/desc/h1/icon/reqLevel/reqJob/tuc/islot/vslot/stat 완전 일치).
- DB2 Character/Cap grid `Cap_000.wz`: C# 3166 rows == Rust
  `wz db2-characters` 3166 rows (동일 컬럼 완전 일치).
- DB2 Character/Shoes grid `Shoes_000.wz`: C# 1703 rows == Rust
  `wz db2-characters` 1703 rows (동일 컬럼 완전 일치).
- DB2 Character/Accessory grid `Accessory_000.wz`: C# 2369 rows == Rust
  `wz db2-characters` 2369 rows (동일 컬럼 완전 일치).
- DB2 Character/Longcoat grid `Longcoat_000.wz`: C# 2493 rows == Rust
  `wz db2-characters` 2493 rows (동일 컬럼 완전 일치).
- DB2 Character/Glove grid `Glove_000.wz`: C# 661 rows == Rust
  `wz db2-characters` 661 rows (동일 컬럼 완전 일치).
- DB2 Character/Pants grid `Pants_000.wz`: C# 579 rows == Rust
  `wz db2-characters` 579 rows (동일 컬럼 완전 일치).
- DB2 Character/Shield grid `Shield_000.wz`: C# 142 rows == Rust
  `wz db2-characters` 142 rows (동일 컬럼 완전 일치).
- DB2 Character/Cape grid `Cape_000.wz`: C# 1139 rows == Rust
  `wz db2-characters` 1139 rows (동일 컬럼 완전 일치).
- DB2 Character/PetEquip grid `PetEquip_000.wz`: C# 511 rows == Rust
  `wz db2-characters` 511 rows (동일 컬럼 완전 일치).
- DB2 Character/Weapon grid `Weapon_000.wz`: C# 5655 rows == Rust
  `wz db2-characters` 5655 rows (동일 컬럼 완전 일치).
- DB2 Character/Hair grid `Hair_000.wz`: C# 9004 rows == Rust
  `wz db2-characters` 9004 rows, `default/hairOverHead` 아이콘 경로 포함 일치.
- DB2 Character/Face grid `Face_000.wz`: C# 7424 rows == Rust
  `wz db2-characters` 7424 rows, `default/face` 아이콘 경로 포함 일치.
- DB2 Character/Mechanic grid `Mechanic_000.wz`: C# 23 rows == Rust
  `wz db2-characters` 23 rows (동일 컬럼 완전 일치).
- Skill summary `String/Skill.img`: C# `SummaryParser.GetSkillSummary` 20629
  rows == Rust `wz skill-summary` 20629 rows (desc + h/h1..hn 완전 일치).
- ItemTooltip `02000012`: C# `GetItemAttributeString` + StringLinker output ==
  Rust `wz item-tooltip` (name/desc/auto_desc/attributes 완전 일치).

## 남은 작업

- DB2/CharaSimControl/Comparer HTML/EasyComparer 등 UI·리포팅 모듈.
- Common Controls/OpenAPI, LuaConsole, Network, PluginBase.
- WorldMap 실제 QuestLimit/Fog 데이터는 클라이언트에 없음.
- weather는 원본 C# 로더/에셋에 없음.
