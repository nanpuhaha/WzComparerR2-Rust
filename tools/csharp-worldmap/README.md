# C# WzLib comparison harness

Runs the original C# `WzComparerR2.WzLib` parser against the same inputs as Rust.

## Run

```
dotnet run -c Release \
  -p:WzLibPath=/path/to/WzComparerR2.WzLib/bin/Debug/net8.0/WzComparerR2.WzLib.dll \
  -- fixture ../../crates/wz-gui/tests/fixtures/worldmap_quest_fog.json
```

Commands:
- `fixture <json>` — builds a `Wz_Node` tree and runs the C# `UIWorldMap`
  `LoadQuestLimit`/`LoadFog`/MapList field extraction; prints JSON.
- `real <wz>` — loads a real WZ and counts images / QuestLimit / Fog nodes.
- `mapnodes <wz> <image.img>` — recursively counts parsed image nodes.
- `gear <wz> <image.img>` — emits CharaSim gear tooltip lines for the item
  (same `ItemStringHelper.GetGearPropString` labels as Rust).

Verified output (macOS, .NET 10):
- fixture output matches `crates/wz-gui/tests/fixtures/worldmap_quest_fog.expected.json`.
- `Data/Map/WorldMap/WorldMap_000.wz` → `{"images":85,"quest":0,"fog":0}`.
- `Data/Map/Map/Map0/Map0_000.wz 000010000.img` → C# 4721 nodes.
  Rust oracle reports 4722 because it counts the outer Directory wrapper.
  Per-node path diff after stripping the oracle wrapper matches exactly
  (4721 = 4721; no missing/extra WZ nodes).

Verified gear output (Weapon_000.wz / 01302000.img):
```json
["착용 레벨 : 0", "공격력 : +15", "업그레이드 가능 횟수 : 8"]
```
Rust `gear_tooltip::tests::weapon_tooltip_lines_and_bitmap` compares exactly
against this output and renders the bitmap.
