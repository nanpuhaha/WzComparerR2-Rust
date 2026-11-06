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

- NPC 대사: `NpcQuote.CreateFromNode`(n/f/w/d 0..99, `dialogue`/`dialog`,
  `bubble`), `#p/#o/#m/#t/#q/#i/#v/#y/#illu` 치환, 태그 제거, HashSet 중복 제거
  후 `\r\n` 연결 → `NPC 대사` + ` - ` 라인

C# `StringLinker.StringWorldArchiveMob/Npc` + `StringMonsterBook` +
`NpcQuote.CreateFromNode`을 사용한 하네스와 비교:
`String_000.wz` + `Etc_002.wz` **1,845개** 항목(NPC 대사 767개) 완전 일치.
fixture: `crates/wz-cli/tests/fixtures/world_archive.expected.json`,
테스트: `world_archive_matches_csharp`, CLI: `wz world-archive`.

## PluginBase `FindWz` 경로 조회

`MainForm.CharaSimLoader_WzFileFinding` + `Wz_Node.FindNodeByPath` 동작을
`wz-db::plugin_find`로 옮겼다.

- 경로를 `/` 또는 `\`로 분리, 첫 세그먼트를 `Wz_Type`으로 해석
- 등록된 타입 WZ의 루트에서 나머지 세그먼트를 순서대로 탐색
- 중간에 `.img`를 만나면 추출 후 내부로 계속 진행
- 결과: 이름/값 분류(`null`/`int:`/`string:`/`canvas`/`sound`)/자식 수

C# `Wz_Structure.WzNode.FindNodeByPath(true, ...)`와 비교:
Mob/Npc/Skill/Reactor/String **57개 경로** 완전 일치 (found/name/value/children).
fixture: `crates/wz-cli/tests/fixtures/findwz.expected.json`,
테스트: `findwz_matches_csharp`, CLI:
`wz find-wz --map Mob=<wz> ... -- <path>`.
플러그인 DLL 로딩/메뉴 주입은 WinForms 호스트가 필요해 제외한다.

## Network 패킷 계약 직렬화 (`WcClient.EncodePack`)

Newtonsoft `TypeNameHandling.Objects` + `[JsonObject("id")]` +
`byte[]`→Base64 컨버터 조합을 그대로 재현했다.

| 패킷 | `$type` | 결과 |
|---|---|---|
| `PackHeartBeat` | 0 | `{"$type":"0"}` |
| `PackLoginReq` | 3 | `WcID`, `NickName` |
| `PackLoginResp` | 4 | `SessionID` |
| `PackSendChat` | 9 | `Message`(null 포함), `Group` |
| `PackCryptReq` | 1 | Base64 `Exponent`/`Modulus` |
| `PackCryptResp` | 2 | Base64 두 키 |

7개 계약의 JSON 문자열과 `uint16 LE 길이 + UTF-8` 프레임 바이트가 C#
`EncodePack` 실행 결과와 완전 일치.
fixture: `crates/wz-network/tests/packets.json`,
테스트: `pack_csharp_parity.rs`.

## NPC/Reactor 툴팁 텍스트

- `NpcTooltipRenderer`: `StringLinker.StringNpc.Name`, `ID:n`,
  `등장위치 :`, `Etc/NpcLocation.img/<id>` 자식마다
  ` <StringMap.Name>(<원본 mapID>)`, 위치가 없으면 ` 불명`
  - 맵 이름은 렌더러와 같이 `StringResult.Name`(=`street : mapName`)을 쓴다.
- `ReactorTooltipRenderer`: `info/viewName` → `info/name` → `info/info`,
  `action`, 0부터 연속된 숫자 자식 수(`ClipCount`),
  `이름: …`/`클립: n개`/`액션: …`

C# `StringLinker` + 동일 필드 추출 하네스와 비교:
`Npc_000.wz` + `Reactor_000` **12,629개**(NPC 12,188 + Reactor 441)
완전 일치. fixture: `crates/wz-cli/tests/fixtures/chara_tooltips.expected.json`,
테스트: `chara_tooltips_match_csharp`, CLI: `wz chara-tooltips`.

## DamageSkin 모델 (`DamageSkin.CreateFromNode`)

`DamageSkinTooltipRenderer`는 텍스트 라인이 없어(ID/샘플만 그린다) 모델 필드를
비교 대상으로 삼았다.

- `desc`, `extractID`, `sample` 존재, `customType`
- `effect`/레거시 `NoRed0`,`NoRed1`,`NoCri0`,`NoCri1`,`NoCustom`
  - 각 섹션의 숫자 키 집합(직접 UOL/PNG 또는 자식 중 첫 UOL/PNG)
  - `numberSpace`, `bottomFix`(>0), `dotSpace`
- 원본 switch가 무시하는 `NoBlue*`/`NoViolet*`/`NoProduction*` 섹션은 제외

`Etc_000.wz/DamageSkin.img` **402개**가 C# 추출과 완전 일치.
fixture: `crates/wz-cli/tests/fixtures/damage_skins.expected.json`,
테스트: `damage_skin_models_match_csharp`, CLI: `wz damage-skin-model`.

## Network TCP 프레이밍/로그인 상태 머신

`WzComparerR2.Network.WcClient`의 세션 흐름을 `wz-network::session`으로 옮겼다.

- 프레임: `uint16 LE 길이 + 본문`. RC4가 켜지면 **길이까지** 암호화
  (`CryptoStream`이 길이를 포함해 감싸는 원본과 동일).
- 상태: `Connected → CryptoRequested → Ready → LoginSent → LoggedIn`
- `handle_crypt_resp`: `$type=="2"` 확인 → Base64 키 복호화(주입된
  `KeyDecryptor`) → `Rc4` 두 방향 시작
- `send_login`/`handle_login_resp`: `$type 3/4`

검증:
- `crates/wz-network/tests/loopback.rs` — 실제 TCP 루프백에서 CryptReq →
  CryptResp → RC4 LoginReq → LoginResp 전 구간
- `wz net-loopback` 실행 출력:
```
state Connected
send PackCryptReq -> CryptoRequested
recv PackCryptResp: s2c="s2c-key" c2s="c2s-key" -> Ready
send PackLoginReq -> LoginSent
recv PackLoginResp -> LoggedIn
OK: login flow complete
```
- 패킷 바이트는 앞 절의 C# `EncodePack` 7계약과 일치하므로, 루프백에서
  사용하는 코덱은 C# 검증을 공유한다.
- RSA 복호화는 `KeyDecryptor`로 주입한다(원본은 `RSACryptoServiceProvider`).
- C# `WcClient`는 WinForms/PluginBase 의존으로 macOS에서 실행할 수 없어
  동일 하네스 비교 대신 루프백 검증을 사용했다.

## OpenAPI 아바타 코드 비트팩 (`AvatarCodeStructure`/`Utils.Unpack`)

- 구조 정의는 C# `AvatarCodeStructure.Structure`를 덤프해
  `crates/wz-openapi/src/avatar_structure.json`에 넣었다(v26–36, 39–41, 43, 44).
- `Utils.Unpack`을 그대로 구현: 최상위부터 `bits`만큼 LSB-first로 읽고,
  값이 0이 아니고 하위 항목이 있으면 `ExpandOffset` 위치에 삽입.
- 없는 버전 보정은 `UnpackedAvatarData` 생성자와 동일(최소/최대 클램프).

검증: 19개 버전 × {zeros, random 2048B} = **38케이스**에서 필드 수와
`name/bits/value` 목록 해시가 C# `Utils.Unpack`과 완전 일치.
fixture: `crates/wz-openapi/tests/avatar_vectors.json`,
테스트: `avatar_csharp_parity.rs`, CLI: `wz avatar-unpack <version> <packhex>`.

## Map 툴팁 텍스트 (`MapTooltipRenderer`)

- `Map.CreateFromNode`(findNode=null 경로)와 같은 필드: `link`, `barrier`,
  `barrierArc`, `barrierAut`, `mapMark`, `bgm`, `AFKmob/flyingMob` + `life`의
  몹/ NPC ID(중복 제거)
- `GetMapName`/`GetMapDesc`: `StringMap[id]`, 없으면 `StringMap[link]`
  - street/mapName은 없으면 `(null)`, 설명은 `mapDesc`
- 몹/NPC 이름: `StringMob`/`StringNpc`, 없으면 `(null)`
- 몹 레벨 접미사는 `Mob/<id>.img/info/level`이 필요하지만 이 클라이언트에는
  `level` 노드가 0개라 생략(양쪽 동일)

`Map0_000.wz` **87개**가 C# 하네스와 완전 일치.
fixture: `crates/wz-cli/tests/fixtures/map_tooltips.expected.json`,
테스트: `map_tooltips_match_csharp`, CLI: `wz map-tooltips`.

## Quest 툴팁 모델 (`QuestTooltipRenderer`)

- `Quest.CreateFromNode`의 비트맵 제외 필드
  - `QuestInfo`: name, desc 0/1/2, blocked, demand/base, recommend
  - `Check/0`: lvmin/lvmax
  - `Check/1`: npc, mob/item(`key=id:count`), infoex(`exVariable`:cond>0)
  - `Act/1`: `QuestReward.CreateFromAct1Node`(exp/money/pop/pettameness,
    성향 6종, item(id/count/prop/job/gender), sp)
- 원본과 같이 recommend는 check0/act1 **이후** 적용해 lvmin/lvmax를 덮어쓰고
  lvmax가 있으면 `LvLimit`을 세운다.

`QuestData_000.wz` **18,964개**가 C# 하네스와 완전 일치.
fixture: `crates/wz-cli/tests/fixtures/quest_tooltips.expected.json`,
테스트: `quest_tooltips_match_csharp`, CLI: `wz quest-tooltips`.
렌더러의 태그 치환/줄바꿈/GDI 레이아웃은 범위 밖이다.

## PluginBase 파일 탐색/메타데이터

- `GetPluginFiles`: `Plugin/` 아래 `WzComparerR2.*.dll` 재귀 수집.
  macOS의 C# `Directory.GetFiles`와 동일하게 대소문자를 구분하지 않고
  매칭한다. fixture 트리(하위 2단계 포함)에서 4개 파일이 C#과 일치.
- `PluginEntry` 메타데이터 규칙:
  `Name=AssemblyTitle`, `Author=AssemblyCompany`,
  `FileVersion=InformationalVersion(비어있지 않을 때) 또는 AssemblyFileVersion`.
- **미구현**: 어셈블리 로드 후 `PluginEntry` 서브클래스 열거/생성
  (`PluginManager.LoadPlugin`)은 .NET 메타데이터(PE/CLI) 리더가 필요하다.
  `PluginContext` 메뉴 주입도 WinForms 호스트 의존이다.

CLI: `wz plugin-files <dir>`, C# 하네스: `pluginfiles <dir>`.

## ItemTooltip 추가 텍스트

- `spec/recipeValidDay > 0` → `( 제작 가능 기간 : N일 )`
- `spec/recipeUseCount > 0` → `( 제작 가능 횟수 : N회 )`
- `info/exp_minLev`+`exp_maxLev` → `Character.ExpToNextLevel` 합계로
  `총  경험치량 :N` / `잔여 경험치량:N` (+ `cantAccountSharable/tooltip`)
- 펫 명령어: `interact` + `String/PetDialog.img` → `[사용 가능한 명령어]`,
  `Lv. n 이상 : …`, Tip 2줄

검증: `Item/Consume` 레시피 라인 **827개**가 C# `ItemTooltipRender2`
문구와 완전 일치. `exp_minLev`/펫 `interact`는 데이터 0개로 규칙만 구현.
fixture: `crates/wz-cli/tests/fixtures/item_extras.expected.json`,
테스트: `item_recipe_lines_match_csharp`, CLI: `wz item-extras <Data> <dir>`.

## Network 하트비트/재접속

- 하트비트: `$type=="0"` 수신 시 같은 패킷을 즉시 회신하고 상위로 올리지 않음
  (원본 `OnPackReceived(PackHeartBeat) => Client.Send(pack)`).
- 재접속: `ReconnectingClient`가 EOF/오류 시 `Backoff` 지연 후 재연결.
  지연은 `initial * factor^attempt`를 `max`로 상한, `max_attempts` 초과 시 중단.
  `max_attempts == 0`이면 재접속 비활성(즉시 오류).
- 검증: `crates/wz-network/tests/reconnect.rs`
  - 하트비트 에코 후 서버가 연결을 끊으면 재접속해 다음 패킷 수신
  - 백오프 100/200/300/300ms 상한
  - 재접속 비활성 시 `NotConnected`
- CLI 데모 `wz net-reconnect`:
```
connected (attempts=0)
heartbeat echoed, then reconnect #1; packet={"$type":"4","SessionID":"after-reconnect"}
OK: heartbeat echo + reconnect
```

## ItemTooltip 캐시/기간 태그

`ItemTooltipRender2`의 다음 라인을 순서/문구 그대로 옮겼다.

- `cash`일 때: `noMoveToLocker`, `onlyCash`, `cashTradeBlock`, 그 외
  `tradeBlock==0`이고 501/502/516·5157/5158이 아니면 기본 교환 안내
- `flatRate` → `기간 정액제`, `noScroll` → 펫 주문서/작명 불가,
  `noRevive` → 생명의 물 불가
- `id/1000 == 5533` → 상자 미리보기 안내(선두 개행 포함)

`Item/Cash` **2,503개**가 C# 하네스와 완전 일치.
fixture: `crates/wz-cli/tests/fixtures/item_cash_lines.expected.json`,
테스트: `item_cash_lines_match_csharp`, CLI: `wz item-cash-lines <Data> Cash`.

## ItemTooltip desc_leftalign/pointCost/cant

- `StringItem["desc_leftalign"]` → `ReplaceDescTags`
  - `#t<id>...#` → `StringItem`/`StringEqp` 이름, 없으면 숫자
  - `#cosmetic_EULO#`는 `desc_leftalign` 1,359개 중 0개라 미구현
- `info/pointCost > 0` → `· N 포인트`
- `info/cantAccountSharable/tooltip` → 문자열 그대로

검증: Cash **91** + Consume **1,151** = 1,242개가 C# 하네스와 완전 일치.
fixture: `crates/wz-cli/tests/fixtures/item_text_extras.expected.json`,
테스트: `item_text_extras_match_csharp`,
CLI: `wz item-text-extras <Data> <dir> --string <String_000.wz>`.

## ItemTooltip 성향/tradeAvailable

- 성향 순서: 카리스마/통찰력/의지/손재주/감성/매력, 값 >0만 `, 이름 N`으로
  합쳐 `장착 시 1회에 한해 …의 경험치를 얻을 수 있습니다.(일일제한, 최대치
  초과 시 제외)` 한 줄
- `tradeAvailable` 1/2 → 카르마 안내 문자열을 원본처럼 `"#c"+…+"#"`로 감쌈

검증: Consume **26** + Install **23** = 49개가 C# 하네스와 완전 일치.
fixture: `crates/wz-cli/tests/fixtures/item_incline.expected.json`,
테스트: `item_incline_lines_match_csharp`, CLI: `wz item-incline <Data> <dir>`.

## PluginBase 어셈블리 메타데이터 (`LoadPlugin` 타입 열거)

`wz-db::dotnet_metadata`가 최소 PE/CLI 리더로 다음을 읽는다.

- PE 섹션/RVA 매핑, CLI 헤더, 메타데이터 스트림(`#~`, `#Strings`, `#Blob`)
- 45개 테이블 스키마와 coded index(태그 비트 고정)로 TypeDef/TypeRef/
  NestedClass/Assembly/CustomAttribute/MemberRef 파싱
- 어셈블리 이름/버전, `AssemblyTitle/Company/InformationalVersion/FileVersion`
- public(중첩 public 포함) 타입의 이름, 기반 타입, abstract 여부
- `PluginEntry` 서브클래스(비추상) 판별

검증: fixture 어셈블리(`WzComparerR2.FixturePlugin.dll`)에서 C#
`Assembly.GetExportedTypes()` + 속성 결과와 **완전 일치**
(`Fixture.AlphaPlugin`, `Fixture.DerivedPlugin`, abstract/internal 제외).
fixture: `crates/wz-db/tests/fixtures/plugin_*.json|dll`,
테스트: `plugin_metadata_parity.rs`, CLI: `wz plugin-metadata <dll>`.
인스턴스화/메뉴 주입은 WinForms 의존으로 미구현.

## ItemTooltip 레시피 사용 제한조건

- `spec/recipe`(단일 값/자식 목록)가 있을 때
  - `reqSkill` 없으면 `recipe/10000*10000`
  - `reqSkillLevel` 없으면 1
  - `StringSkill` 이름, `장비제작`/`장신구제작`은 띄어쓰기 보정
  - `< 사용 제한조건 >`, `· {이름} {레벨}레벨 이상`

`Item/Consume` **1,039개**가 C# 하네스와 완전 일치.
fixture: `crates/wz-cli/tests/fixtures/item_recipe_req.expected.json`,
테스트: `item_recipe_requirements_match_csharp`,
CLI: `wz item-recipe-req <Data> Consume --string <String_000.wz>`.

## ItemTooltip 사용기간 (`consumableFrom`/`endUseDate`)

- `info/consumableFrom` → `yyyy년 M월 d일 HH시 mm분부터 사용가능`
- `info/endUseDate` → `yyyy년 M월 d일 HH시 mm분까지 사용가능`
- 두 값이 모두 있으면 각각 한 줄씩(선두 개행 없이 라인 단위로 비교)
- `timeLimited`/`life`는 원본이 `DateTime.Now` 기준 7일/만료일을 쓰므로
  결정적 비교 대상에서 제외

검증: Consume **418** + Cash **19** + Install **2** + Etc **10** = 449개가
C# 하네스와 완전 일치.
fixture: `crates/wz-cli/tests/fixtures/item_expire.expected.json`,
테스트: `item_expire_lines_match_csharp`,
CLI: `wz item-expire <Data> <dir>`.

데이터 주의: `consumableFrom`/`endUseDate` 값 중 **72개가 12자리 미만**
(`yyyyMMddHH` 10자리)이다. 원본 `ItemTooltipRender2`는 길이 검증 없이
`Substring(0, 4)`…를 호출하므로 이런 아이템은 원본에서도 예외가 난다.
Rust는 유효한 12자리 이상 숫자만 포맷하고, C# 하네스도 같은 가드를 두어
비교 가능한 450개 필드(449행)로 맞췄다.

## Common/Controls + Animation 순수 로직 (`wz-db::common_controls`)

원본 `FrameAnimator`/`AnimationRecoder`/`AnimationControl` 타입은
`Microsoft.Xna.Framework`/SharpDX(`Texture2D`, `GraphicsDevice`)에 묶여 있어
macOS에서 어셈블리 타입 로드가 실패한다(하네스에서 `GetTypes()` →
`ReflectionTypeLoadException`, `Could not load file or assembly
'SharpDX.Direct2D1'`). 따라서 아래는 원본 소스의 **메서드 본문을 그대로 옮긴
C# 참조 구현**을 하네스에서 컴파일해 기준값으로 썼다.

| 원본 | Rust | 규칙 |
|---|---|---|
| `FrameAnimator.CreateTimeline` | `create_timeline` | `0`으로 시작하는 지연 누적 |
| `FrameAnimator.GetProcessFromTimeline` | `get_process_from_timeline` | `Array.BinarySearch`, 미발견 시 `~index-1`과 구간 비율 |
| `AnimationRecoder.GetGifTimeLine` | `gif_timeline` | animated는 `preferred`로 분할 후 나머지를 직전 항목에 합산, 비animated는 `max`로 분할(없으면 원본 길이) |
| `AnimationRecoder.GetMaxLength` | `max_length` | 구간 끝 최댓값, 비면 0 |
| `AnimationControl.AddItem` | `add_item_times` | `(start, max(length,end)+start)` |
| `AnimationControl.GetPlayingAni` | `playing_indices` | `max==0`이면 전체, 아니면 `start<=t<end` |
| `AnimationControl.UpdateTimeOffset` | `update_time_offset` | 10ms 격자 반올림, `[0, (max-interval)/10*10]` 클램프 |
| `AnimationControl.Update` | `should_reset` | `cur > max - margin` |
| `AnimationControl.GlobalScale` | `clamp_scale` | `MathHelper.Clamp(v, 0.1, 10)` |

검증: 위 표의 경계/중복/음수 케이스 **160개**(7 + 30 + 90 + 6 + 4 + 5 + 5
+ 8 + 5)가 참조 구현과 완전 일치.
fixture: `crates/wz-db/tests/fixtures/anim_timeline.json`,
테스트: `crates/wz-db/tests/common_controls_parity.rs`,
C# 하네스: `tools/csharp-worldmap ... -- animtimeline`.

구현 불가로 남긴 부분: `GraphicsDeviceService`, `GraphicsDeviceControl`,
`SpriteBatchEx` 배치, 마우스 드래그 컨텍스트, `Timer`/`Stopwatch` 실시간 루프,
`AnimationRecoder` 렌더 타깃·GIF/PNG 인코딩, `AlphaForm`(WinForms 레이어드
윈도우). 모두 장치/윈도우 핸들 의존이다.

## DB2 Map 탭 3분할 + 링크 미니맵

원본 `LoadMap(Part)`는 `Map/Map` 아래 디렉터리를 파티션별로 고르고, 1패스에서
행을 만든 뒤 2패스에서 `info/link` 대상의 `miniMap/canvas`로 아이콘을 보완한다.

| 탭 | 대상 디렉터리 | 행 수 | 미니맵 보유 |
|---|---|---:|---:|
| 17 `Map1` | Map0–Map3 | 4,382 | 3,847 |
| 18 `Map2` | Map4–Map8 | 1,946 | 1,838 |
| 19 `Map3` | Map9 | 11,141 | 9,684 |
| 합계 | | **17,469** | 15,369 |

- 링크 해석은 원본과 같이 `"Map" + LeftStr(link,1)` 디렉터리일 때만 채택한다.
- 자체 캔버스가 없고 링크가 있는 행 4,197개 중 **3,309개**가 2차 패스로만
  미니맵을 얻는다. 단일 파일만 처리하던 기존 구현이 놓치던 부분이다.
- 이 클라이언트에 `Map7` 디렉터리는 비어 있다(파일 0개) — 빈 파티션도 처리.

fixture: `crates/wz-cli/tests/fixtures/db2_map_parts.expected.json`,
테스트: `db2_map_parts_match_csharp`,
CLI: `wz db2-map-part <Data> <part> --string <String_000.wz>`.

## DB2 Mob 탭 3분할

원본 `LoadMob(Part)`는 `String/Mob.img`를 순회하며 `Mob/<id>.img` 존재와
선행 숫자를 검사한다(탭20: 8·9 제외, 탭21: 8, 탭22: 9).

| 탭 | 필터 | 행 수 |
|---|---|---:|
| 20 `Mob` | 선행 숫자 ≠ 8,9 | 742 |
| 21 `Mob001` | 선행 숫자 = 8 | 1,160 |
| 22 `Mob2` | 선행 숫자 = 9 | 1,585 |
| 합계 | | **3,487** |

`id/name/stand/fly/move/hit/die/thumbnail/link` 9필드가 C#과 완전 일치.
이 데이터에서 `info/link`는 0개다(정보 노드 자체가 3,524 이미지 중 227개).

fixture: `crates/wz-cli/tests/fixtures/db2_mob_parts.expected.json`,
테스트: `db2_mob_parts_match_csharp`,
CLI: `wz db2-mob-part <Data> <part> --string <String_000.wz>`.

## Skill 툴팁 헤더 (`SkillTooltipRender2.RenderSkill`)

`wz-db::skill_tooltip`이 `Skill.CreateFromNode`의 관련 필드와 렌더러의 헤더
줄을 옮겼다.

- 이름: `StringLinker.StringSkill[id].Name`, 없으면 `(null)`
- `[최대 레벨 : N]` — `hyperStat != 0`
- `[마스터 레벨 : N]` — `hyperStat == 0` 이고 `PreBBSkill == false`
- `PreBBSkill` = `level` 자식이 1개 이상 (`levelCommon.Count > 0`)
- `MaxLevel` = `PreBBSkill` 이면 연속된 `level/1..n` 개수, 아니면
  `common/maxLevel`(없으면 0)
- `reqLev` / `req/level`, `req/reqAmount`, `req/<스킬ID>` 요구 레벨

검증: `Skill` 전체 **7,055개**가 C# 하네스와 9필드 + 줄 순서 완전 일치.
`desc`는 원문 그대로 비교했고, 렌더러의 `SummaryParser` 변환은 기존
`skill_summary` 슬라이스(20,629행)에서 이미 C#과 일치한다.

데이터 현황(같은 스캔): 7,055 스킬 중 `level` 3, `common` 0, `req` 0,
`reqLev` 0 → 7,052개가 `[마스터 레벨 : 0]`, 3개는 헤더 줄이 없다.

fixture: `crates/wz-cli/tests/fixtures/skill_tooltips.expected.json`,
테스트: `skill_tooltips_match_csharp`,
CLI: `wz skill-tooltips <Data> --string <String_000.wz>`,
C#: `-- skilltooltips <Data> <String_000.wz>`.

## LuaConsole 샌드박스 (`LuaSandbox.InitLuaEnv`)

원본 `LuaSandbox`는 NLua(Lua 5.1 + CLR 브리지)와 WinForms
(`Application.StartupPath`, `FrmConsole`)에 묶여 macOS에서 실행할 수 없다.
이식 가능한 결정적 부분을 `crates/wz-lua`(mlua, vendored Lua 5.4)로 옮겼다.

| 원본 | Rust | 비고 |
|---|---|---|
| `package.path` 구성(스크립트→플러그인→WCR2→현재, `Distinct`) | `package_path_entries`/`package_path` | 5조합 C# 참조 구현과 일치 |
| `lua["env"] = Env` | `Sandbox::init(env, ...)` | JSON → Lua 테이블 |
| `each(userData)` (IEnumerable) | Lua 배열 테이블용 `each` | CLR이 없어 테이블을 받는다 |
| `DoString` | `Sandbox::do_string` | 반환값/오류 전파 |
| `DoStringAsync` 취소 훅 | 미구현 | Lua debug hook + CancellationToken |

검증(실제 실행):
- `crates/wz-lua/tests/sandbox_parity.rs` 5종 — `package.path` 5조합,
  `require`가 스크립트 폴더의 `lua/<mod>/init.lua`·`<mod>.lua`에서 해석,
  다중 반환값, 런타임 오류 전파, non-table `each`가 0개 값을 돌려주는 계약.
- `crates/wz-lua/src/lib.rs` 단위 테스트 4종.
- CLI `wz lua-run <script>` 로 `env`/`each`/`require` 데모 실행 확인.

C# 대조는 `package.path` 부분만 가능하다. `LuaSandbox.InitLuaEnv`가 NLua
네이티브(5.1)와 WinForms를 요구하므로, 하네스는 원본 소스의 path 구성
블록을 **그대로 옮긴 참조 구현**을 컴파일해 쓴다
(`tools/csharp-worldmap ... -- luapath <script> <plugin> <wcr2> <cwd>`).

NLua `luanet.import_type` 기반 CLR 타입 브리지, `RegisterLuaDelegateType`,
WinForms 콘솔/에디터(`FrmConsole`, `FrmLuaEditor`), 문법 강조
(`AppSyntaxModeProvider`)는 재현하지 않는다.

## ItemTooltip `#cosmetic_EULO#`

원본 `ItemTooltipRender2.ReplaceDescTags`는 `desc`에 이 태그가 있으면 코스메틱
ID로 이름/성별/조사를 만들어 치환한다.

- `Gear.GetGearType` — 특수 코드 구간(1098/1099/1212…, 135xx, 119xx, 179x,
  172xx Astra 보조무기 표)을 그대로 옮김
- `Gear.IsSubWeapon`, `Gear.GetGender`, `Gear.GetCosmeticGender`
- 이름: `StringLinker.StringEqp[cosmetic]`, 없으면 `(null)`
- 조사: **치환 문자열 전체**(성별 접미어 포함)의 마지막 한글 음절 종성으로
  `으로`/`로` 결정 → `(남)`/`(여)`가 붙으면 받침이 있어 `으로`
- `AvatarCanvas.HairColor` 8색(검은/빨간/주황/노란/초록/파란/보라/갈색) 접두어 제거

검증: `desc`에 태그가 있는 **1,133개 아이템**이 C#(실제 `Gear.GetGender` 호출)
결과와 `gender/eqp_name/replacement/replaced`까지 완전 일치.
fixture: `crates/wz-cli/tests/fixtures/item_cosmetic_eulo.expected.json`,
테스트: `item_cosmetic_eulo_matches_csharp`,
CLI: `wz item-cosmetic-eulo <Data> --string <String_000.wz>`,
C#: `-- itemcosmetic <Data> <String_000.wz>`.

## PluginBase `LoadPlugin` 인스턴스화

원본 `PluginManager.LoadPlugin`은 `GetExportedTypes()`에서 `PluginEntry`
서브클래스(비추상)를 골라 `Activator.CreateInstance(type, context)`로 만든다.
macOS에서도 `PluginContext`를 provider=`null`로 생성할 수 있어 **실행 검증이
가능**했다.

- C# 하네스: `context_created=true`, fixture 플러그인 2종 인스턴스화 성공
- 각 인스턴스의 `Name`(AssemblyTitle), `Author`(AssemblyCompany),
  `Version`(AssemblyName.Version), `FileVersion`(InformationalVersion 우선)
- Rust `wz_db::plugin_entries`가 동일한 2행을 낸다

fixture: `crates/wz-db/tests/fixtures/plugin_entries_activated.json`,
테스트: `plugin_activation_parity`,
C#: `-- pluginactivate <plugin.dll> <PluginBase.dll>`.

남은 것: `OnLoad()`가 호출하는 `Context.AddRibbonBar`(DevComponents/WinForms).
