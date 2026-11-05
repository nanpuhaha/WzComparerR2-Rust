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
| Skill description summary | `wz-db/src/skill_summary.rs` | ✅ | C# `SummaryParser.GetSkillSummary` 20629행과 완전 일치 |
| ItemTooltip 속성 태그/이름/설명 | `wz-db/src/item_tooltip.rs` | ✅ | C# `GetItemAttributeString` + StringLinker `02000012` 완전 일치 |
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
| `Character/Shoes/Shoes_000.wz` DB2 Character/Shoes grid | 1703 rows | Rust `wz db2-characters` 1703행 동일 컬럼 완전 일치 |
| `Character/Accessory/Accessory_000.wz` DB2 Character/Accessory grid | 2369 rows | Rust `wz db2-characters` 2369행 동일 컬럼 완전 일치 |
| `Character/Longcoat/Longcoat_000.wz` DB2 Character/Longcoat grid | 2493 rows | Rust `wz db2-characters` 2493행 동일 컬럼 완전 일치 |
| `Character/Glove/Glove_000.wz` DB2 Character/Glove grid | 661 rows | Rust `wz db2-characters` 661행 동일 컬럼 완전 일치 |
| `Character/Pants/Pants_000.wz` DB2 Character/Pants grid | 579 rows | Rust `wz db2-characters` 579행 동일 컬럼 완전 일치 |
| `Character/Shield/Shield_000.wz` DB2 Character/Shield grid | 142 rows | Rust `wz db2-characters` 142행 동일 컬럼 완전 일치 |
| `Character/Cape/Cape_000.wz` DB2 Character/Cape grid | 1139 rows | Rust `wz db2-characters` 1139행 동일 컬럼 완전 일치 |
| `Character/PetEquip/PetEquip_000.wz` DB2 Character/PetEquip grid | 511 rows | Rust `wz db2-characters` 511행 동일 컬럼 완전 일치 |
| `Character/Weapon/Weapon_000.wz` DB2 Character/Weapon grid | 5655 rows | Rust `wz db2-characters` 5655행 동일 컬럼 완전 일치 |
| `Character/Hair/Hair_000.wz` DB2 Character/Hair grid | 9004 rows | Rust `wz db2-characters` 9004행 `default/hairOverHead` 포함 완전 일치 |
| `Character/Face/Face_000.wz` DB2 Character/Face grid | 7424 rows | Rust `wz db2-characters` 7424행 `default/face` 포함 완전 일치 |
| `Character/Mechanic/Mechanic_000.wz` DB2 Character/Mechanic grid | 23 rows | Rust `wz db2-characters` 23행 동일 컬럼 완전 일치 |
| `Character/Dragon/Dragon_000.wz` DB2 Character/Dragon grid | 16 rows | Rust `wz db2-characters` 16행 동일 컬럼 완전 일치 |
| `Character/Ring/Ring_000.wz` DB2 Character/Ring grid | 1127 rows | Rust `wz db2-characters` 1127행 동일 컬럼 완전 일치 |
| `String/Skill.img` skill desc + h | 20629 rows | Rust `wz skill-summary` 20629행 SummaryParser 출력 완전 일치 |
| `Consume_000.wz/0200.img/02000012` ItemTooltip | 1 item | Rust `wz item-tooltip` name/desc/auto_desc/attributes 완전 일치 |
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
  mode matches 682/682 rows for `Coat_000.wz`, 3166/3166 for `Cap_000.wz`,
  1703/1703 for `Shoes_000.wz`, 2369/2369 for `Accessory_000.wz`, and
  2493/2493 for `Longcoat_000.wz`, 661/661 for `Glove_000.wz`, 579/579 for
  `Pants_000.wz`, 142/142 for `Shield_000.wz`, 1139/1139 for
  `Cape_000.wz`, 511/511 for `PetEquip_000.wz`, 5655/5655 for
  `Weapon_000.wz`, 9004/9004 for `Hair_000.wz`, 7424/7424 for `Face_000.wz`,
  23/23 for `Mechanic_000.wz`, 16/16 for `Dragon_000.wz`, and 1127/1127 for
  `Ring_000.wz`. Afterimage is 0/0 in both (image names not `0`-prefixed).
- GUI `Catalog` panel loads the selected image into a filterable DB2 grid.
- C# WzLib harness `db2` mode prints the same rows. Real test:
  `Weapon_000.wz/01302000.img` → C# 270 == Rust 270 rows, exact match.

## 추가 Character 카테고리 및 이미지 뷰어

| 입력 | 결과 | 검증 |
|---|---|---|
| `Character/Android/Android_000.wz` | 215 rows | C# `characters`와 완전 일치 |
| `Character/ArcaneForce/ArcaneForce_000.wz` | 7 rows | 완전 일치 |
| `Character/AuthenticForce/AuthenticForce_000.wz` | 8 rows | 완전 일치 |
| `Character/Bits/Bits_000.wz` | 79 rows | 완전 일치 |
| `Character/EquipBag/EquipBag_000.wz` | 2 rows | 완전 일치 |

DB2 전체 화면 이미지 뷰어는 GUI Catalog에서 선택 이미지의 가장 큰 Canvas를
디코딩해 확대/스크롤 창으로 표시한다.

## DB2 Item 탭 (디렉터리 단위)

원본 `DB2Form.LoadItem` 의미 그대로 구현했다. `Iter.Text.IDString()`은
`int.Parse(...).ToString()`이라 앞뒤 공백이 허용되고, 문자열 노드가 **없을
때만** `"  "`를 쓴다(노드가 있고 값이 비면 빈 문자열 유지). Special은
`Etc/CashPackage.img` 목록으로 거르고 아이콘은 `icon`, Pet은 이미지 자체가
펫이며 아이콘은 `info/iconD`.

| 탭 | 행 수 | C# 비교 |
|---|---:|---|
| Cash | 3539 | 완전 일치 |
| Consume | 18377 | 완전 일치 (`02439339 ` 뒤공백 ID 포함) |
| Etc | 5238 | 완전 일치 |
| Install | 3851 | 완전 일치 |
| Pet | 614 | 완전 일치 |
| Special | 1224 | 완전 일치 |
| 합계 | **32843** | 6/6 탭 완전 일치 |

회귀 테스트는 `crates/wz-cli/tests/fixtures/db2_item_tabs.expected.json`의
행 수 + SHA-256으로 32,843행 전체를 고정한다
(`commands::db2::item_tab_tests::db2_item_tabs_match_csharp`).

## 데이터 부재 전수 스캔

`wz parity-scan Data` 실행 결과(2026-11-06, 실제 클라이언트 데이터):

```
Skill: 7055 skill nodes, 0 common nodes, 3 level nodes
WorldMap: 168 images
WorldMap node QuestLimit/Fog: 0
Familiar files: 0
BIN files: 0
weather: 원본 C# 소스에 weather 구현 없음
```

- Skill `common`이 0이라 `SummaryParser`의 수치 치환 경로는 검증할 데이터가
  없다(resolver 실패 경로 20,629행은 일치).
- WorldMap `QuestLimit`/`Fog` 노드가 없다.
- Familiar(`Character/Familiar`, `Etc/FamiliarInfo.img`)와 `.BIN` 파일이 없다.
- weather는 원본 `WzComparerR2`/`Common`/`WzLib` 소스에 등장하지 않는다.

## Gear 스탯 문자열 (`ItemStringHelper.GetGearPropString`)

`wz-avatar::gear_prop_line`을 원본 switch(signFlag=0) 그대로 옮기고,
실제 Character WZ의 `info` 정수/문자열 스탯을 전수 비교했다. C#은
`reqLevel`/`tuc`/`incACC`/`incEVA`/`incMDD`를 주석 처리해 null을 돌려주므로
Rust도 동일하게 null이며, 툴팁 표시용은 `tooltip_stat_line`이 담당한다.

| 카테고리 | 고유 (key,value) 쌍 | 결과 |
|---|---:|---|
| Coat | 228 | 완전 일치 |
| Weapon | 1262 | 완전 일치 |
| Cap | 965 | 완전 일치 |
| Accessory | 1576 | 완전 일치 |
| Face | 4 | 완전 일치 |
| Ring | 978 | 완전 일치 |
| TamingMob | 89 | 완전 일치 |
| 합계 | **5102** | 7/7 완전 일치 |

`wz gear-props <character.wz> --json`으로 출력하며, 회귀 테스트는
`crates/wz-cli/tests/fixtures/gear_props.expected.json`의 쌍 수 + SHA-256으로
고정한다 (`commands::db2::gear_props_tests::gear_props_match_csharp`).

## OpenAPI 오프라인 helpers (`NexonOpenAPI.Utils`)

Nexon API 키/HTTP 없이 검증 가능한 부분을 `crates/wz-openapi`로 옮겼다.

| 함수 | 설명 | C# 비교 |
|---|---|---|
| `decrypt` | `'A'` 오프셋 니블 문자열 → AES-128-CBC(패딩 없음) 복호화 | 16/32/64바이트 벡터 hex 완전 일치 |
| `check_ver` | `pack[len - len/16 - 1]` | 동일 |
| `get_item_id` | `CharTable` 8행 매핑, 8자 초과는 예외 → `None` | 동일 (예외 케이스 포함) |

fixture: `crates/wz-openapi/tests/openapi_vectors.json`,
테스트: `crates/wz-openapi/tests/csharp_parity.rs`.
C# 하네스: `tools/csharp-worldmap ... -- openapi <text>`.

HTTP 기반 캐릭터/아바타 조회는 API 키가 필요해 대상에서 제외한다.

## ItemTooltip 6탭 전수 비교

`wz db2-item-tab-tooltips <Data> <dir>`로 Item 6탭 모든 아이템의
name/desc/attributes를 만들고, C# `itemtooltips` 모드(같은 LoadItem 순회 +
`GetItemAttributeString`)와 비교했다.

| 탭 | 행 수 | 결과 |
|---|---:|---|
| Cash | 3539 | 완전 일치 |
| Consume | 18377 | 완전 일치 |
| Etc | 5238 | 완전 일치 |
| Install | 3851 | 완전 일치 |
| Pet | 614 | 완전 일치 |
| Special | 1224 | 완전 일치 |
| 합계 | **32843** | 6/6 완전 일치 |

`sharableOnce`는 `"월드 내 나의 캐릭터 간 1회 이동 가능\n(이동 후 교환불가)"`,
`accountSharableAfterExchange`는
`"1회 교환 가능\n(거래 후 월드 내 나의 캐릭터 간 이동만 가능)"`로 개행 위치까지
원본과 같다. 회귀 테스트는
`crates/wz-cli/tests/fixtures/db2_item_tab_tooltips.expected.json`의 행 수 +
SHA-256으로 고정한다 (`db2_item_tab_tooltips_match_csharp`).

## Network 오프라인 (`wz-network`)

| 요소 | 원본 | 검증 |
|---|---|---|
| RC4 스트림 | `RC4CryptoTransform` | C# 실행 결과와 12개 (key 길이 8/16/32 × 데이터 1/8/17/64B) 벡터 완전 일치 |
| 프레임 | `WcClient.EncodePack` (uint16 LE + UTF-8 JSON) | 인코딩/부분 수신/초과 길이 자체 테스트 |

fixture: `crates/wz-network/tests/rc4_vectors.json`,
테스트: `crates/wz-network/tests/csharp_parity.rs`,
C# 하네스: `tools/csharp-worldmap ... -- networkrc4 <Network.dll> <keyhex> <datahex>`.

TCP 연결, 재접속, 로그인 상태 머신은 서버가 필요해 제외한다.

## CharaSimControl 텍스트 헬퍼 (`ItemStringHelper`)

GDI 없이 호출 가능한 순수 텍스트 함수를 포팅하고 C# 실행 결과와 비교했다.

| 함수 | 입력 범위 | 결과 |
|---|---|---|
| `GetJobName` | 0..=60000 전 코드 | 417개 매핑 완전 일치 |
| `GetFifthJobName` | skill 9종 × jobId 22종 = 198 | 완전 일치 |
| `GetItemCoreSpecString` | 17종 × value 4종 × desc 6종 = 408 | 완전 일치 (받침 판정 포함) |

fixture: `crates/wz-avatar/tests/job_names.json`,
`crates/wz-avatar/tests/fifth_job_names.json`,
`crates/wz-db/tests/core_specs.json`.
C# 하네스: `jobnames 60000`, `fifthjobnames`, `corespec`.

## AchievementTooltip (`AchievementTooltipRenderer`)

`wz-db::achievement`가 `Achievement.CreateFromNode`와 렌더러의 텍스트 라인
순서를 옮겼다.

- 이름/설명, score, difficulty(`difficulty` 노드가 있을 때만, 기본 `normal`),
  uiForm(`uiType`이 있을 때만, 기본 `basic`), block(기본 `none`)
- 기간: `yyyyMMddHHmmss` → `yyyy년 MM월 dd일 HH시 mm분` 두 개를 ` ~ `로 연결
- 미션(`uiForm`이 `mission`/`all`일 때), `카테고리: 대/소분류`,
  `선행 업적: 이름(00037)`, `업적창에 표시되지 않음`

C# `Achievement.CreateFromNode` + 렌더러 리터럴과 비교:
`AchievementData_000.wz` **1,579개** 전부 일치 (13개 필드 + lines).
fixture: `crates/wz-cli/tests/fixtures/achievements.expected.json` (행 수 +
SHA-256), 테스트: `achievements_match_csharp`.
`wz achievements <wz> [--json]`로 출력한다.

## WorldArchiveTooltip (`WorldArchiveTooltipRender`)

`wz-db::world_archive`가 렌더러의 텍스트 라인 구성을 옮겼다.

- `SplitLine` 구분자 6종(`\r\n`, `\\r\\n`, `\\r`, `\\n`, `\r`, `\n`)
- 몹: 월드 아카이브 설명 → `[몬스터북]` → 도감 설명
- NPC: 월드 아카이브 설명, 대사만 있으면 `(정보 없음)`

C# `StringLinker.StringWorldArchiveMob/Npc` + `StringMonsterBook`을 사용한
하네스와 비교: `String_000.wz` + `Etc_002.wz` **1,845개** 항목 완전 일치.
fixture: `crates/wz-cli/tests/fixtures/world_archive.expected.json`,
테스트: `world_archive_matches_csharp`, CLI: `wz world-archive`.
NPC 대사(`NpcQuote`) 섹션은 대화/버블 노드 탐색이 필요해 제외한다.
