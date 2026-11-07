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
| WzComparerR2.DB2 | 12 | 6,292 | `wz db2`, `wz db2-icons`, `wz db2-items`, `wz db2-item-tab`, `wz db2-maps`, `wz db2-map-part`, `wz db2-mobs`, `wz db2-mob-part`, `wz db2-skills`, `wz db2-npcs`, `wz db2-reactors`, `wz db2-music`, `wz db2-damage-skins`, `wz db2-morphs`, `wz db2-taming-mobs`, `wz db2-characters`, GUI 표/이미지 뷰어 | 🟢 |
| CharaSimResource | 2 | 7,490 | — (WinForms 리소스) | ⬜ |
| WzComparerR2.Network | 28 | 1,954 | `wz-network` RC4/프레임/계약 JSON/TCP 로그인 루프백 | 🟡 |
| WzComparerR2.LuaConsole | 10 | 1,451 | `wz-lua` 샌드박스 + NLua CLR 브리지 실행 대조 | 🟡 |
| WzComparerR2.PluginBase | 10 | 693 | `wz find-wz`, `wz plugin-files`, PE 메타데이터, `LoadPlugin` 인스턴스화 | 🟡 |
| WzComparerR2.Updater | 3 | 276 | `wz patch` (부분) | 🟡 |

## 디렉터리별

### WzComparerR2 (main)

| 디렉터리 | 파일 | LOC | 대응 | 상태 |
|---|---:|---:|---|---|
| (root) WinForms 메인/비교 창 | 48 | 26,281 | `wz-gui` 최소 PoC | ⬜/🟡 |
| CharaSimControl | 40 | 21,299 | gear tooltip(base/set/potential/starforce), `GetGearPropString`/`GetJobName`/`GetFifthJobName`/`GetItemCoreSpecString` 전수 비교, Skill `SummaryParser` 요약, ItemTooltip 6탭 32,843개 속성, AchievementTooltip 1,579개, WorldArchiveTooltip 1,845개(NPC 대사 767), NPC/Reactor 툴팁 12,629개, DamageSkin 모델 402개, Map 툴팁 87개, Quest 모델 18,964개, 레시피 827개, 캐시 태그 2,503개, desc_leftalign/pointCost/cant 1,242개, 성향/trade 49개, 레시피 제한 1,039개, 사용기간 449개, bitmap | 🟡 |
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
| Controls | 16 | 2,138 | `wz-db::common_controls`(타임라인/GIF), `controls_input`(히트테스트/드래그/휠), `draw_math`(숫자/경로/보더) | 🟡 |
| OpenAPI | 4 | 2,086 | `wz-openapi` helpers + AvatarCodeStructure/Unpack + HTTP 계층(목 서버 + 실 엔드포인트 오류 경로 검증) | 🟡 |
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
   실데이터 비교했고, DB2 전체 화면 이미지 뷰어(GUI)도 구현했다.
   Map 탭 3분할(탭17 Map0–3, 탭18 Map4–8, 탭19 Map9)과 `info/link` 기반
   미니맵 2차 패스를 구현해 **17,469행**(4,382 / 1,946 / 11,141)을 C#과
   비교했고, Mob 탭 3분할(선행 숫자 8/9 제외·8·9)로 **3,487행**
   (742 / 1,160 / 1,585)을 비교했다.
   Familiar는 `Character/Familiar`가 0개라 행 규칙만 구현하고 합성
   fixture로 검증했고, BIN 그리드 컨테이너(`SaveBin`/`LoadBin`)는 C#
   `BinaryWriter`/`BinaryReader`와 **바이트 단위로 일치**한다(`.BIN` 파일은
   0개).
6. `WzComparerR2.Common/Controls` (2,138 LOC) — GDI/SharpDX 비의존 순수 로직을
   `wz-db::common_controls`로 옮겼다. `FrameAnimator.CreateTimeline`/
   `GetProcessFromTimeline`, `AnimationRecoder.GetGifTimeLine`/`GetMaxLength`,
   `AnimationControl.AddItem`/`UpdateTimeOffset`/재생 리셋/`GlobalScale`.
   나머지(그래픽 장치, 스프라이트 배치, 마우스 드래그, XNA `Timer`,
   `AnimationRecoder` 렌더 타깃/GIF 인코딩)는 Device/Texture2D 의존이다.
   `OpenAPI` (2,086 LOC) 중 네트워크 없이 검증 가능한 `Decrypt`,
   `CheckVer`, `GetItemID`를 `wz-openapi`로 옮겨 C# 실행 결과와 비교했다.
   추가로 `AvatarCodeStructure` v26–44 구조를 C#에서 덤프해
   `Utils.Unpack`(비트 읽기 + ExpandOffset 삽입)을 구현했고, 19개 버전 ×
   2개 팩 38케이스가 C# 결과와 일치한다. 캐릭터 조회/아바타 HTTP 호출은
   Nexon API 키가 필요해 제외한다.
7. `LuaConsole` (1,451) — 미구현.
   `PluginBase` (693) 중 `PluginManager.FindWz` 경로 조회를 `wz find-wz`로
   옮겼다. 타입별 WZ 등록 후 세그먼트를 따라가며 `.img`를 추출하는 원본
   동작과 57개 실제 경로가 C# WzLib `FindNodeByPath(true, ...)`와 일치한다.
   추가로 `PluginManager.GetPluginFiles`(`Plugin/**/WzComparerR2.*.dll`)와
   `PluginEntry` 메타데이터 파생 규칙(Title/Company/Version/Informational→File)을
   구현해 C# 탐색 결과와 4개 fixture에서 일치했다.
   어셈블리 타입 열거를 위해 최소 .NET PE/CLI 메타데이터 리더를 구현했다
   (`wz-db::dotnet_metadata`): 어셈블리 이름/버전, AssemblyTitle/Company/
   Informational/FileVersion, public 타입/기반 타입/추상 여부. fixture
   어셈블리에서 C# `Assembly.GetExportedTypes()` 결과(Alpha/Derived)와 완전
   일치한다. 인스턴스화(`Activator` + `PluginContext`)와 메뉴 주입은 WinForms
   호스트가 필요해 여전히 미구현이다.
   `Network` (1,954) 중 RC4 스트림, `EncodePack` 프레이밍, 패킷 계약 JSON
   직렬화를 `wz-network`로 옮겼다. RC4는 C# `RC4CryptoServiceProvider`와
   12개 벡터, 패킷은 `WcClient.EncodePack`과 7개 계약(하트비트/로그인/채팅/
   암호화 키)에서 바이트 단위로 일치한다. TCP 프레이밍/RC4 적용/로그인 상태
   머신(`ClientSession`)은 로컬 루프백 테스트와 `wz net-loopback`으로
   `Connected→CryptoRequested→Ready→LoginSent→LoggedIn` 전이를 검증했다.
   C# `WcClient`는 WinForms/PluginBase 로드가 필요해 macOS에서 인스턴스화할
   수 없으므로, 프로토콜 검증은 C#과 바이트 일치가 확인된 패킷 코덱 fixture를
   공유하는 방식으로 수행했다. 추가로 하트비트 에코(수신 즉시 회신)와
   재접속/백오프(`ReconnectingClient`, 시도별 지연·상한)를 구현해 루프백
   테스트로 검증했다. 실서버 연동은 제외한다.

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
- OpenAPI 아바타 비트팩: v26–44 38케이스 C# 일치.
- OpenAPI 실 엔드포인트: 미인증/오류 키 요청을 실제로 보내
  `OPENAPI00004`/`OPENAPI00005` 봉투를 캡처하고 파서·URL·헤더를 고정.
  성공 경로는 API 키 필요.
- OpenAPI 오프라인 helpers: C# `Utils.Decrypt`/`CheckVer`/`GetItemID`를
  16/32/64바이트 벡터에서 비교해 완전 일치(`wz-openapi` 통합 테스트).
- ItemTooltip 사용기간: `consumableFrom`/`endUseDate`를 `년/월/일/시/분`
  형식으로 포맷해 Consume 418·Cash 19·Install 2·Etc 10 = **449개**가
  C#과 일치. `timeLimited`/`life`는 원본이 `DateTime.Now`를 쓰므로 제외.
- ItemTooltip 레시피 `< 사용 제한조건 >`/`· 스킬 N레벨 이상` 라인을
  Consume **1,039개**에서 C#과 일치(장비제작/장신구제작 띄어쓰기 포함).
- ItemTooltip 성향 경험치와 `tradeAvailable`(카르마) 라인을 Consume 26 +
  Install 23 = **49개**에서 C#과 일치.
- ItemTooltip `desc_leftalign`(1359개 중 `#t` 25개 치환), `pointCost`,
  `cantAccountSharable/tooltip`을 Cash 91 + Consume 1,151 = **1,242개**에서
  C#과 일치. `#cosmetic_EULO#`는 데이터에 없어 미구현.
- ItemTooltip 캐시/기간/펫 제한 태그: `cash/noMoveToLocker/onlyCash/
  cashTradeBlock/flatRate/noScroll/noRevive`와 5533 미리보기 문구를
  Cash **2,503개**에서 C#과 일치.
- ItemTooltip 추가 텍스트: `spec/recipeValidDay`·`recipeUseCount` 라인을
  Consume **827개**에서 C#과 일치. `exp_minLev`와 펫 `interact` 명령어는
  규칙을 구현했지만 클라이언트에 데이터가 0개다(`wz parity-scan`).
- `QuestTooltipRenderer` 모델: 이름/설명 0~2/차단/추천 레벨/수요/Check1
  몹·아이템·infoex/Act1 보상(exp·meso·pop·친밀도·성향·아이템·SP)을
  `Quest.CreateFromNode`·`QuestReward.CreateFromAct1Node`에서 아이콘만 제외해
  옮겼고, `QuestData_000.wz` **18,964개**가 C#과 일치.
- `MapTooltipRenderer` 텍스트: 마을/맵 이름, `ID:`, 설명, BGM, 몹/NPC
  이름 목록을 `Map.CreateFromNode` 필드와 `StringMap`으로 구성해
  `Map0_000.wz` **87개**가 C#과 일치.
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
- ItemTooltip 아이콘 픽셀: `GearGraphics.EnlargeBitmap`(2배 최근접 확대),
  `ItemTooltipRender22` 배치식(`iconX+6+(1-originX)*2`,
  `picH+6+(33-originY)*2`), 불투명 blit, 그리고 **정수 straight-alpha
  source-over**까지 실제 클라이언트 아이콘 6개(27×30/27×27, origin `[-3,30]`)로
  비교해 캔버스 SHA-256이 완전 일치. GDI+의 premultiplied/감마 합성과 텍스트
  래스터는 macOS에서 실행 불가라 남는다.
- LuaConsole 취소: `DoStringAsync`의 디버그 훅(매 명령어) 취소 계약을 옮기고
  fixture 4케이스(즉시 취소/정상 완료/N번째 훅 취소/사전 취소)와 취소 후 샌드박스
  재사용을 검증.
- LuaConsole 출력/이력: `env:Write/WriteLine`(C# `string.Format`)를 포팅하고
  self 인자 제거·출력 버퍼·명령 이력(↑/↓)을 구현. fixture 14케이스 + GUI
  테스트로 검증.
- LuaConsole 콘솔 패널: 검증된 `wz_lua` 샌드박스로 청크를 실행하고 반환값/오류를
  이력으로 렌더. `ExecutionOutcome`/`ConsoleModel` 단위 테스트 5종과 스모크
  (`lua console values=2 [3, lua]`).
- LuaConsole 에디터 거터/커서: 줄 번호 거터, 커서→줄/열/토큰, 선택 토큰 종류,
  소스 변경 시에만 재토큰화. GUI 테스트 3종 + 스모크.
- OpenAPI 성공 파싱: 공식 스키마 fixture로 200 본문 파싱(12필드 + 아바타 코드
  추출)을 고정. 실 200 캡처는 API 키 필요.
- LuaConsole 에디터 패널: 검증된 토크나이저 결과를 egui `LayoutJob`으로 옮겨
  `Lua.xshd` 색(5종)으로 렌더. 색 매핑 fixture + GUI 테스트 2종 + 스모크
  (`tokens=125 colored=57 colors=5`).
- LuaConsole 문법 강조: `Lua.xshd`의 주석/문자열/키워드/구두점/숫자 규칙을
  토크나이저로 옮기고, 원본 xshd를 파싱한 C# 스캐너와 원본 예제 5개의
  토큰 스트림(수 + SHA-256)이 완전 일치. 에디터 UI/색 렌더링은 남는다.
- 서식 문자열 파서: `Text.Parser.Parse`를 이식하고 실제 `String_000.wz`의
  태그 포함 문자열 151개(882 run)를 **원본 C# `Parser.Parse` 실행 결과**와
  비교해 완전 일치. `#fc` 오류 시 토큰 글자가 남는 동작, `\r` 무시, 끝 `\`
  처리 같은 특이점까지 고정.
- ItemTooltip 색상 run: `item22ColorTable`(c/$r/$g/$S/#fc)과 상단·하단·유효기간
  블록을 오프스크린 렌더에 반영, 픽셀이 색 토큰에 반응함을 테스트로 고정.
- ItemTooltip 오프스크린 렌더: 파서→배치→아이콘 픽셀 연산→게임 폰트를 합쳐
  300×153 RGBA를 만들고 SHA-256을 fixture로 고정. GUI 인스펙터가 이 경로를
  사용한다. GDI+ 감마/힌팅과의 픽셀 동일성은 주장하지 않는다.
- `GetGearPropString22`: 2열 라벨/값 포맷(중첩 switch·null 원소·단일 라벨 케이스)을
  옮기고 실제 장비 54케이스 × 3 flag = 162행을 실제 C# 호출과 비교.
- `GetGearPropString` signFlag: 0/1/2 부호 규칙(음수에서 `+-5`)을 옮기고 실제
  장비 속성 54케이스 × 3 flag = 162행을 실제 C# 호출과 비교.
- 세트 아이템: `SetItem.CreateFromNode`의 파트/이펙트/Option/OptionToMob/
  activeSkill/bonusByTime/Desc 병합을 옮기고 실제 SetItemInfo.img 644세트
  구조를 실제 C# 호출과 완전 일치. `Potential.from_node`도 함께 이식.
- 잠재능력 코어: `Potential`의 요약 치환(길이 내림차순)/레벨/옵션타입/
  `ToString`/`IsPotentialEx`를 옮기고 실제 ItemOption.img 600케이스 + 옵션
  판정 1,127건 + 레벨 215건을 실제 C# 호출과 비교.
- 아이템 분류 판정: `Item.GetItemType`(백만 단위, 5xxx Pet/Cash 구분)을 옮기고
  실제 아이템 ID 598개 + 경계 18개를 실제 C# 호출과 616행 비교.
- 장비 직업 제한: `GetExtraJobReqString(GearType…)` 51그룹과
  `GetAstraExtraJobReqString`(직업명 `(N차)` 제거)을 옮기고 GearType 161개 ×
  4시나리오 = 644행을 실제 C# 호출과 비교.
- 장비 타입명: `GetGearTypeString`의 139그룹/156이름을 옮기고 GearType 전체
  161개 이름에 대해 실제 C# 호출과 비교.
- 3열 비교/직업 제한: `GetGearPropDiffString22`, `GetExtraJobReqString(int)`,
  `GetExtraJobReqStringList`(일반/MSN)을 옮기고 실제 C# 호출과 45+35+12케이스 비교.
  빈 MSN 목록은 원본이 DivideByZero지만 Rust는 None으로 처리함을 문서화.
- 소형 포맷터: 몹 분류/공격속도/등급/비교 문자열을 옮기고 실제 C# 호출과
  12+13+7+45 케이스를 비교.
- 가격 축약/몹 스킬명: `ToThousandsNumberExpr`(KMS 소수 1자리, MSEA Q/T/B/M/K)과
  `GetMobSkillName`을 옮기고 실제 C# 호출과 142×2 + 63 케이스를 비교.
- 가격/수치 표기: `ToCJKNumberExpr`의 plain/detailed 두 모드를 옮기고 실제
  아이템 가격 130케이스(260문자열)를 실제 C# 호출과 비교.
- 스킬 요구량 라벨: `GetSkillReqAmount`의 만 단위 분기/중국어 라벨을 옮기고
  실제 C# 호출과 13케이스 × 2 요구량 = 26행 비교.
- 스킬 점선 타일 기하: `DrawV6SkillDotline`의 브러시 타일/clip 규칙을 옮기고
  7케이스에서 C# 전사 구현과 일치(점 비트맵 자체는 GDI+ 리소스라 제외).
- 아이템 분류명: `GetItemCategoryName`을 실제 C# 호출과 6값 비교.
- ItemTooltip 공통 속성: `GetItemPropString` 11종과 상단 속성 순서/특례를
  옮기고, 실제 아이템 속성 303케이스에서 실제 C# 메서드 호출과 완전 일치.
  `DrawV6SkillDotline`은 리소스 비트맵/GDI+ 브러시 의존이라 미이식.
- ItemTooltip `#t` 링크: 원본 정규식의 9자리 제한/닫는 `#` 조건을 옮기고 실제
  아이템 문자열 40개 + 경계 4개를 실제 .NET Regex/StringLinker 결과와 비교.
  기존 포팅이 10자리 id에서 어긋나던 버그도 수정.
- ItemTooltip 세로 배치: `RenderItem`의 `picH`/`splitterH` 누적을 주입 높이로
  옮기고, 실제 Consume 18,376개에서 뽑은 320개로 C# 하네스와 `pic_h/icon_y/
  splitter_h` 완전 일치.
- ItemTooltip 텍스트 레이아웃: `TextRenderer.DrawPlainText` 코어를 옮기고,
  C# 하네스가 원본 `TextRenderer<string>`를 상속해 12케이스의 줄 사각형/최종
  y를 비교했다. 픽셀 비교는 `Graphics.MeasureString`(GDI+) 의존이라 여전히
  불가하고, 색상 태그 파서는 미포팅이다.
- ItemTooltip `#cosmetic_EULO#`: `desc`에 실제로 **1,133건** 존재한다
  (이전 조사는 `desc_leftalign`만 봐서 0건으로 오판). `Gear.GetGearType`/
  `IsSubWeapon`/`GetGender`/`GetCosmeticGender`, 조사(으로/로) 판정,
  `AvatarCanvas.HairColor` 8색 접두어 제거를 옮겨 1,133행이 C#과 일치.
- `SkillTooltipRender2` 헤더 텍스트: 이름, `[마스터 레벨 : N]`/`[최대 레벨 : N]`
  분기(`PreBBSkill`=level 자식 존재, `hyperStat`), `reqLev`/`req` 요구 스킬을
  `Skill` 7,055개에서 C#과 일치. 이 클라이언트는 `common` 0·`req` 0·`level` 3이라
  7,052개가 `[마스터 레벨 : 0]`, 3개는 헤더 없음(PreBBSkill)이다.
- DB2 Map 3분할: 17,469행 일치. `info/link` 2차 패스로만 미니맵이 채워지는
  행이 **3,309개**(자체 캔버스 없이 링크가 있는 4,197행 중)다.
- DB2 Mob 3분할: 3,487행 일치(이 클라이언트는 `info/link` 0개, `info` 노드도
  227개만 존재).
- `LuaConsole` 샌드박스: `LuaSandbox.InitLuaEnv`의 결정적 부분을
  `wz-lua`(mlua 5.4)로 옮겼다. `package.path` 5조합이 원본 로직과 일치하고,
  `env` 주입·`each()`·`require`·반환값/오류 전파를 실제 Lua 스크립트 9종으로
  검증했다. 추가로 **NLua가 macOS arm64에서 동작함을 확인**해, 원본
  프렐류드(`luanet.import_type`/`get_method_bysig`)를 그대로 실행하고
  `each()`가 CLR `IEnumerable`을 순회하는 계약(4개 컬렉션 + 테이블/nil)을
  대조했다. 이어서 `RegisterLuaDelegateType`도 NLua 1.7.3으로 실제 검증해,
  핸들러 시그니처가 델리게이트와 **정확히 일치해야** 바인딩됨을 확인했다
  (원본 1-파라미터 `LuaGlobalFindNodeFunctionHandler`는 바인딩 실패, out-param
  델리게이트는 프로세스 abort). Rust는 `host_call`/`call_first_return`로 같은
  계약을 재현했다. WinForms 콘솔 창/에디터와 문법 강조는 여전히 호스트 의존이다.
- `Common/Controls` 순수 로직: 타임라인 누적, 구간 진행률, GIF 지연 분해,
  구간 최대 길이/재생 판정, 시간 오프셋 클램프, 스케일 클램프가 C# 참조
  구현과 완전 일치(7 타임라인, 30 프로브, 90 GIF, 6 구간, 4 최대길이,
  5 오프셋, 5 리셋, 8 스케일 케이스).
- 오프스크린 wgpu: CPU 스프라이트를 실제 GPU 텍스처로 업로드해 nearest/linear
  샘플링과 straight-alpha 블렌드를 수행하고 readback SHA-256을 고정. GPU
  블렌드가 premultiplied로 기록됨(`(0,0,255,128)→(0,0,128,128)`)을 확인.
- 프레임 베이킹: `ApplyFlip`/`ApplyRotation`을 CPU 역방향 최근접 샘플링으로
  옮기고 실제 Character 캔버스 6개 × 54케이스의 크기/origin/픽셀 SHA-256이
  C# 참조 구현과 일치.
- CPU 스프라이트 샘플링/합성: 실제 Character 캔버스 6개의 픽셀에 AtlasRect
  추출·flip·알파 스케일·straight-alpha/opaque 블렌드를 적용해 36케이스
  SHA-256이 C#과 완전 일치.
- 스프라이트 호출 기하: `AnimationGraphics.Draw`/`ApplyFlip`/`ApplyRotation`의
  목적 사각형·origin·회전 경계·블렌드 상태를 실제 Character 캔버스 120개로
  C#과 완전 일치(위치 5×배율 4, flip 4조합, 회전 6각도, 블렌드 4종).
- GDI 비의존 드로잉 계산: `DrawGearDetailNumber` 전진(숫자 `w+1`, `-` y+3,
  `%` x+1/`w+2`), `AnimationGraphics.Draw` 목적 사각형, `DrawPath` 1px 사각형,
  `GetBorderPath`/`GetIconBorderPath`가 C# 31케이스와 완전 일치.
- `Common/Controls` 입력 계산: 히트테스트 사각형(은행가 반올림),
  `Rectangle.Contains` 경계, 히트 순서, 드래그 델타, 드래그 저장 박스,
  휠 줌(정수 나눗셈·100% 횡단·클램프)이 C# 115케이스와 완전 일치.
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
- Common Controls의 그래픽 장치/스프라이트/마우스 드래그(GDI·SharpDX),
  LuaConsole의 NLua CLR 브리지·콘솔 UI.
- PluginBase의 메뉴 주입(`OnLoad` → WinForms RibbonBar).
- Network TCP/재접속/로그인 상태 머신(서버 필요).
- OpenAPI 성공(200) 응답 호출(Nexon API 키 필요). 오류 경로·요청/파싱 계층은 실 서버/목 서버로 검증됨.
- WorldMap 실제 QuestLimit/Fog 데이터는 클라이언트에 없음.
- weather는 원본 C# 로더/에셋에 없음.

## DB2 전체 화면 이미지 뷰어

- GUI Catalog의 `선택 이미지 DB2 전체 화면 뷰어` 버튼이 선택 이미지에서 가장 큰
  Canvas를 디코딩해 별도 창에 표시한다. zoom 슬라이더(0.1..=8.0), 100% 버튼,
  양방향 스크롤을 지원한다.
- `crates/wz-gui/src/panels/catalog.rs`의 `egui::Window` + `ScrollArea` 구현.
