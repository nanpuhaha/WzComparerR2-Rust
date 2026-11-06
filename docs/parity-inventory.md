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

C# 대조:
- `package.path` — 원본 path 구성 블록을 그대로 옮긴 참조 구현
  (`-- luapath ...`).
- **CLR 브리지 — 실제 NLua 실행**. `NLua 1.7.5`가 macOS arm64에서 동작함을
  확인해(네이티브 Lua 포함), `LuaSandbox.InitLuaEnv`의 프렐류드를 **원문
  그대로** 돌렸다. `luanet.import_type('System.Collections.IEnumerable')` +
  `get_method_bysig` + `each()`가 CLR `IEnumerable`을 순회하는지 6케이스로
  기록했다 (`-- luaclr`).

| 케이스 | `select('#', each(x))` | 순회 결과 |
|---|---|---|
| `List<int>{10,20,30}` | 1 | `1=10;2=20;3=30;` |
| `List<string>{"가","나"}` | 1 | `1=가;2=나;` |
| `int[]{1,2,3}` | 1 | `1=1;2=2;3=3;` |
| 빈 `List<int>` | 1 | `` |
| Lua 테이블 `{1,2,3}` | 0 | 오류(반복자 없음) |
| `nil` | 0 | 오류(반복자 없음) |

Rust `wz-lua`는 같은 관찰 계약을 Lua **userdata**로 구현해(`each`는
`type()=='userdata'`일 때만 반복자 반환) 위 6케이스가 완전 일치한다
(`crates/wz-lua/tests/lua_clr_parity.rs`, fixture `lua_clr.json`).

주의(의도적 차이): 원본은 `env`도 CLR 객체(userdata)라 `type(env)`가
`userdata`지만, Rust는 JSON을 Lua 테이블로 넣어 `table`이다. `env.tag` 같은
필드 접근 결과는 동일하다.

여전히 재현하지 않는 것: `RegisterLuaDelegateType`, WinForms
콘솔/에디터(`FrmConsole`, `FrmLuaEditor`), 문법 강조
(`AppSyntaxModeProvider`).

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

## OpenAPI HTTP 계층 (`NexonOpenAPI` / `MapleStory.OpenAPI`)

원본의 실제 HTTP는 `MapleStory.OpenAPI` 3.8.1 NuGet 패키지가 수행한다.
패키지 DLL에서 상수를 추출해 계약을 고정했다
(`crates/wz-openapi/tests/fixtures/nexon_api_paths.json`).

| 항목 | 값 |
|---|---|
| base URL | `https://open.api.nexon.com` |
| 인증 헤더 | `x-nxopen-api-key` |
| 리전 세그먼트 | `maplestory`(KMS) / `maplestorysea`(MSEA) / `maplestorytw`(TMS) |
| 경로 | 패키지에서 38개 추출, 사용하는 것은 `/v1/id`, `/v1/character/basic` |

구현 (`wz-openapi::http`):
- `OpenApiConfig::url` — `{base}/{segment}{path}?{query}`, 쿼리는 UTF-8 퍼센트 인코딩
- `OpenApiConfig::headers` — `x-nxopen-api-key`
- `Transport` 트레이트 + `TcpTransport`(`http://` 한정, 목 서버/테스트용)
- `ApiError` — `error` 봉투(`OPENAPI00004` 등) / 상태코드 / JSON / 전송 오류
- `CharacterBasic`/`CharacterId` DTO(snake_case 그대로)
- `avatar_code_from_image_url` — `look/<CODE>` 추출(아바타 Unpack 입력)

검증(실제 요청/응답):
- `crates/wz-openapi/tests/http_mock.rs` — 로컬 목 HTTP 서버 2종
  - `GET /maplestory/v1/id?character_name=%ED%99%8D%EA%B8%B8%EB%8F%99`
  - `GET /maplestory/v1/character/basic?ocid=ocid-123`
  - 두 요청 모두 `x-nxopen-api-key` 헤더 포함, 12개 필드 매핑 일치
  - 400 + `OPENAPI00004` 봉투 → `ApiError::Api`로 표면화, MSEA 세그먼트 확인
- `crates/wz-openapi/tests/api_constants.rs` — 경로/헤더/세그먼트가 패키지 상수와 일치
- CLI 데모: 목 서버에 대해 `wz open-api-fetch --base-url http://127.0.0.1:PORT --name 홍길동 --key mock-key --json` 실행,
  서버 로그에 위 두 요청과 헤더가 찍히고 12개 필드가 파싱됨

한계: 실제 엔드포인트 호출은 Nexon API 키가 필요하다. HTTPS 전송은
`Transport` 구현 주입으로 대체하며, 저장소에는 목 서버 검증용 `TcpTransport`만
있다(순수 Rust 원칙상 TLS 클라이언트를 새로 들이지 않았다).

## DB2 BIN 그리드 컨테이너 (`DataGrid.SaveBin`/`LoadBin`)

형식(`BinaryWriter` 기준, `int`는 little-endian, `string`은 7비트 길이 + UTF-8):

```
int32 columns
int32 rows
rows × columns:
  string tag        // "str" | "Bitmap"
  str    -> string value
  Bitmap -> int32 length, byte[length]
```

확인한 원본 특이점:
- `SaveBin`은 `string`/`Bitmap` 셀만 쓰는데 `LoadBin`은 모든 셀의 태그를
  읽는다 → 다른 타입이 섞이면 스트림이 어긋난다.
- `LoadBin`의 루프가 `i <= m - 2`라서 **마지막 행을 버린다**.
- PNG는 `MemoryStream.GetBuffer()`(용량)로 쓰여 0 패딩이 붙는다. 용량은
  GDI+가 스트림에 쓰는 청크 크기에 따라 달라진다(측정: 같은 4097바이트도
  1회 쓰기 4097, 1KB 청크 8192). 재현 불가능한 플랫폼 의존이라 Rust는
  정확한 페이로드 길이를 쓴다(C# 하네스도 동일하게 맞춤).

검증:
- `crates/wz-db/tests/db2_bin_parity.rs` 3종
  - `write_bin()` 결과가 C# `BinaryWriter` 산출 바이트와 **base64까지 동일**
  - `read_bin()`이 C# `LoadBin`과 같은 2행(마지막 행 제외)을 복원
  - `read_bin_all()`은 선언된 3행을 모두 복원
- 단위 테스트 4종(헤더/문자열 7비트/비트맵 길이/오류)
- C# 하네스: `-- db2bin`

## DB2 Familiar 그리드 (`LoadFamiliar`)

`Character/Familiar`가 0개라 실데이터 비교는 불가하고, 노드 규칙만 옮겼다.

- `ID` = 이미지 이름(.img 제거)
- `MobID` = `info/MobID`(7자리 패딩) → 없으면
  `Etc/FamiliarInfo.img/<ID>/mob`(기본 `100100`) → 둘 다 없으면 **이전 값 유지**
  (원본은 `MobID`를 루프 밖에서 선언한다)
- `SkillID` = `info/skill/id`, 이름/설명은
  `String/FamiliarSkill.img/skill/<id>` 우선, 없으면 `String/Familiar.img/skill/<id>`
  → `"{skillID}:{name}"`
- `CardID` = `FamiliarInfo.img/<ID>/consume`(있으면) 아니면 `info/monsterCardID`
- 카드 아이콘 = `Item/Consume/0287.img/0<CardID>/info/icon` 또는
  `0238.img/0<CardID>/info/iconRaw`
- `CardName` = `String/Consume.img/<CardID>/name`

검증: 합성 노드 fixture 단위 테스트 2종
(`info/MobID` 우선순위, FamiliarInfo 기본값, 이전 MobID 유지).

## ItemTooltip 텍스트 레이아웃 (`TextRenderer.DrawPlainText`)

`GearGraphics.DrawString`은 `FormattedTextRenderer`(= `TextRenderer<Font>`의
파생)에 위임하고, `WordWrapEnabled = false`로 `DrawRunsInner`를 호출한다.
그 코어(`SplitWords` → `MeasureRuns` → `LayoutRuns` → `Flush`)를 그대로 옮겼다.

- 측정 함수를 **주입**받아 순수 계산만 수행(`measure(&str) -> i32`)
- 인덱스는 원본과 같이 UTF-16 코드 유닛 기준
- `PrepareRuns`(StringReader.ReadLine 의미), `SplitWords`(공백/탭/
/
),
  `MeasureRuns`, `MeasureChars`, 정렬 보정(`Center`/`Right`), 줄 끝 `,.` 병합
  규칙까지 이식

검증 방법이 이전 슬라이스와 다르다: C# 하네스가
**`WzComparerR2.Text.TextRenderer<string>`를 직접 상속**해 추상 메서드
(`MeasureRuns`/`MeasureChars`/`Flush`)만 결정적 측정(`7 × UTF-16 길이`)으로
바꾸고 `DrawPlainText`를 호출한다. 즉 **줄바꿈/오프셋 누적/정렬은 원본 코드
그 자체**이며 전사(transcription)가 아니다.

검증: 12개 케이스(줄바꿈, 문자 단위 분할, 명시적 `
`/`
`/`
`,
Center/Right 정렬, 줄 끝 `,.` 병합, 공백만, 빈 문자열)에서 `(start,len,x,y)`
목록과 최종 `y`가 완전 일치.

fixture: `crates/wz-db/tests/fixtures/text_layout.json`,
테스트: `crates/wz-db/tests/text_layout_parity.rs`,
C#: `-- textlayout`.

이식 중 잡은 실제 버그: `flush()`가 `start = end = -1`로 인덱스를 초기화하는
줄을 빠뜨려 다음 줄 조각이 이전 줄 시작 인덱스를 물고 늘어졌다. 원본과
비교하면서 수정했다.

범위 한계(정직하게):
- 픽셀 비교는 여전히 불가능하다. 실제 `GearGraphics`는 폰트 메트릭을
  `Graphics.MeasureString`에서 얻고, macOS에는 GDI+가 없다.
- 색상 태그 파서(`Parser.Parse`, `#c`/`#fc`/`#t`/이미지 토큰)는 이번 슬라이스에
  포함하지 않았다. `DrawFormatString` 경로도 같은 `LayoutRuns` 코어를 쓰므로
  줄바꿈/오프셋 계산은 공유되지만, 태그별 run 분할은 미검증이다.

## ItemTooltip 아이콘 픽셀 (`GearGraphics.EnlargeBitmap`)

`ItemTooltipRender22`/`GearTooltipRender2`는 아이템 아이콘을 `EnlargeBitmap`으로
2배 확대해 `DrawImage` 한다. 이 함수는 `Bitmap.LockBits`(32bpp → stride ==
width*4) + `Marshal.Copy`로 바이트 배열을 만든 뒤 다음 두 단계만 수행한다.

1. 원본 `(j, i)` 픽셀을 목적지 `(2j, 2i)`와 `(2j+1, 2i)`에 복사
2. 목적지 행 `2i`를 행 `2i+1`로 통째로 복사

색 공간/알파 합성과 무관한 **순수 바이트 변환**이라 GDI+ 없이 픽셀 단위로
비교할 수 있다.

검증:
- 입력은 실제 클라이언트 캔버스다. 아이템 아이콘은 `_outlink`로 분할
  `_Canvas` 파일에 있으므로 `Item/Consume/_Canvas/_Canvas_000.wz`에서
  `0200.img/0200000N/info/icon`(27×30), `iconRaw`(27×27) 6개를 디코드했다.
- C# 하네스는 원본 바이트 루프를 그대로 옮긴 참조 구현으로 확대해 SHA-256을 낸다.
- `crates/wz-db/tests/pixel_ops_parity.rs` — 6개 아이콘의 출력 길이/크기/SHA-256
  완전 일치(`wz-db::pixel_ops::enlarge_2x`).

fixture: `crates/wz-db/tests/fixtures/pixel_ops_input.json`(입력 RGBA),
`pixel_ops.json`(C# 출력 해시), C#: `-- pixelops <fixture.json>`,
CLI: `wz pixel-fixture <canvas.wz> --limit N`.

### 아이콘 블록 합성 (불투명 blit)

`ItemTooltipRender22`는 확대 아이콘을 다음 좌표에 그린다.

```
x = iconX + 6 + (1 - originX) * 2
y = picH  + 6 + (33 - originY) * 2
```

이 배치와 **알파 255 픽셀만** 복사하는 blit을 `wz-db::pixel_ops::compose_icon_block`
으로 구현했다(120×120 투명 ARGB 캔버스, `iconX=15`, `picH=7`).

- 입력: 실제 캔버스 6개 + 원본 아이템 파일의 `origin`
  (`origin`은 분할 `_Canvas`가 아니라 원본 `Consume_000.wz`의
  `PngRef.sub_property`에 들어 있어 경로로 대조한다)
- C# 하네스가 같은 좌표식/같은 불투명 blit을 수행
- `crates/wz-db/tests/icon_block_parity.rs` — 6개 모두 `(dx,dy)`와 캔버스
  SHA-256 완전 일치(예: `dx=29, dy=19`, `eef9db66…5da3`)

fixture: `icon_block.json`, C#: `-- iconblock <fixture.json>`.

### 알파 source-over 합성

반투명 아이콘을 배경 위에 얹는 경로를 **정수 straight-alpha** 규칙으로
구현하고 고정했다(`wz-db::pixel_ops::blend_pixel`).

```text
outA = srcA + round(dstA * (255 - srcA) / 255)
outC = round((srcC*srcA + dstC*dstA*(255-srcA)/255) / outA)
```

반올림은 각 나눗셈에서 `+d/2` 후 정수 나눗셈(양수 전용)으로 명시했다.

검증:
- 배경은 fixture에 담긴 결정적 반투명 패턴(120×120, alpha 64..191)
- 실제 아이콘 6개를 확대·배치 후 전 픽셀 source-over
- C# 하네스가 **같은 정수 수식**으로 합성한 결과와 SHA-256 완전 일치
  (`crates/wz-db/tests/icon_blend_parity.rs`, 예: `5501fcce…d2c6`)
- 단위 테스트로 규칙 고정: 불투명 위 반투명(`[0,0,255,128]` over
  `[255,0,0,255]` → `[127,0,128,255]`), 투명 소스/투명 배경 경계

남는 한계(명시적 분리):
- 실제 `Graphics.DrawImage`는 **premultiplied alpha + 감마**를 쓰므로 위
  정수식과 픽셀 값이 다를 수 있다. macOS에는 GDI+가 없어 그 경로는 실행
  비교가 불가능하다. 즉 "원본과 동일한 합성식"이 아니라 "GDI+ 없이 재현
  가능한 결정적 합성식"을 검증한 것이다.
- 텍스트 래스터라이제이션(폰트 힌팅/안티에일리어싱)도 GDI+ 의존이라 미검증이다.

## Common/Controls 마우스 히트테스트/드래그/휠 (`AnimationControl`)

WinForms 이벤트 객체 대신 좌표/델타를 입력으로 받는 순수 계산으로 옮겼다.
C# 의미론에 의존하는 지점이 있어 그대로 재현했다.

| 원본 | Rust | 주의점 |
|---|---|---|
| `GetItemAt`/`GetItemsAt` 사각형 | `item_rect` | `Math.Round(float)` = **은행가 반올림**(0.5→짝수) |
| `Rectangle.Contains` | `rect_contains` | 오른쪽/아래 경계 제외(`x <= px < x+w`) |
| 히트 순서 | `hit_items` | 뒤에서부터(위 레이어 먼저) |
| `MouseMove` 드래그 | `drag_position` | `cur - mouseDown + start` |
| 드래그 저장 박스 | `outside_drag_box` | `(mouseDown, dragSize)` 밖이면 저장 |
| `MouseWheel` + `GlobalScale` | `wheel_scale` | `e.Delta / 120`이 **정수 나눗셈**(<120은 0틱), 100% 횡단 시 1 고정, 0.1~10 클램프 |

검증: C# 하네스가 실제 `Math.Round`/`System.Drawing.Rectangle`로 계산한
**115케이스**(rects 18, hits 60, ties 6, drags 4, drag boxes 8, wheels 9)와
완전 일치.
- 은행가 반올림 확인: `3*0.5→2`, `5*0.5→2`, `7*0.5→4`, `1*1.5→2`,
  `3*1.5→4`, `5*1.5→8`
- 휠: `(0.95, +120) → 1.0`(100% 횡단 고정), `(9.9, +240) → 10.0`(클램프),
  `(1.0, +60) → 1.0`(정수 나눗셈 0틱)

fixture: `crates/wz-db/tests/fixtures/controls_input.json`,
테스트: `crates/wz-db/tests/controls_input_parity.rs`,
C#: `-- controlsinput`.

범위: WinForms 컨트롤 인스턴스화/이벤트 구독/`Invalidate` 호출은 여전히
호스트 의존이다. 여기서는 이벤트가 전달하는 좌표/델타로부터의 계산만 다룬다.

## GDI 비의존 드로잉 계산 (`wz-db::draw_math`)

툴팁/애니메이션 렌더링에서 **GDI 없이 계산 가능한 기하**만 옮겼다.

| 원본 | Rust | 규칙 |
|---|---|---|
| `GearGraphics.DrawGearDetailNumber` | `gear_detail_number_glyphs` | 숫자 `x += w+1`, `-`는 `y+3`에 그리고 `x += w+1`, `%`는 `x+1`에 그리고 `x += w+2`, 그 외 문자는 전진하지 않음(원본 switch에 default 없음) |
| `AnimationGraphics.Draw` | `sprite_destination` | `SpriteBatch.Draw(position: Zero, origin: frame.Origin, scale: 1)` + 아이템 `Position` 평행이동 → `(pos - origin, source_size)` |
| `SpriteBatchEx.DrawPath` | `path_rects` | 폴리라인 → 1px 사각형, 역방향은 `+1` 보정, 닫힌 경로는 마지막 점 사각형 생략, 0 이하 크기는 버림 |
| `GearGraphics.GetBorderPath`/`GetIconBorderPath` | `border_path`/`icon_border_path` | 13점/5점 고정 경로 |

검증: C# 하네스가 같은 루프를 옮기고 사각형 판정에 실제
`System.Drawing.Rectangle`을 사용해 4개 그룹 **31케이스**
(글리프 18, 목적 사각형 4, 경로 6, 보더 3+3)를 계산했고 모두 일치
(`crates/wz-db/tests/draw_math_parity.rs`).

폭은 실제 리소스 비트맵(`ToolTip_Equip_Can_*`)이 없어 fixture 값
(`0-9=7`, `-=6`, `%=9`, cannot은 +2)으로 고정했다.

남는 한계: 텍스트 래스터라이제이션과 GDI+ `DrawImage`의 premultiplied/감마
합성은 여전히 macOS에서 실행 불가다.

## OpenAPI 실제 엔드포인트 (오류 경로)

`https://open.api.nexon.com`에 **실제로 요청**해 응답을 캡처했다
(2026-11-06).

```text
GET /maplestory/v1/id?character_name=test
  -> 400 {"error":{"name":"OPENAPI00004","message":"Please input valid parameter"}}

GET /maplestory/v1/id?character_name=test   (x-nxopen-api-key: test-invalid-key)
  -> 400 {"error":{"name":"OPENAPI00005","message":"The apikey is not valid."}}
```

- 우리 클라이언트가 만드는 URL(`OpenApiConfig::url`)과 헤더
  (`x-nxopen-api-key`)가 위 호출과 **문자열 단위로 동일**함을 테스트로 고정
- `wz_openapi::parse_api_error`가 두 실측 본문을 `ApiErrorBody`로 정확히 해석
- 400 응답이 `ApiError::Api`로 승격되는 경로 확인

fixture: `crates/wz-openapi/tests/fixtures/nexon_error_no_key.json`,
`nexon_error_bad_key.json`;
테스트: `crates/wz-openapi/tests/real_endpoint_errors.rs` (5종).

남는 한계: **성공(200) 경로는 Nexon API 키가 필요**해 호출하지 못했다.
요청/파싱 계층 자체는 목 서버(`http_mock.rs`)로 이미 검증되어 있다.

## 데이터 부재 재스캔 (범위 확대)

이전 스캔은 WorldMap/String 등 일부 경로만 봤다. 이번에 범위를 넓혀
다시 확인했다.

| 항목 | 스캔 범위 | 결과 |
|---|---|---|
| `QuestLimit` / WorldMap `Fog` | `Data/Map` 전체 137 WZ / 28,490 이미지 | `QuestLimit` 0. `fog` 4건은 `Effect3.img/BossLucid/fog`, `ForestTraining.img/obj/fog` 같은 **무관한 스프라이트 이름** |
| `Recipe.img` | `Data` 전체 792 WZ 이미지 엔트리 | 0 |
| Familiar 이미지 | `Data` 전체 이미지 엔트리 | 0 |
| weather 이미지 | `Data` 전체 이미지 엔트리 | 0 (원본 C#에도 기능 없음) |

즉 `#cosmetic_EULO#` 때와 달리, 이 항목들은 스캔 범위를 넓혀도 데이터가 없다.

## LuaConsole 델리게이트 마샬링 (`RegisterLuaDelegateType`)

원본 `LuaConsole`는 WzLib 노드 조회를 Lua 함수로 넘기기 위해
`lua.RegisterLuaDelegateType(typeof(GlobalFindNodeFunction), typeof(LuaGlobalFindNodeFunctionHandler))`
를 쓴다. macOS에서 NLua **1.7.3**(원본과 동일 버전)이 동작하므로 실제로
등록·호출해 계약을 확인했다.

### 확인된 바인딩 규칙

핸들러의 `CallFunction` 시그니처는 **델리게이트 시그니처와 정확히 같아야**
NLua가 바인딩한다.

| 케이스 | 결과 |
|---|---|
| `GlobalFindNodeFunction`(=`(string, Wz_File=null)`) + **1-파라미터** 핸들러(원본 형태) | `System.ArgumentException: Cannot bind to the target method because its signature is not compatible with that of the delegate type.` |
| 같은 델리게이트 + **2-파라미터** 핸들러 | 정상 호출, `nil` 반환 시 `(null)` |
| `delegate string TwoArg(string, int)` + 동일 시그니처 핸들러 | `x:7`, `:-3` |
| 같지만 Lua 함수가 3-파라미터인 경우 | `x7nil` (세 번째는 `nil`) |
| `delegate int Calc(int a, out int b)` | **프로세스 abort(exit 134)** — try/catch 불가 |

즉 **원본 `LuaGlobalFindNodeFunctionHandler`(1-파라미터)는 NLua 1.7.3에서
바인딩되지 않는다**. 실제로 동작시키려면 핸들러가 `(string, Wz_File)`을 받아야
한다. 이 사실은 코드만 보고는 알 수 없고 실행으로만 확인된다.

### Rust 재현

`wz-lua`에 호스트가 Lua 콜백을 호출하고 **첫 반환값만** 쓰는 계약을 넣었다.

- `Sandbox::call_first_return(callback, args)` — `MultiValue` 중 첫 값만 반환
- 전역 `host_call(callback, ...)` — 같은 규칙의 Lua 노출 함수

검증: fixture(`crates/wz-lua/tests/fixtures/lua_delegate.json`)의 실행 가능한
4케이스(`(null)`, `x:7`, `:-3`, `x7nil`)가 완전 일치하고, 1-파라미터 핸들러
바인딩 실패 메시지와 out-param abort(134)를 fixture로 고정했다
(`crates/wz-lua/tests/host_callback.rs`, 3 테스트).

C# 하네스: `-- luadelegate <case>` (케이스별 프로세스 분리 — abort를 기록하기 위해).

남는 한계: 실제 WzLib `Wz_Node` 반환 마샬링은 이 저장소에 노드 어셈블리
의존이 커서 값(`nil`/문자열) 수준에서만 확인했다. WinForms 콘솔/에디터 UI와
문법 강조는 여전히 호스트 의존이다.

## ItemTooltip 세로 배치 (`RenderItem`의 picH/splitterH)

텍스트 높이는 `Graphics.MeasureString`(GDI+)에서 나오므로 실행할 수 없다.
그래서 **높이를 주입**하고 원본의 누적 규칙만 그대로 옮겼다
(`wz-db::item_tooltip_layout`).

| 원본 | 규칙 |
|---|---|
| 시작 | `picH = 0` → `+= 10` → `+= 20` |
| 라벨 | `wonderGrade`가 1/4/5/6이면 `+= 15`, 그 외 양수는 `-= 15` 후 `+= 15`(순증 0). 없으면 BTS → BLACKPINK `else-if` |
| 상단 속성 | 각 줄 높이만큼 |
| 유효기간/생명의 물 | 각 18 |
| 1차 splitter | `splitterH.Add(picH - 1)` |
| 아이콘 | `picH += 7`, `iconY = picH`, `picH += 3` |
| 본문 블록 | 요구레벨/펫스탯/설명/성향/레시피/정액제 각각 `+= 높이` 후 `+= 3` |
| 하단 속성 | 있으면 `picH = max(picH, iconY + 88)`, splitter, `+= 10`, 각 높이 |
| 마무리 | `picH = max(iconY + 92, picH + 2)` |

검증:
- 입력은 실제 `Data/Item/Consume` **18,376개**에서 기능별로 뽑은 **320개**
  (하단 속성 100, 사용기간 113, 레시피 60, 설명만 60)
- 텍스트 높이는 결정적 규칙(`wrapped_lines`: `\n` 분리 후 40칸 접기, 높이 18/줄)으로
  양쪽에 동일하게 주입
- C# 하네스(`-- itemlayout <fixture.json>`)가 같은 누적 루프를 실행
- `crates/wz-db/tests/item_layout_parity.rs` — 320개 모두 `pic_h`, `icon_y`,
  `splitter_h` 완전 일치

fixture: `item_layout_input.json`(실데이터 유래 입력),
`item_layout_expected.json`(C# 출력),
CLI: `wz item-layout-fixture <Data> --string <String_000.wz> --dir Consume`.

남는 한계: 폰트 래스터라이제이션과 GDI+의 premultiplied/감마는 여전히 실행
불가다. 상단 속성(`GetItemTopAttributeString`)은 spec 기반이라 이번 비교에서는
0으로 두었다(하단 `desc_leftalign` 1,126건은 포함).

## Network 실서버 접속 시도 (2026-11-06)

제공된 `100.115.176.120`에 원본 기본 포트(2100)를 포함한 16개 포트를
프로브했다.

```text
80, 443, 2100, 2101, 2102, 3306, 5000, 6379, 7575, 8000,
8080, 8484, 8888, 9000, 21000, 21001
-> 전부 ConnectionRefusedError (Errno 61)
```

호스트는 응답하지만(거부 응답) 어떤 포트에서도 서비스가 열려 있지 않다.
실서버 검증은 **올바른 주소/포트**가 있어야 진행할 수 있다.

## 서식 문자열 파서 (`Text.Parser.Parse`)

툴팁/스킬 설명의 `#c`, `#fcAARRGGBB#`, `#$r`, `#$^f`/`#$$`, `#k`,
`#@id/w/h@`, `\n` 토큰을 run 목록으로 분해한다. GDI와 무관하므로 C# **원본
`Parser.Parse`를 직접 실행**해 비교했다(`WzComparerR2.Common`은 macOS에서 로드
가능).

| 토큰 | 원본 동작(그대로 유지) |
|---|---|
| `#c` | flush 후 색 스택에 `c` push(이미 `c`면 push 안 함) |
| `#fc…` | `0x`는 건너뛰고 `8자리 + #`일 때만 색 push. 형식이 틀리면 `break`만 하고 **strPos를 되돌리지 않아 토큰 글자가 본문에 남는다** |
| `#k` | 색 pop |
| `#$xx` | 색 스택 push(`$x` 등) |
| `#$^f` / `#$$` | 폰트 스택 push/pop |
| `#@id/w/h@` | `id`/`w`/`h` 중 파싱되는 값만 사용, 실패 시 이미지 버림 |
| `#` (그 외) | 스택이 1개면 `c` push, 아니면 pop |
| `\r` | **무시**(개행 아님) |
| `\n` | `LineBreak` 요소 |
| `\` + 그 외 | 문자 자체를 버림(`\0`) |
| 끝의 단독 `\` | `#`로 바뀌어 색 토글로 처리 |
| 빈 run | 만들지 않음(`sb.Length > offset`일 때만) |

검증:
- 입력은 `Data/String/String_000.wz`에서 `#`가 있는 실제 문자열 **151개**
  (`#t` 12, `#fc` 12, `#$` 4, `#@` 1, `#k` 12, `#c` 90, 무작위 20)
- C# 하네스가 원본 `Parser.Parse`를 호출해 **882개 run**을 덤프
- `crates/wz-db/tests/format_parser_parity.rs` — run 종류/텍스트/색/폰트/
  이미지 id·크기/개행이 모두 일치
- 단위 테스트 7종으로 위 표의 특이점 고정

fixture: `format_parser_input.json`, `format_parser_expected.json`;
C#: `-- formatparse <fixture.json>`.

이로써 ItemTooltip 파이프라인에서 **남은 미검증部分是 글리프 래스터라이제이션과
GDI+ premultiplied/감마 합성뿐**이다(파서 → run → 레이아웃 기하 → 아이콘
확대/배치/불투명 blit/정수 source-over → 세로 배치가 모두 실행 비교됨).

## LuaConsole 문법 강조 (`Resources/Lua.xshd`)

원본 에디터(`FrmLuaEditor`)는 `AppSyntaxModeProvider` → `Lua.xshd`로 강조한다.
ICSharpCode.TextEditor는 WinForms라 로드할 수 없으므로, **설정 파일이 정의한
토큰 경계**를 `wz-lua::highlight`로 옮겼다(색/굵기는 UI 몫).

| 규칙 | 처리 |
|---|---|
| `--[[ … ]]` / `--` | 블록/줄 주석(우선순위 최상) |
| `"…"`, `'…'`(줄 끝에서 중단), `[[ … ]]` | 문자열 |
| `LanguageKeywords` 21개 | 키워드 |
| `AccessKeywords` `self` | 접근 키워드 |
| `Punctuation` 25종(중복 `/` 제거 후) | 구두점 |
| 숫자 | `\d+(\.\d+)?` — xshd에 정규식이 없어(구형 `<Digits>` 기본값) 이 규칙으로 고정 |
| 그 외 | 식별자/공백/기타 |

검증:
- C# 하네스가 **원본 `Lua.xshd`를 실제로 파싱**해 키워드/구두점 목록을 추출하고,
  같은 규칙으로 원본 예제 5개를 토큰화
- Rust 토큰 스트림이 5개 파일에서 **토큰 수와 SHA-256까지 완전 일치**
  (`DumpAnimations` 915, `DumpImages` 659, `DumpSounds` 494, `DumpXml` 324,
  `Helper` 233)
- 원본 예제 파일은 저장소에 넣지 않으므로, 원본 체크아웃이 없으면 규칙 집합
  검사만 수행하고 실제 비교는 건너뛴다(`WZ_C2_EXAMPLES`로 경로 지정 가능)
- BOM은 `strip_bom`으로 제거한다(.NET `ReadAllText`와 동일)

fixture: `crates/wz-lua/tests/fixtures/lua_syntax.json`,
테스트: `crates/wz-lua/tests/lua_syntax_parity.rs`, 단위 테스트 6종,
C#: `-- luasyntax <Lua.xshd> <file.lua>…`.

남는 것: 에디터 UI(콘솔/에디터 창), 색상/폰트 렌더링, `MarkPrevious`
(직전 토큰을 메서드 호출색으로 바꾸는 규칙)는 UI 계층이라 미구현이다.

## ItemTooltip 오프스크린 렌더 (`wz-avatar::item_tooltip_canvas`)

검증된 부품을 실제 RGBA 캔버스로 합친다.

1. `wz_db::format_parser` — `#c`/`#t`/`#fc`/`#@` 토큰 제거(원본 Parser와 일치)
2. `wz_db::item_tooltip_layout` — `picH`/`splitterH`/`iconY`
3. `wz_db::pixel_ops` — 아이콘 2배 확대 + 배치식 + straight-alpha source-over
4. 게임 폰트 래스터(`draw_game_text`, GUI와 동일 경로)

출력: 300×`picH` RGBA. 배경/테두리/구분선은 고정 색, 텍스트는 게임 폰트.

검증:
- 입력은 **실제** 클라이언트 아이콘(`0200.img/02000000/info/icon` 27×30,
  origin `-3,30`)과 실제 `String_000.wz` 서식 문자열
- 300×153, SHA-256 `76cffc4d2e1609536bc2fe2d467011824a54baf35f250cb2cd90b0339eaee526`
  를 fixture로 고정
- `crates/wz-avatar/tests/item_tooltip_canvas_render.rs` 2종(해시 고정,
  아이콘 없이 렌더) 통과
- CLI `wz item-tooltip-render <Data> <fixture.json> [--out out.ppm]`로 같은 해시 출력
- GUI 인스펙터의 아이템 툴팁 경로가 이 렌더러를 사용한다(egui 텍스처 업로드)

남는 한계: 게임 폰트는 GDI+가 아니므로 **원본 픽셀과 동일하지 않다**. GDI+
premultiplied/감마와 글리프 힌팅은 macOS에서 실행할 수 없어 여전히 미검증이며,
이 렌더는 "결정적 회귀 기준"이지 "원본과의 픽셀 동일성" 주장이 아니다.

## 스프라이트 호출 기하 (`AnimationGraphics.Draw` / `FrameAnimationData`)

`AnimationGraphics.Draw`는 프레임마다 `SpriteBatch.Draw`를 한 번 호출한다.
회전 0, 배율 1, `SpriteEffects.None`, `Color.White * A0`,
`StateEx.NonPremultipled_Hidef`이며, 위치는 `animator.Position` 평행이동이다.
`FrameAnimationData`의 `ApplyFlip`/`ApplyRotation`은 **텍스처를 구워** origin을
바꾼다.

| 함수 | 규칙 |
|---|---|
| `sprite_destination_scaled` | `(position - origin*scale, size*scale)`, 은행가 반올림 |
| `frame_sprite_call` | source/destination/origin/alpha + 회전 0·배율 1·flip 없음·NonPremultiplied |
| `flipped_origin` | `flipX → w - originX`, `flipY → h - originY` |
| `rotation_bound` | 네 모서리를 회전해 min/max → `ceil` 크기·origin (`Math.Sin/Cos` double 정밀도) |
| 블렌드 상태 | `NonPremultiplied_Hidef`, `Opaque`, `SrcAlphaMask`, `MultiplyRGB` 인자 |

검증:
- 입력은 실제 `Data/Character` 캔버스 **120개**(width/height/origin)
- C# 하네스가 같은 공식을 double로 계산해 5개 위치 × 4개 배율 = 20 목적 사각형,
  flip 4조합, 회전 6각도(0/30/90/180/270/45)를 덤프
- `crates/wz-db/tests/sprite_geometry_parity.rs` 2종이 **모든 값과 4개 블렌드
  상태**를 완전 일치로 확인
- 부동소수 주의: `MathHelper.ToRadians(90)`의 cos가 0이 아니라 `-4.37e-8`이라
  회전 경계가 1px 밀린다(원본도 동일). Rust도 같은 float 상수·double 삼각함수를
  써서 일치시켰다.

fixture: `sprite_frames_input.json`, `sprite_geometry.json`,
CLI: `wz frame-geometry-fixture <Data> --dir Character`,
C#: `-- spritegeometry <fixture.json>`.

남는 것: 실제 GPU 드로우(텍스처 업로드/샘플러/블렌드 적용)는 wgpu 호스트
의존이며, 여기서는 **호출 인자**만 검증했다.

## CPU 스프라이트 샘플링/합성 (`SpriteBatch.Draw` 등가)

`AnimationGraphics.Draw`가 만든 호출 인자를 **실제 픽셀**에 적용한다.

- `extract_rect` — `AtlasRect` 만큼 소스에서 잘라낸다(경계 검사 포함)
- `compose_sprite` — 목적 사각형으로 이동, 크기가 다르면 최근접 샘플링,
  `flip_h`/`flip_v`, `frame.A0` 알파 스케일
- 블렌드
  - `NonPremultiplied_Hidef`: straight-alpha source-over
    (`outC=(Cs*As+Cd*Ad*(255-As)/255)/outA`, `outA=As+Ad*(255-As)/255`,
    각 나눗셈 `+d/2` 반올림)
  - `Opaque`: 소스 복사
  - 그 외(`MultiplyRGB` 등): `ComposeError::UnsupportedBlend`

검증:
- 입력은 실제 `Data/Character` 캔버스 **6개**의 RGBA 픽셀
- 케이스는 AtlasRect 3종(전체/1·4분면/중앙) × flip 2조합 = **36개**
  (블렌드는 케이스 인덱스로 교대, 알파 200/255 혼합)
- C# 하네스가 같은 추출/합성을 120×120 투명 캔버스에 수행
- `crates/wz-db/tests/sprite_compose_parity.rs` — 36개 SHA-256 완전 일치
- 단위 테스트 5종(추출/불투명 flip/클리핑/알파/미지원 블렌드)

fixture: `sprite_compose_input.json`, `sprite_compose.json`,
CLI: `wz sprite-compose-fixture <Data> --dir Character --limit 6`,
C#: `-- spritecompose <fixture.json>`.

남는 것: 실제 GPU 텍스처 업로드/샘플러(선형 보간)·sRGB 변환은 호스트 의존이며,
여기서는 최근접 샘플링 + straight-alpha 규칙만 검증했다.

## LuaConsole 에디터 패널 (egui 색상 렌더)

원본 `FrmLuaEditor`는 ICSharpCode.TextEditor(WinForms)를 쓴다. 여기서는 검증된
토크나이저 결과를 **egui `LayoutJob`**으로 옮겨, 같은 `Lua.xshd` 색으로
표시한다(`wz-gui::panels::lua_editor`).

색 매핑(`Resources/Lua.xshd`):

| 토큰 | 색 |
|---|---|
| Comment | Green `#008000` |
| String/Char/긴 문자열 | Red `#FF0000` |
| Digits | DarkBlue `#00008B` |
| Punctuation | DarkGreen `#006400` |
| LanguageKeywords / AccessKeywords | Blue `#0000FF` |

검증:
- `crates/wz-lua/tests/fixtures/lua_syntax_colors.json` + `lua_syntax_colors`
  테스트로 매핑 고정
- GUI 단위 테스트 2종: 합성 스크립트(토큰 125, 색 5종)와 텍스트 왕복
- 스모크 출력: `SMOKE OK: lua highlight tokens=125 colored=57 colors=5`

패널은 소스 편집 + 강조 미리보기를 제공하며, 기본 소스는 모든 토큰 종류를
포함하는 합성 스크립트다(원본 예제 파일은 저장소에 넣지 않는다).

남는 것: 실제 에디터 위젯(줄 번호, 커서 단위 토큰 재계산, 스크롤 동기화)과
GDI+/TextEditor의 글꼴 힌팅은 UI 의존이라 미구현이다.

### 에디터 거터·커서 재토큰화

- `gutter_text` — `1..=줄수` 거터(줄 수는 `\n`+1)
- `token_at_char` — 문자 오프셋 → 토큰 종류(끝 경계는 다음 토큰)
- `cursor_status` — `줄 L, 열 C · 토큰 Kind`
- `selection_kinds` — 선택 구간과 겹치는 토큰 종류(중복 제거)
- 소스가 바뀔 때만 재토큰화(egui temp 캐시), 선택이 있으면 종류를 함께 표시

검증: GUI 테스트 3종(거터/커서·토큰/선택) + 스모크
`SMOKE OK: lua editor lines=18 cursor=줄 1, 열 9 · 토큰 Comment`.

## OpenAPI 성공(200) 경로 파싱

실제 200 응답은 Nexon API 키가 필요해 캡처하지 못했다. 공식 스키마를 따른
fixture로 파싱을 고정했다.

- `parse_character_basic_json` / `parse_character_id_json` 공개
- fixture `nexon_character_basic_200.json`(12필드)로 값·`look/<CODE>` 추출 확인
- malformed 본문은 `ApiError::Json`
- 목 서버 200 왕복(`http_mock.rs`)과 함께 성공 경로 파싱이 검증됨

테스트: `crates/wz-openapi/tests/success_fixture.rs` (3종).

## 플러그인 리본 주입 모델 (`PluginContext.AddRibbonBar`)

원본은 DevComponents `RibbonBar`를 만들어 패널에 넣는다. 컨트롤을 만들 수 없어
**배치 규칙**을 `wz-db::plugin_ribbon`으로 옮겼다.

1. 탭 이름을 `OrdinalIgnoreCase`로 찾는다(없으면 `无法找到RibbonPanel。`)
2. 기준은 `ribbonPanel.Controls[0]`(가장 최근 추가된 바)
3. 새 바 `Location = (last.X + last.Width, last.Y)`,
   `Size = (max(1, width), last.Height)`
4. `Controls.Add` 후 `SetChildIndex(bar, 0)` → 새 바가 인덱스 0

따라서 같은 탭에 연속 추가하면 오른쪽으로 이어 붙고, 폭 0/음수는 1로
클램프되어 다음 바 위치에 영향을 준다.

검증:
- C# 하네스가 실제 `PluginContext`(provider=null)를 생성해 사용 가능함을
  확인(`context_created=true`)하고, 같은 규칙으로 6개 호출을 계산
- 탭 3개(초기 바 포함), 호출 6개(대소문자 혼용·폭 0·음수·없는 탭·빈 패널)
- `crates/wz-db/tests/plugin_ribbon_parity.rs` — 호출별 좌표/오류와 최종
  패널 바 순서가 완전 일치(예: 폭 0→1이라 다음 바 x가 211)
- 단위 테스트 4종

fixture: `plugin_ribbon_input.json`, `plugin_ribbon.json`,
C#: `-- pluginribbon <PluginBase.dll> <fixture.json>`.

남는 것: 실제 `RibbonBar`/`RibbonPanel` 위젯 생성과 `OnLoad()` 안에서의
호출 순서(플러그인별)는 WinForms 호스트 의존이다.

## 오프스크린 wgpu 업로드/샘플링/블렌드 (`wz-gui::offscreen`)

GUI가 쓰는 것과 같은 `wgpu` 30으로, CPU에서 만든 RGBA 스프라이트를 텍스처로
업로드하고 화면 크기 쿼드에 그린 뒤 readback 한다. 창/서피스가 필요 없다.

- 업로드: `Rgba8Unorm`, `TEXTURE_BINDING | COPY_DST`
- 샘플러: `Nearest` / `Linear`(ClampToEdge)
- 블렌드: straight-alpha source-over
  (`color: SrcAlpha/OneMinusSrcAlpha`, `alpha: One/OneMinusSrcAlpha`)
- 렌더 타깃: `Rgba8Unorm` + `COPY_SRC`, row padding 256 정렬
- readback SHA-256 고정

검증(실제 GPU 실행):
- 2×2 스프라이트(불투명 빨강/초록, 반투명 파랑, 투명) → 4×4·8×8 렌더
- `nearest_4x4` `6231e73b…d891`, `nearest_8x8` `9b503e6e…302c`,
  `linear_8x8` `42a03bc3…2644`를 fixture로 고정
- Nearest와 Linear 해시가 다름을 확인
- **premultiplied 관찰**: 반투명 파랑 `(0,0,255,128)`이 GPU 블렌드 후
  `(0,0,128,128)`로 기록된다. 즉 이 경로는 GDI+의 premultiplied와 같은
  계열이며, CPU `sprite_compose`(straight-alpha)와 값이 다르다.
- GPU 어댑터가 없으면 테스트는 skip

fixture: `crates/wz-gui/tests/fixtures/offscreen_sprite.json`,
테스트: `offscreen::tests`, `offscreen_hash_tests` (GUI 68개에 포함),
스모크: `SMOKE OK: gpu sprite nearest 8x8 sha256=…` / `linear 8x8 …`.

## 플러그인 이벤트/무효화 모델 (`PluginContext` 이벤트, `Invalidate`)

WinForms 없이 관찰 가능한 규칙만 옮겼다(`wz-db::plugin_events`).

- **이벤트**: `SelectedNode1/2/3Changed`, `WzOpened`, `WzClosing`,
  `MainStyleChanged`. 구독 순서대로 호출되고, `-=`는 **마지막 일치 핸들러**를
  제거한다(`Delegate.Remove`). 없는 핸들러 제거는 무시.
- **무효화**: `Invalidate(rect)`는 대기 영역과 **합집합**, `Invalidate()`는
  전체 무효화로 부분 영역을 흡수한다. `Update()`가 대기 영역을 **한 번의
  페인트**로 소비하며, 무효 영역이 없으면 Paint를 발생시키지 않는다.
  `Refresh()` = Invalidate + Update.

검증:
- C# 하네스가 **실제 .NET 멀티캐스트 델리게이트**와
  `System.Drawing.Rectangle.Union`으로 같은 시나리오를 실행
- 구독 7건(중복 포함)/해제 3건/발생 5건, 무효화 10단계
- `SelectedNode1Changed`는 `[1,2,1]` → `-=1` 후 `[1,2]`
- `MainStyleChanged` `[5,5]` → `-=5` 후 `[5]` (마지막만 제거)
- Paint: `(0,0,15,15)`, `Whole`, `Whole` (전체 무효화가 이후 부분 무효화를 흡수)
- `crates/wz-db/tests/plugin_model_parity.rs` 완전 일치, 단위 테스트 5종

fixture: `plugin_model_input.json`, `plugin_model.json`,
C#: `-- pluginmodel <fixture.json>`.

남는 것: 실제 컨트롤의 WM_PAINT 큐/스레드 선점과 `BeginInvoke` 순서는
WinForms 메시지 루프 의존이라 모델링하지 않았다.

## 프레임 텍스처 베이킹 (`ApplyFlip` / `ApplyRotation`)

원본은 `RenderTarget2D` + `SpriteBatch.Draw`로 텍스처를 구워 origin을 바꾼다.
GPU 없이 **역방향 최근접 샘플링**으로 같은 결과를 만든다
(`wz-avatar::frame_bake`).

- **flip**: 같은 크기에서 픽셀을 좌우/상하 반전하고
  `origin = flipped_origin(w, h, origin, fx, fy)`
- **rotation**: `((angle+360)%360)` 정규화(0이면 항등),
  `rotation_bound`(f64 삼각함수, `MathHelper.ToRadians` float 상수)로
  새 크기/origin, `q = dest - newOrigin`, `v = R(-θ)q`,
  `src = Math.Round(origin + v)`(은행가 반올림)

검증:
- 입력은 실제 `Data/Character` 캔버스 6개의 픽셀/origin
- 케이스: flip 4조합 × 6 + 회전 5각도(30/45/90/180/270) × 6 = **54개**
- C# 참조 구현과 `width/height/origin` 및 픽셀 **SHA-256 완전 일치**
- `crates/wz-avatar/tests/frame_bake_parity.rs` 1종 + 단위 테스트 4종
  (예: 44×43 → 90°에서 60×60, origin `[16,16] → [28,22]`)

fixture: `frame_bake_input.json`, `frame_bake_expected.json`,
C#: `-- framebake <fixture.json>`.

한계: XNA `SpriteBatch` 기본 샘플러는 **선형**이지만 결정적 비교를 위해
최근접으로 고정했다. 실제 GPU 베이킹(선형/감마)은 wgpu 오프스크린 경로로는
가능하지만 C# `GraphicsDevice` 실행이 macOS에서 불가해 대조하지 못했다.

## 메인 윈도우 탭/명령 수명주기

WinForms/DevComponents 컨트롤 없이 관찰 가능한 상태 전이만 옮겼다
(`wz-db::main_window`).

- **탭**: `AddTab`은 끝에 추가하고 선택을 바꾸지 않는다. 첫 탭은 자동 선택
  (컨트롤 전제). `select(index)`는 범위 밖이면 상태를 바꾸지 않는다.
- **명령**: 등록 순서가 표시/디스패치 순서. `requires_selection` 명령은 선택
  노드 유무에 따라 `Enabled`가 재계산되고, 비활성/미등록 디스패치는 실행되지
  않는다(`MainForm.cs:1388`의 버튼 토글 패턴).
- **OnLoad**: `loadedPlugins` 순서대로 호출하고, 예외는 플러그인별로 기록한 뒤
  다음 플러그인으로 계속한다(`PluginManager.PluginOnLoad`).

검증:
- C# 하네스가 **실제 `PluginContext` + fixture 플러그인 어셈블리**를 로드해
  활성화 순서 `[AlphaPlugin, DerivedPlugin]`을 얻음 (기존
  `plugin_entries_activated.json`과 동일)
- 탭 3개/선택 3회(범위 밖 포함), 명령 3개/선택 이벤트 3회/디스패치 4회,
  OnLoad 3건(중간 실패 포함)이 완전 일치
- `crates/wz-db/tests/main_window_parity.rs` 1종 + 단위 테스트 3종

fixture: `main_window_input.json`, `main_window.json`,
C#: `-- mainwindow <PluginBase.dll> <plugin.dll> <fixture.json>`.

가정: 첫 탭 자동 선택과 부분 선택 해제 시의 재선택은 컨트롤 없이 확인할 수
없어 모델에서 명시한 규칙을 따른다(문서화).

## LuaConsole 콘솔 패널 (실행 이력/입력/Run)

원본 `FrmConsole`은 WinForms 콘솔이다. egui 패널로 출력 이력/입력/Run을
재현하고, 실행은 **검증된 `wz_lua::Sandbox`**에 위임한다
(`wz-gui::panels::lua_console`).

- `ExecutionOutcome::{Values, Error}` — 청크 반환값(문자열화) 또는 오류
- `ConsoleModel::submit` — 명령 이력 + 반환값별 출력(없으면 `(반환값 없음)`),
  오류는 오류 항목, 제출 후 입력 비움. 빈 입력은 무시
- `thread_local` 샌드박스로 프레임 간 전역 상태 유지
- 반환값/오류/명령/시스템 항목을 모노스페이스 + 색으로 렌더

검증(GUI 73개 테스트):
- `return 1+2, 'lua'` → `["3","lua"]`
- `error('boom')` → 오류 메시지에 `boom`
- 이력 순서(Command→Output)/입력 초기화/빈 입력 무시/오류 기록
- `host_call`이 콘솔 샌드박스에서도 동작
- 스모크: `SMOKE OK: lua console values=2 [3, lua]`

남는 것: 원본 콘솔의 `print` 리다이렉트, 명령 히스토리(↑/↓), 자동완성,
`DoStringAsync` 취소 훅은 UI 의존이라 미구현이다.
