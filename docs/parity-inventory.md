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
| `LifeItem` + `BehaviorController` 이동/전투 | `crates/wz-gui/src/life.rs` | 🟡 | 순찰/hit/die 검증, 공격/추적 AI는 단순화 |
| `SkyWhaleItem` | `map.rs` + GUI overlay | ✅ | 합성 파서 테스트 |
| `IlluminantClusterItem` 시작/끝 스프라이트 | `wz-gui/src/illuminant.rs` | ✅ | sellas cluster 0/1 실데이터 |
| `ToolTip` 맵 오브젝트 | `map.rs` + `String/ToolTipHelp.img` | ✅ | 실제 000050000 |
| MiniMap canvas/icons/names | `map.rs` + GUI panel | ✅ | 2313x1592, 아이콘 4종, String 이름 |
| `WorldMap` BaseImg/MapList | `wz-gui/src/worldmap.rs` | ✅ | 실제 WorldMap000 7 spots |
| WorldMap `QuestLimit`/`Fog` | `worldmap.rs` + GUI | 🟡 | 합성 픽스처만 (클라이언트 노드 없음) |

## 4. MapRender 렌더링

| C# | Rust | 상태 | 검증 |
|---|---|---|---|
| `MsSpriteRenderer` / `SpriteBatch` | `crates/wz-render/src/batch.rs` | ✅ | batch tests |
| `LightRenderer.DrawSpotLight` | `wz-render/src/light.rs` | ✅ | 36분할 메시 테스트 |
| `waterBack`/`waterFront` 파이프라인 | `wz-render/src/device.rs`, `water.rs` | ✅ | 실제 993258900 텍스처 + 픽셀 리드백 |
| waterBack→scene→waterFront 합성 | `wz-gui/src/compose.rs` | ✅ | `back_is_behind_scene...`, 993258900 픽셀 회귀 |
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

- 🟡 Life 공격/추적 AI: patrol/hit/die는 구현, 원본 `BehaviorController`의
  공격/추적/비행 전체 상태는 단순화.
- 🟡 WorldMap QuestLimit/Fog: 파서+UI+합성 테스트만. 이 클라이언트
  `WorldMap_000.wz` 85개 이미지에 해당 노드가 없음.
- 🟡 Weather: 원본 MapRender/클라이언트 데이터에 weather 노드·자산이 없음.
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
