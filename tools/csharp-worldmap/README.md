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
- `tamingmobs <taming.wz> <String_000.wz>` — emits DB2 TamingMob rows
  (`id/name/desc/h1/icon/reqLevel/reqJob/tuc/islot/tamingMob`).
- `characters <character.wz> <String_000.wz>` — emits DB2 Character equipment
  rows (`id/name/desc/h1/icon/reqLevel/reqJob/tuc/islot/vslot/stat`).
- `skillsummary <String_000.wz> <limit>` — runs the original
  `SummaryParser.GetSkillSummary` on every skill `desc` and `h/h1..hn`.
- `itemtooltip <item.wz> <image.img> <id> <String_000.wz>` — runs
  `ItemTooltipRender2.GetItemAttributeString` + StringLinker name/desc.

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

Verified DB2 TamingMob grid output:
```
dotnet run ... -- tamingmobs ../../Data/Character/TamingMob/TamingMob_000.wz ../../Data/String/String_000.wz
→ 1265 rows
```
Rust `wz db2-taming-mobs ... --json` matches all 1265 rows exactly.

Verified DB2 Character grid output (Coat):
```
dotnet run ... -- characters ../../Data/Character/Coat/Coat_000.wz ../../Data/String/String_000.wz
→ 682 rows
```
Rust `wz db2-characters ... --json` matches all 682 rows exactly.

Verified DB2 Character grid output (Cap):
```
dotnet run ... -- characters ../../Data/Character/Cap/Cap_000.wz ../../Data/String/String_000.wz
→ 3166 rows
```
Rust `wz db2-characters ... --json` matches all 3166 rows exactly.

Verified Skill summary output:
```
dotnet run ... -- skillsummary ../../Data/String/String_000.wz 100000
→ 20629 rows (all skill desc + h entries)
```
Rust `wz skill-summary ... --limit 100000 --json` matches all 20629 rows exactly.
The fixture test keeps the first 2000 rows (`skill_summary_2000.expected.json`).

Verified ItemTooltip output:
```
dotnet run ... -- itemtooltip ../../Data/Item/Consume/Consume_000.wz 0200.img 02000012 ../../Data/String/String_000.wz
→ {"name":"엘릭서","desc":"...","attributes":["교환 불가"]}
```
Rust `wz item-tooltip ... --json` matches exactly.

Verified DB2 Character grid output (Shoes):
```
dotnet run ... -- characters ../../Data/Character/Shoes/Shoes_000.wz ../../Data/String/String_000.wz
→ 1703 rows
```
Rust `wz db2-characters ... --json` matches all 1703 rows exactly.

Verified DB2 Character grid output (Accessory):
```
dotnet run ... -- characters ../../Data/Character/Accessory/Accessory_000.wz ../../Data/String/String_000.wz
→ 2369 rows
```
Rust `wz db2-characters ... --json` matches all 2369 rows exactly.

Verified DB2 Character grid output (Longcoat):
```
dotnet run ... -- characters ../../Data/Character/Longcoat/Longcoat_000.wz ../../Data/String/String_000.wz
→ 2493 rows
```
Rust `wz db2-characters ... --json` matches all 2493 rows exactly.

Verified DB2 Character grid output (Glove):
```
dotnet run ... -- characters ../../Data/Character/Glove/Glove_000.wz ../../Data/String/String_000.wz
→ 661 rows
```
Rust `wz db2-characters ... --json` matches all 661 rows exactly.

Verified DB2 Character grid output (Pants):
```
dotnet run ... -- characters ../../Data/Character/Pants/Pants_000.wz ../../Data/String/String_000.wz
→ 579 rows
```
Rust `wz db2-characters ... --json` matches all 579 rows exactly.

Verified DB2 Character grid output (Shield):
```
dotnet run ... -- characters ../../Data/Character/Shield/Shield_000.wz ../../Data/String/String_000.wz
→ 142 rows
```
Rust `wz db2-characters ... --json` matches all 142 rows exactly.

Verified DB2 Character grid output (Cape):
```
dotnet run ... -- characters ../../Data/Character/Cape/Cape_000.wz ../../Data/String/String_000.wz
→ 1139 rows
```
Rust `wz db2-characters ... --json` matches all 1139 rows exactly.

Verified DB2 Character grid output (PetEquip):
```
dotnet run ... -- characters ../../Data/Character/PetEquip/PetEquip_000.wz ../../Data/String/String_000.wz
→ 511 rows
```
Rust `wz db2-characters ... --json` matches all 511 rows exactly.

Verified DB2 Character grid output (Weapon):
```
dotnet run ... -- characters ../../Data/Character/Weapon/Weapon_000.wz ../../Data/String/String_000.wz
→ 5655 rows
```
Rust `wz db2-characters ... --json` matches all 5655 rows exactly.

Verified DB2 Character grid output (Hair):
```
dotnet run ... -- characters ../../Data/Character/Hair/Hair_000.wz ../../Data/String/String_000.wz
→ 9004 rows
```
Rust `wz db2-characters ... --json` matches all 9004 rows exactly, including
the `default/hairOverHead` icon path.

Verified DB2 Character grid output (Face):
```
dotnet run ... -- characters ../../Data/Character/Face/Face_000.wz ../../Data/String/String_000.wz
→ 7424 rows
```
Rust `wz db2-characters ... --json` matches all 7424 rows exactly, including
the `default/face` icon path.

Verified DB2 Character grid output (Mechanic):
```
dotnet run ... -- characters ../../Data/Character/Mechanic/Mechanic_000.wz ../../Data/String/String_000.wz
→ 23 rows
```
Rust `wz db2-characters ... --json` matches all 23 rows exactly.

Verified DB2 Character grid output (Dragon):
```
dotnet run ... -- characters ../../Data/Character/Dragon/Dragon_000.wz ../../Data/String/String_000.wz
→ 16 rows
```
Rust `wz db2-characters ... --json` matches all 16 rows exactly.

Verified DB2 Character grid output (Ring):
```
dotnet run ... -- characters ../../Data/Character/Ring/Ring_000.wz ../../Data/String/String_000.wz
→ 1127 rows
```
Rust `wz db2-characters ... --json` matches all 1127 rows exactly.
Afterimage returns 0 rows in both because its image names do not start with `0`
(not a Character grid target).

Additional verified Character categories (same `characters` mode):
```
Android 215, ArcaneForce 7, AuthenticForce 8, Bits 79, EquipBag 2
```
All match Rust `wz db2-characters` exactly.

Verified DB2 Item tab grids (`itemtab <Data> <dir> <String_000.wz>`):
```
Cash 3539, Consume 18377, Etc 5238, Install 3851, Pet 614, Special 1224
```
All 32,843 rows match Rust `wz db2-item-tab` exactly. The harness mirrors
`DB2Form.LoadItem`: `IDString()` = `int.Parse(...).ToString()`, `"  "` only when
the String node is missing, Special filtered by `Etc/CashPackage.img`.

Verified `GetGearPropString` output (`gearprops <character.wz>`):
```
Coat 228, Weapon 1262, Cap 965, Accessory 1576, Face 4, Ring 978, TamingMob 89
= 5102 distinct (key,value) pairs
```
All match Rust `wz gear-props` exactly, including nulls for `reqLevel`/`tuc`/
`incACC`/`incEVA`/`incMDD`.

Verified OpenAPI helpers (`openapi <text>`):
```
dotnet run ... -- openapi OBDLADCO...
-> decrypt hex / decrypt_len / check_ver / item_id
```
Rust `wz-openapi` matches 16/32/64-byte vectors exactly, including the
8-char `GetItemID` exception case.

Verified ItemTooltip sweep (`itemtooltips <Data> <dir> <String_000.wz>`):
```
Cash 3539, Consume 18377, Etc 5238, Install 3851, Pet 614, Special 1224
= 32843 items
```
All name/desc/attributes match Rust `wz db2-item-tab-tooltips` exactly.

Verified Network RC4 (`networkrc4 <WzComparerR2.Network.dll> <keyhex> <datahex>`):
```
key 8/16/32 bytes × data 1/8/17/64 bytes = 12 vectors
```
All match Rust `wz-network::Rc4` exactly.

Verified `ItemStringHelper` text helpers:
```
jobnames 60000      -> 417 non-null codes
fifthjobnames       -> 198 (skill, jobId) combos
corespec            -> 408 (kind, value, desc) combos
```
All match Rust (`wz-avatar::get_job_name`, `wz_avatar::get_fifth_job_name`,
`wz_db::item_core_spec_string`) exactly.

Verified AchievementTooltip (`achievements <AchievementData_000.wz>`):
```
1579 achievement rows
```
All name/desc/score/difficulty/uiForm/block/category/prior/missions/rewards
and rendered text lines match Rust `wz achievements` exactly.

Verified WorldArchive tooltip text (`worldarchive <String_000.wz> <Etc_002.wz>`):
```
1845 mob/npc entries (world archive desc + monster book + NPC quotes)
```
All lines match Rust `wz world-archive` exactly.

Verified PluginBase `FindWz` path lookup (`findwz Mob=<wz> ... -- <paths>`):
```
57 paths across Mob/Npc/Skill/Reactor/String
```
found/name/value/children all match Rust `wz find-wz`.

Verified Network packet serialization (`netpack <WzComparerR2.Network.dll>`):
```
heartbeat, loginreq, loginresp, sendchat, sendchat-null, cryptreq, cryptresp
```
JSON + framed bytes match Rust `wz-network::pack` exactly.

Verified NPC/Reactor tooltip text (`charatooltips <npc.wz> <reactor.wz> <String_000.wz> <Etc_000.wz>`):
```
12629 rows (12188 NPC + 441 reactor)
```
Lines match Rust `wz chara-tooltips` exactly.

Verified DamageSkin model (`damageskinmodel <Etc_000.wz> DamageSkin.img`):
```
402 skins
```
desc/extractID/customType/sample and per-section keys/spacing/bottomFix match
Rust `wz damage-skin-model` exactly.

Verified OpenAPI avatar unpack (`avatarstructure <v>`, `avatarunpack <v> <packhex>`):
```
19 versions x 2 packs = 38 cases
```
Field lists match Rust `wz_openapi::avatar::unpack` exactly.

Verified Map tooltip text (`maptooltips <Map0_000.wz> <String_000.wz>`):
```
87 maps
```
street/map name/desc/bgm/mobs/npcs match Rust `wz map-tooltips` exactly.

Verified Quest tooltip model (`questtooltips <QuestData_000.wz>`):
```
18964 quests
```
Info/Check/Act1 reward fields match Rust `wz quest-tooltips` exactly.

Verified PluginBase file discovery (`pluginfiles <plugin-dir>`):
```
WzComparerR2*.dll recursive, case-insensitive
```
Matches Rust `wz plugin-files` on a fixture tree (4 files).
Assembly type enumeration (`LoadPlugin`) is not implemented: it requires a
.NET metadata reader.

Verified ItemTooltip recipe lines (`itemextras <Data> Consume`):
```
827 items
```
`recipeValidDay`/`recipeUseCount` lines match Rust `wz item-extras` exactly.

Verified ItemTooltip cash/tag lines (`itemcashlines <Data> Cash`):
```
2503 items
```
All lines match Rust `wz item-cash-lines` exactly.

Verified ItemTooltip text extras (`itemtextextras <Data> <dir> <String_000.wz>`):
```
Cash 91, Consume 1151
```
desc_leftalign(`#t` 치환)/pointCost/cantAccountSharable all match Rust.

Verified ItemTooltip incline/trade lines (`itemincline <Data> <dir>`):
```
Consume 26, Install 23
```
Incline and tradeAvailable lines match Rust `wz item-incline` exactly.

Verified PluginBase reflection (`pluginentries <dll>`):
```
WzComparerR2.FixturePlugin 1.2.3.4
AlphaPlugin, DerivedPlugin (abstract/internal excluded)
```
Matches Rust `wz plugin-metadata` exactly via the minimal PE/CLI reader.

Verified ItemTooltip recipe requirements (`itemrecipereq <Data> Consume <String_000.wz>`):
```
1039 items
```
`< 사용 제한조건 >` skill/level lines match Rust `wz item-recipe-req` exactly.

Verified ItemTooltip expire lines (`itemexpire <Data> <dir>`):
```
Consume 418, Cash 19, Install 2, Etc 10 = 449 rows
```
`consumableFrom`/`endUseDate` formatting matches Rust `wz item-expire` exactly.
72 malformed (<12 digit) field values exist; the original C# throws on them,
so both sides process only valid values.

Verified Common/Controls pure logic (`animtimeline`):
```
timelines 7, probes 30, gif_cases 90, item_times 6, max_length 4,
time_offset 5, reset 5, scale 8
```
The originals (`FrameAnimator`, `AnimationRecoder`, `AnimationControl`) fail to
load on macOS because they reference SharpDX, so the harness compiles the
verbatim C# method bodies as a reference implementation. All cases match Rust
`wz-db::common_controls`.

Verified DB2 partitioned grids:
```
mappart <Data> 1|2|3 <String_000.wz>  -> 4382 / 1946 / 11141 rows
mobpart <Data> 1|2|3 <String_000.wz>  ->  742 / 1160 /  1585 rows
```
`mappart` mirrors `LoadMap(Part)` including the `info/link` minimap second pass;
`mobpart` mirrors the leading-digit partition filter of `LoadMob(Part)`.
Both match Rust `wz db2-map-part` / `wz db2-mob-part` exactly.

Verified Skill tooltip header (`skilltooltips <Data> <String_000.wz>`):
```
7055 skills
```
name/master-level/pre_bb/hyper/req lines all match Rust `wz skill-tooltips`.

Verified LuaConsole `package.path` (`luapath <script> <plugin> <wcr2> <cwd>`):
```
5 input combinations -> entries / joined path
```
`LuaSandbox.InitLuaEnv` needs NLua + WinForms, so the harness compiles the
verbatim path-building block. Rust `wz_lua::package_path` matches all cases;
script execution is verified separately with real Lua fixtures.

Verified ItemTooltip `#cosmetic_EULO#` (`itemcosmetic <Data> <String_000.wz>`):
```
1133 items
```
Uses the real `Gear.GetGender`; `gender/eqp_name/replacement/replaced` match
Rust `wz item-cosmetic-eulo` exactly.

Verified PluginBase instantiation (`pluginactivate <plugin.dll> <PluginBase.dll>`):
```
context_created=true; Fixture.AlphaPlugin, Fixture.DerivedPlugin
```
`PluginContext` is constructed with a null provider, so `LoadPlugin`'s
`Activator.CreateInstance(type, context)` path runs on macOS. Metadata matches
Rust `wz_db::plugin_entries`.
