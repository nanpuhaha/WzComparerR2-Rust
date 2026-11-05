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
