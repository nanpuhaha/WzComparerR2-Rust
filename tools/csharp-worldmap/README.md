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
- `set <etc.wz> <setId>` — emits SetItem name/completeCount/Effect lines.
- `potential <item.wz> <code> <level>` — emits ItemOption lines.
- `db2 <wz> <image.img>` — emits the DB2 node table rows
  (`path/name/node_type/value/children`).

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

Verified set/potential output:
```
dotnet run ... -- set ../../Data/Etc/Etc_000.wz -1
→ {"name":"결속의 반지","complete":5,"effects":[{"count":1,"lines":["공격력 : +2",...]},...]}
dotnet run ... -- potential ../../Data/Item/Item_000.wz 1 1
→ ["STR : +1"]
```
Rust tests compare these fixtures exactly and append the same set/potential
sections to the gear tooltip bitmap.

Verified DB2 output:
```
dotnet run ... -- db2 ../../Data/Character/Weapon/Weapon_000.wz 01302000.img
→ 270 rows
```
Rust `wz db2 ... --json --limit 1000` produces exactly the same 270 rows.
