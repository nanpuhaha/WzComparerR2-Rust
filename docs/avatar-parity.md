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
  updates the effect bone skin for scene renderers; the GUI can step frames manually
  or auto-play with `tick_effect` using the WZ delays.
- Chair: `ChairInfo` captures `info/scale`, `customChair`, group taming IDs/`bodyRelMove`.
  Group taming members are loaded from `Character/TamingMob` (via their `sit` action)
  and merged with their `bodyRelMove` offsets. Real sample: chair `03016200`
  (`03016.img`) with taming mobs `01983279`..`01983282`. `CustomChairType` is an enum
  (`Unsupported`, `ScaleAvatarChair`, `Other(String)`); `effective_scale()` handles
  `ScaleAvatarChair` and falls back to `info/scale` for other/unknown types.
- Custom origin: the two-step C# semantics are implemented:
  `CustomOriginMap` (originValue → childName) + `CustomOrigin` (childName → Point)
  → `apply_custom_origin_map`. The GUI accepts `childName:x,y;...`.
- Chair pose: when a `Chair` is in the equipment list, the body frame prefers `sit`
  over `stand1`/`stand`. Group taming members also use `sit`.
- Current-client parity: no known avatar gaps. Future clients may add new
  `CustomChairType` variants (preserved as `Other`), and the GUI part renderer uses
  RGBA only (the scene API supports per-frame effect origins).

## Verification

`WZ_DATA_ROOT=<Data> cargo test -p wz-avatar` passes all 18 real-data tests, including
body/head loading, equipment loading, zmap, and the prism unit test.
