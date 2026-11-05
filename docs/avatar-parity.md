# Avatar anchor / prism / zmap parity

Status: resolved 2026-11-05; body/head/equipment/weapon frame assembly, GUI prism
controls, hair/face mix color, custom-origin parsing, taming, and a first-frame effect
path implemented. This note records the C# sources, the Rust implementation, and the
remaining CharaSim gaps.

## C# sources (pinned `bb98c25…`)

- `WzComparerR2/AvatarCommon/AvatarCanvas.cs`
  - `LoadZ()` / `LoadZ(Wz_Node)` reads the child names of `Base\zmap.img` in order.
  - `AppendBone(root, parentBone, skin, mapName, mapOrigin)` creates the bone chain.
    `mapName` comes from the canvas mini-property `map` (`navel`, `neck`, `brow`, …);
    `mapOrigin` is that entry's vector.
  - `BitmapOrigin.CreateFromNode` reads the canvas `origin` vector.
  - Prism is applied per-part via `PrismDataCollection` + `Prism.Apply`.
- `WzComparerR2/AvatarCommon/Prism.cs` — HSV v2 transform, ported to
  `crates/wz-avatar/src/prism.rs`.
- `WzComparerR2/AvatarCommon/AvatarPart.cs` — `PrismData` is **caller/UI supplied**,
  not read from WZ.

## Current Rust state

- `crates/wz-avatar/src/frame_layers.rs::collect_frame_assembly` is the single
  implementation for frame assembly:
  - reads `origin`, `z`, `map`, `_outlink` from each canvas mini-property;
  - builds the multi-entry `map` bone chain (including joint bones);
  - returns `FrameAssembly { layers, move_vec, root_bone }`.
  `collect_frame_layers` remains as a thin wrapper for callers that only need `layers`.
- `crates/wz-avatar/src/loader.rs::load_frame_parts` calls `collect_frame_assembly`
  for both the body frame (`0000{skin}00.img`, `stand1`/`alert`) and the head frame
  (`00012000.img`, `front`/`back`). The loader no longer re-implements
  `origin`/`map`/bone-chain logic.
- `load_item_frame` / `load_item_canvas` assemble equipment and weapon item images
  (`default`/`stand1`/`walk1`) through the same `collect_frame_assembly` path.
  `merge_bone_tree` merges the resulting bone tree into the avatar root.
- `AvatarPart::apply_prism(&PrismData)` exposes the C# `Prism.Apply` path. Prism
  settings are caller-supplied, matching the C# UI; WZ does not store them.
- The GUI avatar panel exposes prism controls (enabled, color type, hue, saturation,
  brightness, convert pure black) and re-renders the selected parts with the prism
  applied; the original loaded RGBA is preserved.
- `zmap` order is read from `Base/Base_000.wz::zmap.img` and falls back to
  `STANDARD_ZMAP`.
- Mix color: `mix_rgba8` ports C# `AvatarCanvas.MixBitmaps` (4-bit channel
  quantization). `MixOptions` applies Hair/Face variants through `MixNodes`; the GUI
  exposes hair/face color and opacity.
- Custom origin: `AvatarPart.custom_origin_map` is parsed from `info/customOrigin`
  (WZ child name → custom origin ID). Applying a selected origin map is caller-driven,
  matching C# `AvatarCanvas.CustomOrigin`.
- Taming: the `Taming` slot loads `Character/TamingMob` through `load_item_frame`.
- Effect: the `Effect` slot loads all frames, delays, **origins and z indices** of
  `ItemEff.img/<id>/effect/default/<n>`. `LoadedAvatar::apply_effect_frame(index)`
  updates the effect bone skin for scene renderers, and the GUI applies the current
  frame's origin to the effect quad (`state.effect_origin`) while stepping frames
  manually or auto-playing with `tick_effect` using the WZ delays.
- Chair: `ChairInfo` captures `info/sitAction`, `removeBody`, `fixFrameIdx`/
  `fixActionFrame`, `sitEmotion`, `tamingMob`, `customChair`, and group taming IDs/
  `bodyRelMove`. Group taming members are loaded from `Character/TamingMob` (via their
  `sit` action) and merged with their `bodyRelMove` offsets. Real sample: chair
  `03016200` (`03016.img`) with taming mobs `01983279`..`01983282`.
- C# `CustomChairType` has **only** `Unsupported` and `ScaleAvatarChair`. The other
  WZ type strings (`flipXYChair`, `dummyAvatarChair`, `rotatedSleepingBagChair`, …)
  are **not implemented by the C# avatar renderer**; they fall through to
  `Unsupported`. `CustomChairType::Other` preserves the raw type, and
  `CustomChairInfo` parses `flipXYChairInfo`, `rotatedSleepingBagChairInfo`, and
  `dummyAvatarChair` data plus flattened unknown fields in `extra`.
- C# chair behavior ported here:
  - `sitAction` wins; `hide`/`hideBody` and `removeBody` hide body+head.
  - `ScaleAvatarChair` uses `scaleAvatar/scale`; other types use 100.
    `info/scale` is **not** a chair scale — it belongs to taming mobs.
  - `fixFrameIdx`/`fixActionFrame` selects the body frame.
  - `info/tamingMob` and `customChair/self/tamingMob` add a `TamingMob` part;
    the chair image stays (C# adds `ChairPart` and `TamingPart` together).
  - `customChair/self` and `customChair/avatar/*` are parsed for future rendering.
- WZ-only visual behavior (not present in C# source) is implemented as an explicit
  extension:
  - `flipXYChair` → horizontal/vertical mirror, `flipCharacterPos` offset, weapon hide.
  - `rotatedSleepingBagChair` → 90° rotation, `avatarPos` offset, weapon hide.
  - `dummyAvatarChair` → `customChair/avatar/*` taming mobs at their `pos`/`z`
    (with `left` mirroring), plus the `self` taming.
  - `rollingHouseChair` → every `part/N` frame's `part0..part7` layers, resolved
    through metadata `_outlink` into `_Canvas`, with `partZ` z-order and `posLv/lv1`
    avatar offset. All frames and delays are loaded; the GUI exposes a frame slider
    and `Rolling house 자동 재생` (WZ delay driven).
  - `androidChair` → `androidChairInfo/customEffect/0..15` outlink frames,
    `pos`/`reverseFlip`/`z`, `forcedHideBody`/`forcedAction`; played through the
    same chair frame slider/auto-play (fixed 100ms because WZ has no delay).
  - `petChair` → `petChairInfo` slots (`pos`, `reverseFlip`, `wonderGrade`,
    `tGapMin/Max`, `randAct/*/prob`) are parsed. Up to three pet item IDs supplied
    by the GUI (`Pet items`) load every frame of `stand0`/`stand1`/`move`/`rest0`
    and the `randAct` actions from `Item/Pet/_Canvas`, placed at the slot positions
    with `reverseFlip` mirroring. `Pet 자동 재생` advances frames by `delay` and
    switches actions using the WZ weights and `tGapMin/Max`.
  - Pet equipment: `Character/PetEquip/_Canvas/<equip>.img/<petId>/<action>/<frame>`
    is loaded for the matching pet action/frame and overlaid on the pet. The GUI
    supplies one equip ID per pet (`Pet equips`).
  - `eventPointChair` → `index`/`point`/`term` are parsed and shown in the GUI
    (`Chair type`, `Event point`). Rendering uses the taming part; the chair body
    is hidden (`removeBody`).
  - `hashTagChair` → `action/*` list parsed; the first action
    (`forceCharacterAction` + frame) drives the body pose.
  - `mirrorChair` → `avatarPosY`/`avatarInvisibleWeapon` parsed; weapon hidden and
  body/equipment parts are reflected across `avatarPosY` (`mirror_*` parts).
  - `2019rollingHouseChair` / `yetiChair` → generic `part/N/part0..` animation
    frames are loaded through the same chair frame slider/auto-play.
  - `identityChair` → the player level is rendered from `identityChairInfo/number`
    digit sprites at `numPos` (GUI `Level` input). Name/job/gender text still needs
    a text renderer.
  - `scoreChair` → the `scoreEffect` group whose `[min,max]` contains the GUI
    `Score` input is selected and its effect frames play through the chair
    animation path.
  - `popChair` / `popButtonChair` → the GUI `Pop` value is rendered with a small
    built-in 5x7 bitmap font at `textPos`/`popPos` (`popMin`/`popMax` clamped).
  - `timeChair` → the GUI `Time` minutes value is rendered as `HH:MM` at `textPos`
    (clamped by `timeMax`).
  - `outfitVoteChair` → the GUI `Vote` value (fallback `Score`) is rendered at
    `scoreTextPos` with the built-in font; the avatar layer/button rectangles are parsed.
  - `bgmSelectionChair` → button rectangle and taming are loaded; a caller-supplied
    BGM string is rendered at the button origin. The BGM list/audio is not in the
    chair WZ.
  - `showcaseChair` → `pos/0..3` and `slotSet` are parsed; up to four equipment
    IDs supplied by the GUI (`Showcase items`) are loaded from `Character/<slot>`
    and placed at those positions. `item_id_to_slot` now handles both 7-digit
    (`1000000`) and 8-digit equipment IDs.
  - Text: `Etc/*.wz::Font.img/NotoSansMP-Medium` (TTF raw data) is loaded with
    `fontdue` and rasterized for `identityChair` name/job/gender and
    `popButtonChair` name/guild. The GUI supplies the strings. The font is read
    from the user's Data directory, not bundled.
  - `Item/Install` category lookup accepts both 5-digit (`03018.img`) and 6-digit
    (`030156.img`) image names.
  - Every other `customChair` type exposes its flattened `customChair fields`
    (type + nested key/value) in the GUI; remaining gameplay-only values (name,
    BGM, vote, time, pop counts, etc.) are not simulated.
  - `AvatarPart` now carries final `position`, `origin`, and `z_index`; the GUI
    centers/scales the assembled avatar and draws parts at those coordinates.
- Custom origin: the two-step C# semantics are implemented:
  `CustomOriginMap` (originValue → childName) + `CustomOrigin` (childName → Point)
  → `apply_custom_origin_map`. The GUI accepts `childName:x,y;...`.
- Chair pose: when a `Chair` is in the equipment list, the body frame prefers `sit`
  over `stand1`/`stand`. Group taming members also use `sit`.
- Map reactors: `MapData.reactors` parses `reactor/<n>` (`id` string/int, x/y/f/name/
  reactorTime); `oracle map-scene` and the GUI viewport load/render
  `Reactor/<id>.img/0/0` first frame with origin/flip.
- Map backgrounds: all `back/<n>` entries are loaded; non-front at z=-1000 and
  front at z=1000 (static first frame, no parallax/animation yet).
- Map light/particle/effect: `MapData.light` parses `MapLight`/`Light2D`
  (`mode`, ambient/directional/back color, luminance limit, per-light type/x/y/color/
  radii/angles). The viewport draws the same 36-segment additive spot-light mesh as
  `LightRenderer.DrawSpotLight`. `MapData.particles` parses `ParticleItem` nodes and
  `wz-db::particle` parses `Effect/particle.img` descriptors; the viewport loads the
  particle texture via `_outlink` and samples position/size/color/alpha/rotation like
  `ParticleSystem.Draw`. `MapData.map_events` parses `effect/<n>` (`type`,
  `defaultAnimation`, `changedAnimation`, `tags`).
- Equipment slots: `FaceAccessory`, `EyeAccessory`, `Earrings`, `Pendant`, `Belt`,
  `ShoulderPad`, `Pocket`, `Emblem` load from `Character/Accessory`; `Saddle` loads
  from `Character/TamingMob`; `CashWeapon` maps to `Weapon`. Icon-only images are
  skipped. Ring effect frames load from `Effect/ItemEff.img/<ringId>/0|1`.
  Android items resolve `info/android` and assemble `Etc/Android/<id>.img`
  `basic`/`costume` into a full avatar.
- `wz_avatar::compute_character_stats` aggregates equipment `info` props, set-item
  thresholds (`Etc/SetItemInfo.img`), caller-supplied potential options
  (`Item/ItemOption.img`), and max-star display (`C# GetMaxStar`). Star-force stat
  enhancement values are not in WZ; C# also only displays the star count.
- Body action: `MixOptions.body_action`/`body_frame` select a body pose
  (`walk1`, `jump`, `alert`, `prone`, `ladder`, `rope`, ...). GUI preloads all body
  frames for the selected action and exposes `Action frame` + `Body action 자동
  재생` (120ms). Equipment/head remain static during body playback.
- `info/sitEmotion` selects the head/face emotion action (C# `EmotionTable`);
  taming `characterEmotion` overrides it.
- taming `info/flip` mirrors body/equipment parts **and** the body/equipment bone
  subtree (chair/taming/effect/pet bones are excluded). taming `characterAction`/
  `characterEmotion` override the body pose/emotion for the `sit`/`stand0` key.
- chair `effect2` and `Effect/ItemEff.img/<chairId>/0|1` all frames are loaded
  (metadata `_outlink` → `Effect/_Canvas`) and played through the chair frame
  slider/auto-play. The static chair base stays; effect layers are replaced per frame.
- Current-client parity for the C#-implemented chair behavior: complete. The remaining
  gap is data-driven behavior for WZ-only custom chair types, which has no C# reference
  implementation.

## Verification

`WZ_DATA_ROOT=<Data> cargo test -p wz-avatar` passes all 18 real-data tests, including
body/head loading, equipment loading, zmap, and the prism unit test.

## Patch

- `wz-core::patch` now parses the full decompressed payload, the KMST1125
  leading file-hash table, create/modify/delete records and each modify
  command stream (`CalcNewFileLength`).
- `wz patch <file> [--info] [--old DIR --out DIR]` can inspect records or apply
  them. Create/modify output checksums and old-file checksums are verified.
- Verified with the real `00419to00420.patch`: 44 create (43 files + 1 dir),
  415 modify, 1 delete, 1024+ hash entries; all 43 created files match the
  reference extraction SHA-256 and every modify output length sums to
  42,701,118,706 bytes.

## Sound

- `wz-core::sound` decodes `Sound_DX8` `WAVEFORMATEX` (plain / KMS / GMS),
  classifies PCM/MP3/binary, builds RIFF headers for PCM and extracts raw MP3.
- `wz sound <wz> <img/name> [--out file] [--play]` inspects, extracts and plays.
- GUI: map `info/bgm` playback and arbitrary Sound path playback with stop.
- Verified with `Sound_000.wz/Bgm00.img/DragonDream` (1,261,192-byte MP3,
  44.1k/22.05k stereo) and `AchievementEff.img/GradeUp` (afplay exit 0).

## Map backgrounds (animation / parallax / tiling)

- Background layers now load `Map/Back/<bS>.img/<back|ani>/<no>` instead of the
  first image canvas, matching `BackItem.LoadFromNode` + `PreloadResource`.
- All frames and `delay` values are decoded; animation follows the same
  accumulated-delay selection as the original frame animator.
- Viewport position uses the original `GetMeshBack` rules: camera ratio
  `camera * (100+rx)/100`, time scroll for tile modes 4-7, `cx/cy` tiling
  (with width/height fallback for both-tile), flip UV and `a` alpha.
- Real `000010000.img` test: 14 backgrounds, tile modes 1/4, `cx=1200/2000`,
  non-empty `no` values load the correct 900x11 / 900x584 etc. frames.

## Reactors (stage / hit)

- `wz-gui::reactor` loads every numeric stage and `hit` animation from
  `Reactor/<id>.img`, resolving `info/link` to the linked reactor.
- Each frame keeps its canvas size, RGBA and origin; frame delays follow the
  same accumulated-delay selection as the original animator.
- Clicking a visible reactor applies `NextStage = Stage + 1`: the current
  stage's `hit` animation plays once, then the next stage becomes current
  (`ReactorItem.Controller`). Hidden reactors (stage without frames) are not
  hit-tested, matching a null render object.
- Click also resolves `Sound/Reactor.img/<id>/<stage>`; UOL nodes such as
  `../../1002009/0/Hit` are followed before decoding.
- Real-data tests: `1002009` hit frames (64x45, 86x56), `100000` stage 0
  (29x35), `Reactor.img/1209001/0` UOL sound decode.

## Map life & minimap

- `MapData.life` now carries the map placement index. `wz-gui::life` loads
  `Mob/<id>.img` / `Npc/<id>.img` `stand` (fallback `default`/`move`) frames,
  follows `info/link`, and keeps canvas origin/flip/hide/delay.
- Canvas `_outlink` values (for example
  `Npc/_Canvas/1541000.img/stand/0`) are resolved through the corresponding
  `_Canvas` archive; the 1x1 placeholder canvas is not used.
- The viewport animates life by accumulated frame delay, draws at
  `(x-origin, y-origin)` with flip, skips `hide`, and draws NPC/mob names
  above the head using StringLinker (`info/hideName` honored).
- `MapData.mini_map` parses `miniMap` width/height/center/mag; the GUI loads
  the `miniMap/canvas` (including `_outlink`) and displays it with a camera
  marker.
- Real tests: NPC `1541000` (48x75 stand) + hidden `1541067`, mob `100100`,
  `000010000` minimap `2313x1592 center=(1338,635) mag=4`, canvas pixels.

## Portals & minimap icons

- `MapData.portals` now keeps `image`, impact/range and `shownAtMinimap` /
  `enchantPortal`.
- `wz-gui::portal` mirrors `PortalItem.PortalTypes` and loads
  `MapHelper.img/portal/game/<type>/<image|default>` (fallback to the type node),
  preferring `portalStart` frames, with origin/delay animation.
- The viewport draws portal frames with animation time and world origin.
- Minimap icons load `MapHelper.img/minimap/{portal,npc,mob,hiddenportal}`
  canvases and draw portals (types 2/7/10, and 8/11 when `shownAtMinimap`) plus
  visible NPC/mob markers over the minimap canvas.
- Real test: map `000010000` portal type 2 (`pv`) loads its frames; all four
  minimap icons decode to non-empty RGBA.

## Life AI & skill preview

- `MapData.life` now parses `rx0`/`rx1`/`mobTime`. Mob `LifeView`s patrol
  `rx0..rx1` at a simplified 60 px/s, flip on direction change, and interpolate
  Y on their `fh` foothold (`BehaviorController` movement subset).
- `stand` and `move` animations are both loaded; `move`/`walk1` is used while
  moving and falls back to `stand`.
- The viewport ticks life AI every frame and minimap NPC/mob markers follow the
  moved positions.
- `wz-gui::skill` mirrors `SummonSkill` path rules
  (`Skill/<id/10000 or id/100>.img/skill/<id>/<ani>`); the inspector has a
  skill preview (ID, ani, x/y, flip) that renders the animation in the viewport.
- Real tests: mob `100100` move frames + real `000020000` foothold patrol,
  skill `1001005/effect` (11 frames, first 144x128).

## Life combat / death & minimap names

- `LifeView` now loads `hit1`/`die1` frames and keeps
  `Alive → Hit → Alive` / `Alive → Dying → Dead` state (`BehaviorController`
  subset). Hit/Dying stop patrol movement; Dead keeps the last die frame.
- Clicking a visible mob applies 25 damage; four hits trigger death. NPCs ignore
  clicks. The same click is also used for reactor stage transitions only when no
  mob was hit.
- `StringLinker::map_display_names(id)` returns `streetName` / `mapName`; the
  minimap panel shows region and map name from String.wz.
- Real tests: mob `100100` has stand/move/hit/die frames; `000020000` foothold
  patrol; `map_display_names` finds named maps in `String_000.wz`.

## World map, map tooltips, sky whale / illuminant cluster

- `MapData` parses `ToolTip/<n>` (+ `<n>char`), `skyWhale/<n>`
  (start/end/width/speed/vrate/applyTerm) and `illuminantCluster/<n>`
  (start/end/size/speed/particle/startPoint/endPoint).
- Map ID now falls back to the image name in the GUI/oracle paths, which the
  `ToolTipHelp.img/Mapobject/<mapId>/<n>` lookup needs.
- `ToolTipHelp.img` `Title`/`Desc` are loaded for map tooltips; hovering a
  tooltip rectangle paints the title/desc at the pointer.
- `wz-gui::worldmap` lists `Map/WorldMap/*.wz` images, loads
  `info/WorldMap`, `BaseImg/0` (+origin/spot) and `MapList` spots; the inspector
  world-map panel shows the base image, spots, titles and map numbers.
- Sky whale and illuminant cluster paths are drawn as world-space overlays with
  endpoints/labels (matching the original fly-layer editor visualization).
- Real tests: map `000050000` tooltip rect + String title; `WorldMap000` base
  image + 7 spots + map numbers; synthetic sky whale/illuminant parse test.

## World-map quest/fog & camera movement

- `WorldMapView` parses `QuestLimit/default` + numeric stages (`0` base image,
  `openMap`, `closeMap`, `quest`, `state`, `default`) and `Fog/<n>`
  (`0` image, `quest`, `qState`).
- The world-map panel has Quest/Fog selectors, draws the selected quest base
  image over the default base, and overlays the selected fog image with alpha.
- Map camera now supports the original keyboard movement model: arrow keys or
  WASD accumulate velocity, `Ctrl` applies 3x boost, and velocity decays by
  0.92 per frame (`camera_input::step`, tested).
- This client's `WorldMap_000.wz` contains no `QuestLimit`/`Fog` nodes (checked
  across all 85 images), so those selectors are data-driven and covered by the
  parser/UI path; base-image + 7 spots are verified against real data.

## Illuminant cluster sprites

- `wz-gui::illuminant` loads the original start/end animations from
  `Map/Obj/sellas.img/fieldGimmick/cluster/<startPoint*2>` and
  `<endPoint*2+1>`.
- Frames keep RGBA/origin/delay; the viewport draws them at
  `(start + size/2, start - size/2)` and the matching end point, animating by
  accumulated delay. The existing path overlay remains for the fly layer.
- Real test: `sellas.img` cluster `0`/`1` frames decode with non-empty RGBA.
- This client has no `water`/`weather` map nodes and no MsCustomSprite shader
  data in `Map0`; the waterBack/waterFront WGSL pipelines remain unused until a
  map carrying those custom sprites is available.

## Water back/front sprites

- Found the real fixture `Map/Back/Back_000.wz/2024BlueholeExploration.img`:
  `back/14` uses `shader/waterBack`, `back/15` uses `shader/waterFront`;
  map `993258900` (`Map9_001.wz`) places both (15 as front).
- `wz-gui::water` loads the sprite texture groups (numeric frame → texture
  index → `_outlink` canvas), keeps delays/background parallax/alpha, and
  renders `waterBack`/`waterFront` with a time-based water tint
  (`WaterKind::Back` static, `Front` animated).
- Normal background loading skips shader/sprite backgrounds; the worker routes
  them to `map_waters` instead.
- Real test: `loads_real_water_back_front_layers` verifies both kinds, decoded
  texture dimensions > 1, and front tint animation.
- A synthetic `WorldMap` fixture test covers QuestLimit open/close maps,
  quest/state/default, Fog quest/qState and MapList mapNo.

## Water shader pipelines

- `wz-render` now creates real `waterBack` (group1 = 176 B uniform) and
  `waterFront` (uniform + 3 textures + 3 samplers) pipelines with
  `fs_water_back` / `fs_water_front`.
- `WaterBackParams` / `WaterFrontParams` write the exact WGSL byte layouts
  (176 / 352 B) including vp/vp_inv/resolution_time and water constants.
- `RenderContext::render_water_to_rgba` renders a world-space water quad to an
  offscreen RGBA target using the shader and reads it back; the GUI composites
  the result over the viewport.
- Real map `993258900` waterBack (back/14) and waterFront (back/15) textures are
  uploaded and rendered through the shader. Tests assert non-transparent pixel
  output for both synthetic and real textures.
- Weather: the client/MapData has no `weather` node or weather assets; original
  MapRender only has the three custom shaders (light/waterBack/waterFront).

## Water ordering & parity inventory

- `wz-gui::compose` fixes the layer order explicitly:
  `waterBack → scene → waterFront` (alpha-over). Opaque scene pixels hide
  waterBack; front water remains over the scene.
- Pixel regression for real map `993258900` renders waterBack (no texture) and
  waterFront (3 real textures) through the WGSL pipelines, composes them, and
  asserts back/front ordering.
- `docs/parity-inventory.md` is the explicit C#→Rust feature table with status
  and verification evidence, including data-absent items.

## Life attack/chase/fly & waterFront capture

- `LifeView` adds `Attack`, `Chase`, `Fly` states. Attack animations are all
  `attack*`/`skill*` children; `fly`/`chase` animations are loaded when present.
  Speed comes from mob `info.speed/chaseSpeed/flySpeed`. Attack triggers within
  260 px of the camera target and recovers after the animation.
- Real mob `9833307` (Mob/_Canvas_087) has `fly`, `attack1`, `hit1`, `die1`;
  loader test asserts attack/fly frames. Synthetic tests cover attack recovery,
  chase movement and vertical fly.
- waterFront now binds the current scene RGBA as `bg_tex` (plus the two WZ
  textures) for the shader's background capture.

## Water constants & WorldMap absence evidence

- Water shader constants are parsed from
  `2024BlueholeExploration.img/shader/waterBack/constant` and
  `.../waterFront/constant` (min/max 4500/19500, color0..9/level0..9,
  aberration 0.005, godray 4754..5248, diffuse 1,1,1). No defaults are used
  when the real constants exist.
- `real_worldmap_has_no_quest_or_fog_nodes` scans every image in
  `Map/WorldMap` (85+) and asserts `QuestLimit = 0`, `Fog = 0`; the parser/UI
  remains covered by the synthetic fixture test.

## C# comparison fixtures

- `crates/wz-db/tests/fixtures/map_scene_000010000.expected.json` is compared
  field-for-field with Rust `MapData` (`map_scene_matches_csharp_expected_fixture`).
- `crates/wz-gui/tests/fixtures/worldmap_quest_fog.json` is parsed as a WZ node
  tree; the resulting Quest/Fog/spot JSON and GUI selector labels are compared
  with `worldmap_quest_fog.expected.json`
  (`worldmap_quest_fog_fixture_matches_uiworldmap_behavior`).
- The client's 85+ WorldMap images contain no QuestLimit/Fog nodes, fixed by
  `real_worldmap_has_no_quest_or_fog_nodes`.

## C# WzLib execution comparison

`tools/csharp-worldmap` runs the original C# WzLib parser. On the same input:
- WorldMap fixture → C# JSON matches the Rust expected fixture exactly.
- `WorldMap_000.wz` → C# `images=85, quest=0, fog=0`, matching Rust's
  `real_worldmap_has_no_quest_or_fog_nodes`.
- `Map0_000.wz/000010000.img` → C# 4721 nodes vs Rust 4722 (one virtual/UOL
  wrapper difference).

## Full C# coverage audit

`docs/csharp-coverage.md` enumerates every original project/directory with file
counts, LOC and status. Remaining large unmapped areas are the WinForms main UI,
CharaSimControl, DB2, Common Controls/OpenAPI, LuaConsole, Network and
PluginBase. `wz-diff::virtual_node` implements the original `WzVirtualNode`
merge semantics (recursive directory merge, conflict errors).

## DB2 Item grid

- `wz-db::db2_items` mirrors `DB2Form.LoadItem`: each numeric child of
  `Item/<dir>/<img>.img` becomes `id/name/desc/has_icon`, with name/desc from
  the original `StringLinker` (`String/<dir>.img`).
- `wz db2-items <item.wz> <image> --string <String_000.wz>` prints the grid;
  the GUI Catalog has a DB2 Item grid table.
- Executed C# reference: `tools/csharp-worldmap items ...` loads
  `WzComparerR2.Common.StringLinker` and prints the same rows.
  `Consume_000.wz/0200.img` → C# 530 == Rust 530 rows, exact match.

## DB2 Map grid

- `wz-db::db2_maps` mirrors `DB2Form.LoadMap`: each numeric `MapX/<id>.img`
  becomes `id/street_name/map_name/has_minimap/link`, with names from
  `StringLinker.StringMap` and `info/link` accepting string-encoded IDs
  (`"001130010"`).
- `wz db2-maps <map.wz> --string <String_000.wz>` prints the grid; the GUI
  Catalog has a DB2 Map grid table.
- Executed C# reference: `tools/csharp-worldmap maps ...` uses the original
  `StringLinker`. `Map0_000.wz` → C# 87 == Rust 87 rows, exact match.

## DB2 Mob grid

- `wz-db::db2_mobs` mirrors `DB2Form.LoadMob`: iterate `StringLinker.StringMob`,
  keep IDs whose `<id>.img` exists in the given Mob WZ, and emit
  `id/name/has_stand/has_fly/has_move/has_hit/has_die/has_thumbnail/link`.
- Icon source follows the original `stand/0` → `fly/0` preference (flags expose
  which source exists); full stat columns are empty because this client's split
  `Mob/_Canvas` images mostly lack the `info` stat block.
- `wz db2-mobs <mob.wz> --string <String_000.wz>` prints the grid; the GUI
  Catalog has a DB2 Mob grid table.
- Executed C# reference: `tools/csharp-worldmap mobs ...` uses the original
  `StringLinker`. `_Canvas_011.wz` → C# 55 == Rust 55 rows, exact match.

## DB2 Skill grid

- `wz-db::db2_skills` mirrors `DB2Form.LoadSkill`: each numeric `Skill/<book>.img`
  yields a book row, and each `skill/<id>` yields a skill row with
  `id/row_type/book/name/desc/has_icon/has_common/h_count`.
- Book names use `StringLinker.string_skill_str[book].values["bookName"]`;
  skill name/desc use `string_skill[id]`.
- `wz db2-skills <skill.wz> --string <String_000.wz>` prints the grid; the GUI
  Catalog has a DB2 Skill grid table.
- Executed C# reference: `tools/csharp-worldmap skills ...` uses the original
  `StringLinker.StringSkill/StringSkill2`. `_Canvas_000.wz` → C# 165 == Rust
  165 rows, exact match.

## DB2 NPC grid

- `wz-db::db2_npcs` mirrors `DB2Form.LoadNpc`: each numeric `Npc/<id>.img`
  becomes `id/name/has_stand_0/has_default_0/has_link/link`, with names from
  `StringLinker.StringNpc` and icon-source flags matching `stand/0` →
  `default/0` preference.
- `wz db2-npcs <npc.wz> --string <String_000.wz>` prints the grid; the GUI
  Catalog has a DB2 NPC grid table.
- Executed C# reference: `tools/csharp-worldmap npcs ...` uses the original
  `StringLinker.StringNpc`. `_Canvas_000.wz` → C# 943 == Rust 943 rows, exact
  match.

## DB2 Reactor grid

- `wz-db::db2_reactors` mirrors `DB2Form.LoadReactor`: each numeric
  `Reactor/<id>.img` becomes `id/has_icon/has_info/has_link/link`, where the
  icon source is the original `0/0` canvas and the link comes from
  `info/link` (string-encoded IDs accepted).
- `wz db2-reactors <reactor.wz>` prints the grid; the GUI Catalog has a DB2
  Reactor grid table.
- Executed C# reference: `tools/csharp-worldmap reactors ...` uses the original
  WzLib. `_Canvas_000.wz` → C# 441 == Rust 441 rows, exact match.

## DB2 Music grid

- `wz-db::db2_music` mirrors `DB2Form.LoadMusic/AddMusicRows`: for
  `Sound.wz` images starting with `Bgm`/`PL_3`/`PL_B`/`PL_C`/`PL_M`, recursively
  collect every `Sound_DX8` node as `path/data_len/duration_ms`.
- Paths use the original `GetPath()` `/` form, e.g.
  `Sound_000.wz/Bgm00.img/DragonDream`.
- `wz db2-music <sound.wz>` prints the grid; the GUI Catalog has a DB2 Music
  table.
- Executed C# reference: `tools/csharp-worldmap music ...` uses WzLib.
  `Sound_000.wz` → C# 40 == Rust 40 rows, exact match.

## DB2 DamageSkin grid

- `wz-db::db2_damage_skins` mirrors `DB2Form.LoadDamageSkin`: iterate
  `StringLinker.string_item` entries from `Consume.img`, keep names containing
  `스킨`/`Damage Skin`/`字型`/`ジスキン`/`伤害皮肤`, then check
  `Item/Consume/0243.img` and `0263.img` for `info/icon` (0263 wins) and
  `info/sample`.
- `wz db2-damage-skins <consume.wz> --string <String_000.wz>` prints the grid;
  the GUI Catalog has a DB2 DamageSkin table.
- Executed C# reference: `tools/csharp-worldmap damageskins ...` uses the
  original StringLinker. `Consume_000.wz` → C# 952 == Rust 952 rows, exact
  match.

## DB2 Morph grid

- `wz-db::db2_morphs` mirrors `DB2Form.LoadMorph`: each item in
  `Item/Consume/0221.img` yields `id/name/desc/morph_id/has_icon/has_morph`,
  with `spec/morph` left-padded to 4 digits and `Morph/<id>.img/walk/0`
  checked for the morph preview.
- `wz db2-morphs <consume.wz> --morph <morph.wz> --string <String_000.wz>`
  prints the grid; the GUI Catalog has a DB2 Morph table (loads
  `Data/Morph/Morph_000.wz`).
- Executed C# reference: `tools/csharp-worldmap morphs ...`. `Consume_000.wz` +
  `Morph_000.wz` → C# 234 == Rust 234 rows, exact match.

## DB2 TamingMob grid

- `wz-db::db2_taming_mobs` mirrors the `TamingMob` branch of
  `DB2Form.LoadCharacter`: each `Character/TamingMob/<id>.img` becomes
  `id/name/desc/h1/has_icon/req_level/req_job/tuc/islot/taming_mob`, using
  `StringLinker.string_eqp` for the Eqp/Taming name and description.
- This is the comparable core of the original row; the original also packs the
  whole `info` block into a concatenated text column, which is not reproduced
  as one string here.
- `wz db2-taming-mobs <taming.wz> --string <String_000.wz>` prints the grid;
  the GUI Catalog has a DB2 TamingMob table.
- Executed C# reference: `tools/csharp-worldmap tamingmobs ...`.
  `TamingMob_000.wz` → C# 1265 == Rust 1265 rows, exact match.

## DB2 Character equipment grid

- `wz-db::db2_characters` mirrors the general `DB2Form.LoadCharacter` branch:
  each `Character/<Dir>/<id>.img` becomes
  `id/name/desc/h1/has_icon/reqLevel/reqJob/tuc/islot/vslot` plus
  STR/DEX/INT/LUK/PAD/MAD/PDD/MDD, using `StringLinker.string_eqp`.
- `wz db2-characters <character.wz> --string <String_000.wz>` prints the grid;
  the GUI Catalog has a DB2 Character table.
- Executed C# reference: `tools/csharp-worldmap characters ...`.
  `Coat_000.wz` → C# 682 == Rust 682 rows, exact match.

### DB2 Character/Cap

- The generic `wz db2-characters` path also covers the Cap tab. `Cap_000.wz`
  → C# `characters` 3166 == Rust 3166 rows, exact match
  (`id/name/desc/h1/icon/reqLevel/reqJob/tuc/islot/vslot/stat`).

## Skill tooltip summary (SummaryParser)

- `wz-db::skill_summary` ports the resolver-failure path of
  `SummaryParser.GetSkillSummary`: `#c`/`#$g`/`#$x`/`#k`/`#fc`,
  `#[skillId]prop` skill references, unresolved `#prop#` removal, `\n`/`\r`/
  `\c`/`\\` escapes, tab removal and trailing `\r`/`\n` trimming.
- This client has no skill `common`/`level` stat nodes, so property resolution
  always fails, matching the C# harness `NoSkillResolver`.
- `wz skill-summary <String_000.wz> [--limit] [--json]` prints every skill
  `desc` and `h/h1..hn` summary; the GUI inspector shows the summary next to
  the skill description.
- Executed C# reference: `tools/csharp-worldmap skillsummary ...` on
  `String/Skill.img` → 20629 rows, exact match with Rust. A 2000-row fixture
  backs the test.

## ItemTooltip attributes

- `wz-db::item_tooltip` mirrors `ItemTooltipRender2.GetItemAttributeString`: the
  fixed-order only/quest/pquest/tradeBlock/useTradeBlock/accountSharable/
  exchangeableOnce/multiPet/accountSharableAfterExchange/mintable tags, plus
  the 501/502/516 and pet special cases.
- `wz item-tooltip <item.wz> <image> <id> --string <String_000.wz> [--json]`
  prints name/desc/auto_desc/attributes.
- Executed C# reference: `tools/csharp-worldmap itemtooltip ...` uses
  `ItemStringHelper.GetItemPropString` and StringLinker. `02000012` → exact
  match (`교환 불가`).

### DB2 Character/Shoes

- The generic `wz db2-characters` path also covers the Shoes tab.
  `Shoes_000.wz` → C# `characters` 1703 == Rust 1703 rows, exact match
  (`id/name/desc/h1/icon/reqLevel/reqJob/tuc/islot/vslot/stat`).

### DB2 Character/Accessory

- The generic `wz db2-characters` path also covers the Accessory tab.
  `Accessory_000.wz` → C# `characters` 2369 == Rust 2369 rows, exact match
  (`id/name/desc/h1/icon/reqLevel/reqJob/tuc/islot/vslot/stat`).

### DB2 Character/Longcoat

- The generic `wz db2-characters` path also covers the Longcoat tab.
  `Longcoat_000.wz` → C# `characters` 2493 == Rust 2493 rows, exact match
  (`id/name/desc/h1/icon/reqLevel/reqJob/tuc/islot/vslot/stat`).

### DB2 Character/Glove

- The generic `wz db2-characters` path also covers the Glove tab.
  `Glove_000.wz` → C# `characters` 661 == Rust 661 rows, exact match
  (`id/name/desc/h1/icon/reqLevel/reqJob/tuc/islot/vslot/stat`).

### DB2 Character/Pants

- The generic `wz db2-characters` path also covers the Pants tab.
  `Pants_000.wz` → C# `characters` 579 == Rust 579 rows, exact match
  (`id/name/desc/h1/icon/reqLevel/reqJob/tuc/islot/vslot/stat`).

### DB2 Character/Shield

- The generic `wz db2-characters` path also covers the Shield tab.
  `Shield_000.wz` → C# `characters` 142 == Rust 142 rows, exact match
  (`id/name/desc/h1/icon/reqLevel/reqJob/tuc/islot/vslot/stat`).

### DB2 Character/Cape

- The generic `wz db2-characters` path also covers the Cape tab.
  `Cape_000.wz` → C# `characters` 1139 == Rust 1139 rows, exact match
  (`id/name/desc/h1/icon/reqLevel/reqJob/tuc/islot/vslot/stat`).

### DB2 Character/PetEquip

- The generic `wz db2-characters` path also covers the PetEquip tab.
  `PetEquip_000.wz` → C# `characters` 511 == Rust 511 rows, exact match
  (`id/name/desc/h1/icon/reqLevel/reqJob/tuc/islot/vslot/stat`).
