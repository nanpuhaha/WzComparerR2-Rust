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
- `icons <wz> <image.img>` — decodes ARGB4444/ARGB8888 canvases and emits
  `path/width/height/format/RGBA SHA-256`.
- `items <item.wz> <image.img> <String_000.wz>` — emits the DB2 Item grid rows
  (`id/name/desc/has_icon`) using the original `StringLinker`.
- `maps <map.wz> <String_000.wz>` — emits the DB2 Map grid rows
  (`id/street_name/map_name/has_minimap/link`).
- `mobs <mob.wz> <String_000.wz>` — emits the DB2 Mob grid rows
  (`id/name/stand/fly/move/hit/die/thumbnail/link`) using `StringLinker.StringMob`.
- `skills <skill.wz> <String_000.wz>` — emits DB2 Skill book/skill rows
  (`id/row_type/book/name/desc/icon/common/h_count`) using
  `StringLinker.StringSkill/StringSkill2`.
- `npcs <npc.wz> <String_000.wz>` — emits DB2 NPC rows
  (`id/name/stand/default/link`) using `StringLinker.StringNpc`.
- `reactors <reactor.wz>` — emits DB2 Reactor rows
  (`id/has_icon/has_info/has_link/link`).
- `music <sound.wz>` — recursively lists `Bgm*`/`PL_*` `Sound_DX8` paths with
  byte length and duration.
- `damageskins <consume.wz> <String_000.wz>` — emits DB2 DamageSkin rows
  (`id/name/desc/icon_image/has_sample`) using `StringLinker.StringItem`.
- `morphs <consume.wz> <morph.wz> <String_000.wz>` — emits DB2 Morph rows
  (`id/name/desc/morph_id/has_icon/has_morph`).

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

Verified icon output:
```
dotnet run ... -- icons ../../Data/Character/Weapon/_Canvas/_Canvas_000.wz 01302000.img
→ 18 canvases
```
Rust `wz db2-icons ... --json` matches all 18 RGBA SHA-256 values exactly.

Verified DB2 Item grid output:
```
dotnet run ... -- items ../../Data/Item/Consume/Consume_000.wz 0200.img ../../Data/String/String_000.wz
→ 530 rows
```
Rust `wz db2-items ... --json` matches all 530 rows exactly.
Requires `-p:CommonPath=<WzComparerR2.Common.dll>` (built with
`-p:DotnetEdition=core -p:SystemDrawingCommonVersion=8.0.0`).

Verified DB2 Map grid output:
```
dotnet run ... -- maps ../../Data/Map/Map/Map0/Map0_000.wz ../../Data/String/String_000.wz
→ 87 rows
```
Rust `wz db2-maps ... --json` matches all 87 rows exactly.

Verified DB2 Mob grid output:
```
dotnet run ... -- mobs ../../Data/Mob/_Canvas/_Canvas_011.wz ../../Data/String/String_000.wz
→ 55 rows
```
Rust `wz db2-mobs ... --json` matches all 55 rows exactly.
This client's split `Mob/_Canvas` files mostly contain animation canvases without
the full `info` stat block, so the DB2 stat column is empty for this fixture.

Verified DB2 Skill grid output:
```
dotnet run ... -- skills ../../Data/Skill/_Canvas/_Canvas_000.wz ../../Data/String/String_000.wz
→ 165 rows
```
Rust `wz db2-skills ... --json` matches all 165 rows exactly.

Verified DB2 NPC grid output:
```
dotnet run ... -- npcs ../../Data/Npc/_Canvas/_Canvas_000.wz ../../Data/String/String_000.wz
→ 943 rows
```
Rust `wz db2-npcs ... --json` matches all 943 rows exactly.

Verified DB2 Reactor grid output:
```
dotnet run ... -- reactors ../../Data/Reactor/_Canvas/_Canvas_000.wz
→ 441 rows
```
Rust `wz db2-reactors ... --json` matches all 441 rows exactly.

Verified DB2 Music grid output:
```
dotnet run ... -- music ../../Data/Sound/Sound_000.wz
→ 40 rows
```
Rust `wz db2-music ... --json` matches all 40 rows exactly.

Verified DB2 DamageSkin grid output:
```
dotnet run ... -- damageskins ../../Data/Item/Consume/Consume_000.wz ../../Data/String/String_000.wz
→ 952 rows
```
Rust `wz db2-damage-skins ... --json` matches all 952 rows exactly.

Verified DB2 Morph grid output:
```
dotnet run ... -- morphs ../../Data/Item/Consume/Consume_000.wz ../../Data/Morph/Morph_000.wz ../../Data/String/String_000.wz
→ 234 rows
```
Rust `wz db2-morphs ... --json` matches all 234 rows exactly.
