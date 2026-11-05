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
  - `mirrorChair` → `avatarPosY`/`avatarInvisibleWeapon` parsed; weapon hidden.
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
  - `outfitVoteChair` → the GUI `Score` value is rendered at `scoreTextPos` with
    the built-in font; the avatar layer/button rectangles are parsed.
  - `bgmSelectionChair` → button rectangle and taming are loaded; BGM selection
    itself is metadata-only (no BGM list/value in the chair WZ).
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
