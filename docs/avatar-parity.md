# Avatar anchor / prism / zmap parity

Status: research resolved 2026-11-05. This note records the C# sources, the current
Rust state, and the decision for the remaining work.

## C# sources (pinned `bb98c25…`)

- `WzComparerR2/AvatarCommon/AvatarCanvas.cs`
  - `LoadZ()` / `LoadZ(Wz_Node)` reads the child names of `Base\zmap.img` in order.
  - `AppendBone(root, parentBone, skin, mapName, mapOrigin)` creates the bone chain.
    `mapName` comes from the canvas mini-property `map` (`navel`, `neck`, `brow`, …);
    `mapOrigin` is that entry's vector.
  - `BitmapOrigin.CreateFromNode` reads the canvas `origin` vector.
  - Prism is applied per-part via `PrismDataCollection` + `Prism.Apply`.
- `WzComparerR2/AvatarCommon/Prism.cs` — HSV v2 transform, already ported to
  `crates/wz-avatar/src/prism.rs`.

## Current Rust state

- `crates/wz-avatar/src/frame_layers.rs::collect_frame_layers` is the **complete**
  bone/map/origin implementation: it reads `origin`, `z`, `map`, `_outlink`, builds
  the bone chain (including joint bones), and computes absolute layer positions.
- `crates/wz-avatar/src/loader.rs` is the older PoC path. It now:
  - reads `origin` and the first `map` entry from `PngRef.sub_property`;
  - uses that map name/origin in `append_loaded_part` (fallback `navel`);
  - loads `zmap` order from `Base/Base_000.wz::zmap.img` and falls back to
    `STANDARD_ZMAP`.
- `prism.rs` implements the color transform but is not yet applied by `loader.rs`.

## Decision

`collect_frame_layers` is the reference implementation for avatar assembly. The
remaining full-parity work is to move `loader.rs` onto it (including multi-entry
`map` chains and prism application) rather than extending the PoC path. That is a
post-v0.1 implementation task; this ticket is resolved as research.

The v0.1 improvement is visible in `loader.rs`: anchors/origins are no longer all
`navel`, and zmap order is data-driven when `Base/zmap.img` is available.
