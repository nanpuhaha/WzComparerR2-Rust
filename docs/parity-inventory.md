# C# → Rust 기능 인벤토리

상태: ✅ 구현+실데이터/합성 테스트, 🟡 부분/데이터 없음, ⬜ 미착수.

## 1. WzLib / 데이터 계층

| C# | Rust | 상태 | 검증 |
|---|---|---|---|
| `Wz_Image.cs` 노드/값/Canvas/Sound/RawData/Video | `crates/wz-core/src/image.rs` | ✅ | image 단위 테스트, oracle tree |
| `Wz_Crypto.cs` BMS/KMS/GMS AES, PKG2 키 | `crates/wz-core/src/crypto/*` | ✅ | crypto 단위 테스트 |
| `Wz_Sound.cs` / `TryDecryptWaveFormatEx` | `crates/wz-core/src/sound.rs` | ✅ | `sound::tests::*`, 실제 BGM 추출 |
| `WzPatcher.cs` / `CheckSum.cs` | `crates/wz-core/src/patch.rs` | ✅ | 실패치 43개 SHA-256, 수정 길이 42,701,118,706 |
| `StringLinker.cs` | `crates/wz-db/src/linker.rs` | ✅ | `string_linker_real_wz`, `map_display_names_real_wz` |

## 2. Avatar / CharaSim

| C# | Rust | 상태 | 검증 |
|---|---|---|---|
| `AvatarCanvas.cs` body/equipment/action/delay | `crates/wz-avatar/src/loader.rs` | ✅ | 84 avatar tests |
| Android `Etc/Android` basic/costume | `loader.rs` | ✅ | `android_item_assembles_avatar_from_etc_android` |
| Ring `Effect/ItemEff.img` | `loader.rs` | ✅ | `ring_effect_loads_from_item_eff` |
| CharaSim `Gear`/`SetItem`/`Potential`/`GetMaxStar` | `crates/wz-avatar/src/stats.rs` | ✅ | 3 stats tests |
| 비트맵 툴팁 | `render_tooltip_bitmap` | ✅ | `tooltip_bitmap_renders_title_and_lines` |
| CharaSim gear tooltip | `wz-avatar/src/gear_tooltip.rs` | ✅ | C# `gear`/`set`/`potential` 출력과 라인 일치 + 비트맵 |
| Ring/Android 일부 UI 값 | GUI `avatar_panel` | 🟡 | 일부 입력/표시만 |

## 3. MapRender 데이터

| C# | Rust | 상태 | 검증 |
|---|---|---|---|
| `MapData.LoadInfo` vrect/bgm/mapMark/town/return | `crates/wz-db/src/map.rs` | ✅ | `test_map_scene_parsing_and_footholds` |
| Foothold/LadderRope/Portal/Life | `map.rs` | ✅ | 동일 + oracle map-scene |
| `BackItem` + `GetMeshBack` | `crates/wz-gui/src/background.rs` | ✅ | 4 background tests, 실제 000010000 |
| `ObjItem`/`TileItem` | `map.rs` + GUI loader | ✅ | GUI smoke, oracle |
| `MapLight`/`Light2D` | `map.rs` + `wz-render::SpotLight` | ✅ | spot mesh tests, 실맵 파싱 |
| `ParticleItem`/`ParticleSystem` | `crates/wz-db/src/particle.rs` + `wz-gui/src/particles.rs` | ✅ | 실제 000110000 13개, Fog desc |
| `MapEvent` effect | `map.rs` | ✅ | 실제 100000000 effect 01 |
| `ReactorItem` stage/hit | `crates/wz-gui/src/reactor.rs` | ✅ | 1002009/100000 실데이터 |
| `LifeItem` + `BehaviorController` 이동/전투/비행 | `crates/wz-gui/src/life.rs` | ✅ | 순찰/hit/die/attack/chase/fly, 실 mob 9833307 attack/fly |
| `SkyWhaleItem` | `map.rs` + GUI overlay | ✅ | 합성 파서 테스트 |
| `IlluminantClusterItem` 시작/끝 스프라이트 | `wz-gui/src/illuminant.rs` | ✅ | sellas cluster 0/1 실데이터 |
| `ToolTip` 맵 오브젝트 | `map.rs` + `String/ToolTipHelp.img` | ✅ | 실제 000050000 |
| MiniMap canvas/icons/names | `map.rs` + GUI panel | ✅ | 2313x1592, 아이콘 4종, String 이름 |
| `WorldMap` BaseImg/MapList | `wz-gui/src/worldmap.rs` | ✅ | 실제 WorldMap000 7 spots |
| WorldMap `QuestLimit`/`Fog` | `worldmap.rs` + GUI | ✅ | WZ-node JSON fixture + expected UIWorldMap JSON/selector labels + `real_worldmap_has_no_quest_or_fog_nodes` (85+ 이미지 0개) |

## 4. MapRender 렌더링

| C# | Rust | 상태 | 검증 |
|---|---|---|---|
| `MsSpriteRenderer` / `SpriteBatch` | `crates/wz-render/src/batch.rs` | ✅ | batch tests |
| `LightRenderer.DrawSpotLight` | `wz-render/src/light.rs` | ✅ | 36분할 메시 테스트 |
| `waterBack`/`waterFront` 파이프라인 | `wz-render/src/device.rs`, `water.rs` | ✅ | 실제 993258900 텍스처 + 픽셀 리드백 |
| waterBack→scene→waterFront 합성 | `wz-gui/src/compose.rs` | ✅ | `back_is_behind_scene...`, 993258900 픽셀 회귀 |
| waterFront bg_tex 장면 캡처 | `render_bridge.rs` | ✅ | 현재 scene RGBA를 bg_tex로 바인딩 |
| water 셰이더 상수 | `parse_water_back_params`/`front` | ✅ | min/max 4500/19500, aberration 0.005, godray 4754/5248 |
| Camera pan/zoom/키보드 관성 | `wz-render/src/camera.rs`, `camera_input.rs` | ✅ | camera_input 2 tests |
| Minimap UI | `panels/inspector.rs` | ✅ | GUI tests |
| WorldMap UI | `panels/inspector.rs` | ✅ | 실 WorldMap000 |
| Skill summon preview | `wz-gui/src/skill.rs` | ✅ | 1001005 effect 11프레임 |
| Weather | — | 🟡 | 클라이언트/원본 MapData에 weather 노드·자산 없음 |

## 5. Patcher / Sound / CLI

| C# | Rust | 상태 | 검증 |
|---|---|---|---|
| `WzPatcher` apply create/modify/delete | `patch.rs`, `wz patch` | ✅ | 실제 00419to00420.patch |
| `Wz_Sound.ExtractSound` | `wz sound` | ✅ | Bgm00 1,261,192B MP3, afplay exit 0 |
| GUI 사운드 재생 | `wz-gui/src/sound_player.rs` | ✅ | 실제 BGM, UOL 리액터 사운드 |
| oracle diff/tree/image/map-scene | `wz-cli/src/commands/oracle` | ✅ | JSON 스키마/실데이터 |

## 6. 남은 항목

- ✅ Life 공격/추적/비행: attack/chase/fly 상태, 실 mob `9833307`
  (fly/attack1) 로드와 합성 상태 테스트.
- ✅ WorldMap QuestLimit/Fog: `tests/fixtures/worldmap_quest_fog.json`을
  실제 WZ 노드 구조로 파싱하고 expected JSON(QuestLimit open/close/quest/
  state/default, Fog quest/qState, MapList mapNo)과 GUI selector 라벨을 비교.
  실제 `WorldMap_000.wz` 85개 이미지에는 QuestLimit/Fog 0개임을
  `real_worldmap_has_no_quest_or_fog_nodes` 로 고정.
- ✅/N/A Weather: 원본 MapRender 코드에 weather 로더/셰이더가 없고,
  클라이언트 맵/WZ에도 weather 노드·자산이 없음 (문서화된 미지원 데이터).
- 🟡 정확한 water 셰이더 색수차/디스토션은 GPU 셰이더로 실행되지만,
  텍스처 바인딩은 실제 3장을 사용하고 배경 캡처는 단순화되어 있음.

## 검증 명령

```
WZ_DATA_ROOT="$PWD/Data" cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo deny check
WZ_DATA_ROOT="$PWD/Data" cargo test --manifest-path crates/wz-gui/Cargo.toml --no-default-features
cargo run -q --manifest-path crates/wz-gui/Cargo.toml --no-default-features -- --smoke Data/Map/Map/Map0/Map0_000.wz
```

## C# 비교 검증 (추가)

| 검증 | fixture/테스트 | 비교 대상 |
|---|---|---|
| MapData 기본/배경/포털/라이프/미니맵 | `crates/wz-db/tests/fixtures/map_scene_000010000.expected.json` + `map_scene_matches_csharp_expected_fixture` | C# `MapData.Load*` 필드/의미 |
| WorldMap Quest/Fog | `crates/wz-gui/tests/fixtures/worldmap_quest_fog.*.json` + `worldmap_quest_fog_fixture_matches_uiworldmap_behavior` | C# `UIWorldMap.LoadQuestLimit/LoadFog` |
| Water 상수/픽셀 | `loads_real_water_back_front_layers`, `water_shaders_produce_pixels` | C# `waterBack/waterFront` WZ 상수 + 렌더 출력 |
| Life 상태 | `attack_state_runs_and_recovers`, `chase_moves_toward_target`, `fly_state_moves_vertically`, `loads_real_ambient_mob_attack_and_fly` | C# `BehaviorController` 상태 |

WorldMap 실제 클라이언트 데이터는 QuestLimit/Fog 노드가 없어, 실제 WZ 파일 대신
동일 노드 구조의 fixture로 파서/UI 출력을 C# 동작과 비교한다.

## C# 실행 비교 (WzLib harness)

`tools/csharp-worldmap`는 원본 `WzComparerR2.WzLib.dll`을 참조해 실제 C# 파서를
실행한다.

| 입력 | C# 실행 결과 | Rust 결과 |
|---|---|---|
| `worldmap_quest_fog.json` fixture | expected JSON과 MATCH | 동일 expected JSON 테스트 통과 |
| `Data/Map/WorldMap/WorldMap_000.wz` | `{"images":85,"quest":0,"fog":0}` | `real_worldmap_has_no_quest_or_fog_nodes` 통과 (QuestLimit/Fog 0) |
| `Map0_000.wz / 000010000.img` | 4721 nodes | oracle `node_count=4722` = 4721 + outer Directory wrapper; per-node path diff 일치 |
| `Weapon_000.wz / 01302000.img` gear tooltip | `["착용 레벨 : 0","공격력 : +15","업그레이드 가능 횟수 : 8"]` | Rust `gear_tooltip` 동일 + 비트맵 |
| `Weapon_000.wz / 01302000.img` DB2 | 270 rows | Rust `wz db2` 270 rows path/type/value/children 완전 일치 |
| `_Canvas_000.wz / 01302000.img` icons | 18 canvases, RGBA SHA-256 | Rust `wz db2-icons` 18개 SHA-256 완전 일치 |
| `Item/Consume/Consume_000.wz / 0200.img` DB2 Item grid | 530 rows | Rust `wz db2-items` 530행 id/name/desc/has_icon 완전 일치 |
| `Map/Map/Map0/Map0_000.wz` DB2 Map grid | 87 rows | Rust `wz db2-maps` 87행 id/street/map/minimap/link 완전 일치 |
| `Mob/_Canvas/_Canvas_011.wz` DB2 Mob grid | 55 rows | Rust `wz db2-mobs` 55행 id/name/stand/fly/move/hit/die/thumbnail/link 완전 일치 |
| `Skill/_Canvas/_Canvas_000.wz` DB2 Skill grid | 165 rows | Rust `wz db2-skills` 165행 id/type/book/name/desc/icon/common/h 완전 일치 |
| `Npc/_Canvas/_Canvas_000.wz` DB2 NPC grid | 943 rows | Rust `wz db2-npcs` 943행 id/name/stand/default/link 완전 일치 |
| `Reactor/_Canvas/_Canvas_000.wz` DB2 Reactor grid | 441 rows | Rust `wz db2-reactors` 441행 id/icon/info/link 완전 일치 |
| `Sound/Sound_000.wz` DB2 Music grid | 40 rows | Rust `wz db2-music` 40행 path/bytes/ms 완전 일치 |
| `Item/Consume/Consume_000.wz` DB2 DamageSkin grid | 952 rows | Rust `wz db2-damage-skins` 952행 id/name/desc/icon/sample 완전 일치 |
| `Consume_000.wz` + `Morph_000.wz` DB2 Morph grid | 234 rows | Rust `wz db2-morphs` 234행 id/name/desc/morph/icon/walk 완전 일치 |
| `Character/TamingMob/TamingMob_000.wz` DB2 TamingMob grid | 1265 rows | Rust `wz db2-taming-mobs` 1265행 id/name/desc/h1/icon/reqLevel/reqJob/tuc/islot/tamingMob 완전 일치 |
| `Character/Coat/Coat_000.wz` DB2 Character grid | 682 rows | Rust `wz db2-characters` 682행 id/name/desc/h1/icon/reqLevel/reqJob/tuc/islot/vslot/stat 완전 일치 |
| `Character/Cap/Cap_000.wz` DB2 Character/Cap grid | 3166 rows | Rust `wz db2-characters` 3166행 동일 컬럼 완전 일치 |
| `Etc_000.wz` set -1 | `{"name":"결속의 반지","complete":5,"effects":[...]}` | Rust set effects 동일 |
| `Item_000.wz` potential 1/1 | `["STR : +1"]` | Rust potential lines 동일 |

실행 명령:

```
cd tools/csharp-worldmap
dotnet run -c Release -p:WzLibPath=<WzComparerR2.WzLib.dll> -- fixture ../../crates/wz-gui/tests/fixtures/worldmap_quest_fog.json
dotnet run -c Release -p:WzLibPath=<WzComparerR2.WzLib.dll> -- real ../../Data/Map/WorldMap/WorldMap_000.wz
dotnet run -c Release -p:WzLibPath=<WzComparerR2.WzLib.dll> -- mapnodes ../../Data/Map/Map/Map0/Map0_000.wz 000010000.img
```

## C# source anchors

| C# 원본 | Rust 대응 |
|---|---|
| `MapRender/MapData.cs` LoadInfo/Back/Portal/Life/Reactor/Light/Particle/MapEvents/ToolTip/SkyWhale/IlluminantCluster | `wz-db/src/map.rs`, GUI loader |
| `MapRender/LightRenderer.cs`, `MapLight.cs` | `wz-render/src/light.rs` |
| `MapRender/Patches2/BackItem.cs`, `GetMeshBack` | `wz-gui/src/background.rs` |
| `MapRender/Patches2/LifeItem.cs`, `BehaviorController.cs`, `LifeInfo.cs` | `wz-gui/src/life.rs` |
| `MapRender/Patches2/ReactorItem.cs` | `wz-gui/src/reactor.rs` |
| `MapRender/Patches2/ParticleItem.cs`, `ParticleSystem.cs`, `ResourceLoader.cs` | `wz-db/src/particle.rs`, `wz-gui/src/particles.rs` |
| `MapRender/Patches2/PortalItem.cs` | `wz-gui/src/portal.rs` |
| `MapRender/Patches2/TooltipItem.cs` | `wz-gui/src/state.rs` + `String/ToolTipHelp.img` |
| `MapRender/UI/UIMiniMap.cs`, `UIWorldMap.cs` | `wz-gui/src/panels/inspector.rs`, `worldmap.rs` |
| `MapRender/Effects/ShaderMaterials.cs`, `EffectResources.cs` | `wz-render/src/water.rs`, `device.rs`, `compose.rs` |
| `Patcher/WzPatcher.cs`, `Builder/CheckSum.cs` | `wz-core/src/patch.rs` |
| `WzLib/Wz_Sound.cs`, `Wz_Image.TryDecryptWaveFormatEx` | `wz-core/src/sound.rs` |

## 전체 C# 파일 커버리지

프로젝트/디렉터리별 파일 수·LOC·상태·미구현 목록은
[`docs/csharp-coverage.md`](csharp-coverage.md)에 있다.

이번 추가:
- `wz-diff::virtual_node` — `WzVirtualNode.Combine` 병합 규칙 구현.
- C# WzLib 실행 하네스로 WorldMap fixture/실데이터/맵 노드 수 비교.
- `000010000.img` 4721 vs 4722 차이는 Rust oracle의 outer Directory wrapper
  1개이며, per-node path multiset는 wrapper 제거 후 완전히 일치.

## DB2 data viewer

- `wz-db::db2` flattens a WZ image into `path/name/node_type/value/children`
  rows; Canvas subproperties (`origin`, `_outlink`, `map/...`) are included.
- `wz db2 <wz> <image> [--filter] [--json] [--limit]` prints the table.
- `wz db2-icons <wz> <image> [--filter] [--json]` decodes every Canvas and
  prints `path/width/height/format/RGBA SHA-256`; GUI Catalog also has an icon
  gallery. C# WzLib `icons` mode matches 18/18 canvases for 01302000.img.
- `wz db2-items <item.wz> <image> --string <String_000.wz>` builds the original
  `DB2Form.LoadItem` grid (id/name/desc/has_icon) using StringLinker; C#
  `items` mode matches 530/530 rows for `Consume_000.wz/0200.img`.
- `wz db2-maps <map.wz> --string <String_000.wz>` builds `DB2Form.LoadMap`
  (id/street/map/minimap/link); C# `maps` mode matches 87/87 rows for
  `Map0_000.wz`.
- `wz db2-mobs <mob.wz> --string <String_000.wz>` builds `DB2Form.LoadMob`
  (id/name/stand/fly/move/hit/die/thumbnail/link); C# `mobs` mode matches 55/55
  rows for `_Canvas_011.wz`. 이 클라이언트의 분할 Mob 이미지에는 `info` 스탯이
  거의 없어 DB2 stat 컬럼은 비어 있다.
- `wz db2-skills <skill.wz> --string <String_000.wz>` builds `DB2Form.LoadSkill`
  (book + skill rows, icon/common/h); C# `skills` mode matches 165/165 rows for
  `_Canvas_000.wz`.
- `wz db2-npcs <npc.wz> --string <String_000.wz>` builds `DB2Form.LoadNpc`
  (id/name/stand/default/link); C# `npcs` mode matches 943/943 rows for
  `_Canvas_000.wz`.
- `wz db2-reactors <reactor.wz>` builds `DB2Form.LoadReactor`
  (id/icon/info/link); C# `reactors` mode matches 441/441 rows for
  `_Canvas_000.wz`.
- `wz db2-music <sound.wz>` builds `DB2Form.LoadMusic` (recursive
  `Bgm*/PL_*` sound paths + bytes/duration); C# `music` mode matches 40/40 rows
  for `Sound_000.wz`.
- `wz db2-damage-skins <consume.wz> --string <String_000.wz>` builds
  `DB2Form.LoadDamageSkin` (name keyword + 0243/0263 icon/sample); C#
  `damageskins` mode matches 952/952 rows for `Consume_000.wz`.
- `wz db2-morphs <consume.wz> --morph <morph.wz> --string <String_000.wz>`
  builds `DB2Form.LoadMorph` (spec/morph + Morph walk/0); C# `morphs` mode
  matches 234/234 rows.
- `wz db2-taming-mobs <taming.wz> --string <String_000.wz>` builds the
  `DB2Form.LoadCharacter` TamingMob slice (Eqp names + info fields); C#
  `tamingmobs` mode matches 1265/1265 rows.
- `wz db2-characters <character.wz> --string <String_000.wz>` builds the
  general Character equipment slice (Eqp names + info fields); C# `characters`
  mode matches 682/682 rows for `Coat_000.wz` and 3166/3166 for `Cap_000.wz`.
- GUI `Catalog` panel loads the selected image into a filterable DB2 grid.
- C# WzLib harness `db2` mode prints the same rows. Real test:
  `Weapon_000.wz/01302000.img` → C# 270 == Rust 270 rows, exact match.
