# Avatar anchor / prism / zmap parity

Status: resolved 2026-11-05; body/head/equipment/weapon frame assembly and GUI prism
controls implemented. This note records the C# sources, the Rust implementation, and
the remaining CharaSim gaps.

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
- Still out of scope: C# `CharaSim` mix color / custom origin maps, and the full
  effect/taming/chair layer set.

## Verification

`WZ_DATA_ROOT=<Data> cargo test -p wz-avatar` passes all 18 real-data tests, including
body/head loading, equipment loading, zmap, and the prism unit test.
