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
| WzComparerR2.DB2 | 12 | 6,292 | `wz db2`, `wz db2-icons`, `wz db2-items`, `wz db2-item-tab`, `wz db2-maps`, `wz db2-mobs`, `wz db2-skills`, `wz db2-npcs`, `wz db2-reactors`, `wz db2-music`, `wz db2-damage-skins`, `wz db2-morphs`, `wz db2-taming-mobs`, `wz db2-characters`, GUI 표/이미지 뷰어 | 🟢 |
| CharaSimResource | 2 | 7,490 | — (WinForms 리소스) | ⬜ |
| WzComparerR2.Network | 28 | 1,954 | `wz-network` RC4/프레임/계약 JSON/TCP 로그인 루프백 | 🟡 |
| WzComparerR2.LuaConsole | 10 | 1,451 | — | ⬜ |
| WzComparerR2.PluginBase | 10 | 693 | `wz find-wz` (FindWz 경로 조회) | 🟡 |
| WzComparerR2.Updater | 3 | 276 | `wz patch` (부분) | 🟡 |

## 디렉터리별

### WzComparerR2 (main)

| 디렉터리 | 파일 | LOC | 대응 | 상태 |
|---|---:|---:|---|---|
| (root) WinForms 메인/비교 창 | 48 | 26,281 | `wz-gui` 최소 PoC | ⬜/🟡 |
| CharaSimControl | 40 | 21,299 | gear tooltip(base/set/potential/starforce), `GetGearPropString`/`GetJobName`/`GetFifthJobName`/`GetItemCoreSpecString` 전수 비교, Skill `SummaryParser` 요약, ItemTooltip 6탭 32,843개 속성, AchievementTooltip 1,579개, WorldArchiveTooltip 1,845개(NPC 대사 767), NPC/Reactor 툴팁 12,629개, DamageSkin 모델 402개, bitmap | 🟡 |
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
| OpenAPI | 4 | 2,086 | `wz-openapi` 오프라인 helpers(Decrypt/CheckVer/GetItemID), HTTP는 API 키 필요 | 🟡 |
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
   Face(7424), Mechanic(23), Dragon(16), Ring(1127), Android(215),
   ArcaneForce(7), AuthenticForce(8), Bits(79), EquipBag(2) 스물한 카테고리를
   실데이터 비교했고, DB2 전체 화면 이미지 뷰어(GUI)도 구현했다. 나머지 8개
   전용 그리드는 미구현.
6. `WzComparerR2.Common/Controls` (2,138 LOC) — 미구현.
   `OpenAPI` (2,086 LOC) 중 네트워크 없이 검증 가능한 `Decrypt`,
   `CheckVer`, `GetItemID`를 `wz-openapi`로 옮겨 C# 실행 결과와 비교했다.
   캐릭터 조회/아바타 API 호출은 Nexon API 키가 필요해 제외한다.
7. `LuaConsole` (1,451) — 미구현.
   `PluginBase` (693) 중 `PluginManager.FindWz` 경로 조회를 `wz find-wz`로
   옮겼다. 타입별 WZ 등록 후 세그먼트를 따라가며 `.img`를 추출하는 원본
   동작과 57개 실제 경로가 C# WzLib `FindNodeByPath(true, ...)`와 일치한다.
   플러그인 DLL 로딩/메뉴 주입은 WinForms 호스트가 필요해 제외한다.
   `Network` (1,954) 중 RC4 스트림, `EncodePack` 프레이밍, 패킷 계약 JSON
   직렬화를 `wz-network`로 옮겼다. RC4는 C# `RC4CryptoServiceProvider`와
   12개 벡터, 패킷은 `WcClient.EncodePack`과 7개 계약(하트비트/로그인/채팅/
   암호화 키)에서 바이트 단위로 일치한다. TCP 프레이밍/RC4 적용/로그인 상태
   머신(`ClientSession`)은 로컬 루프백 테스트와 `wz net-loopback`으로
   `Connected→CryptoRequested→Ready→LoginSent→LoggedIn` 전이를 검증했다.
   C# `WcClient`는 WinForms/PluginBase 로드가 필요해 macOS에서 인스턴스화할
   수 없으므로, 프로토콜 검증은 C#과 바이트 일치가 확인된 패킷 코덱 fixture를
   공유하는 방식으로 수행했다. 재접속/실서버 연동은 제외한다.

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
- DB2 Item 6탭 (`Item/<dir>` 전체): C# `LoadItem` 의미 그대로 구현해
  Cash 3539 / Consume 18377 / Etc 5238 / Install 3851 / Pet 614 / Special 1224
  = **32,843행 완전 일치** (id/name/desc/has_icon, 해시 fixture).
- 데이터 부재 재확인: `wz parity-scan Data` → Skill common 0, WorldMap
  QuestLimit/Fog 0, Familiar 파일 0, BIN 파일 0. weather는 원본 C# 소스에
  구현이 없다.
- PluginBase `FindWz`: 57개 경로 C# 일치.
- Network 오프라인: RC4 12개 벡터 + 프레임 코덱 자체 테스트.
- OpenAPI 오프라인 helpers: C# `Utils.Decrypt`/`CheckVer`/`GetItemID`를
  16/32/64바이트 벡터에서 비교해 완전 일치(`wz-openapi` 통합 테스트).
- `DamageSkinTooltipRenderer`는 텍스트 라인이 없고 ID/샘플만 그린다. 대신
  `DamageSkin.CreateFromNode`의 비트맵 제외 모델(ID/desc/extractID/customType/
  섹션 키·spacing·bottomFix·dotSpace)을 옮겨 **402개 스킨**이 C#과 일치.
- `NpcTooltipRenderer`/`ReactorTooltipRenderer` 텍스트: 이름/`ID:`/
  `등장위치 :`/` 맵이름(mapID)`·` 불명`, `이름:`/`클립: n개`/`액션:`를
  실제 데이터 **12,629개**(NPC 12,188 + Reactor 441)에서 C#과 일치.
- `WorldArchiveTooltipRender` 텍스트: `worldArchive.img`(Etc_002) +
  `StringLinker.StringWorldArchiveMob/Npc` + `StringMonsterBook`로 몹/NPC
  설명, `[몬스터북]`, `NPC 대사` 라인을 구성해 **1,845개 항목**
  (NPC 대사 767개)이 C#과 완전 일치.
- `AchievementTooltipRenderer` 텍스트: `Achievement.CreateFromNode` +
  툴팁 라인을 옮겨 `AchievementData_000.wz` **1,579개 업적**의 13개 필드와
  라인 순서가 C#과 완전 일치.
- `ItemStringHelper` 순수 텍스트: `GetJobName` 417개, `GetFifthJobName`
  198조합, `GetItemCoreSpecString` 408조합이 C#과 완전 일치.
- `ItemStringHelper.GetGearPropString` 전수 비교: Coat 228, Weapon 1262,
  Cap 965, Accessory 1576, Face 4, Ring 978, TamingMob 89 — 총 5,102개
  (key,value) 쌍의 문자열이 C#과 완전 일치. `reqLevel`/`tuc`/`incACC`/
  `incEVA`/`incMDD`는 원본에서 null이므로 툴팁 전용 `tooltip_stat_line`으로
  분리했다.
- DB2 Character/Mechanic grid `Mechanic_000.wz`: C# 23 rows == Rust
  `wz db2-characters` 23 rows (동일 컬럼 완전 일치).
- DB2 Character/Dragon grid `Dragon_000.wz`: C# 16 rows == Rust
  `wz db2-characters` 16 rows (동일 컬럼 완전 일치).
- DB2 Character/Ring grid `Ring_000.wz`: C# 1127 rows == Rust
  `wz db2-characters` 1127 rows (동일 컬럼 완전 일치).
- Afterimage는 이미지명이 `0`으로 시작하지 않아 C#/Rust 모두 0행으로 일치.
- Skill summary `String/Skill.img`: C# `SummaryParser.GetSkillSummary` 20629
  rows == Rust `wz skill-summary` 20629 rows (desc + h/h1..hn 완전 일치).
- ItemTooltip `02000012`: C# `GetItemAttributeString` + StringLinker output ==
  Rust `wz item-tooltip` (name/desc/auto_desc/attributes 완전 일치).
- ItemTooltip 6탭 전수: Cash/Consume/Etc/Install/Pet/Special **32,843개
  아이템**의 name/desc/attributes가 C#과 완전 일치 (해시 fixture). 두 문자열
  (`sharableOnce`, `accountSharableAfterExchange`)의 개행 위치도 원본과
  맞췄다.

## 남은 작업

- DB2/CharaSimControl/Comparer HTML/EasyComparer 등 UI·리포팅 모듈.
- Common Controls, LuaConsole.
- PluginBase의 플러그인 로딩/메뉴 주입(WinForms 호스트 필요).
- Network TCP/재접속/로그인 상태 머신(서버 필요).
- OpenAPI HTTP 호출(Nexon API 키 필요).
- WorldMap 실제 QuestLimit/Fog 데이터는 클라이언트에 없음.
- weather는 원본 C# 로더/에셋에 없음.

## DB2 전체 화면 이미지 뷰어

- GUI Catalog의 `선택 이미지 DB2 전체 화면 뷰어` 버튼이 선택 이미지에서 가장 큰
  Canvas를 디코딩해 별도 창에 표시한다. zoom 슬라이더(0.1..=8.0), 100% 버튼,
  양방향 스크롤을 지원한다.
- `crates/wz-gui/src/panels/catalog.rs`의 `egui::Window` + `ScrollArea` 구현.
