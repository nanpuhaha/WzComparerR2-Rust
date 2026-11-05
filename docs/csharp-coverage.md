# C# 솔루션 전체 파일 커버리지 감사

원본: `/Users/jwseo/GitHub/WzComparerR2-seotbeo` (macOS, .NET 10).
LOC는 `*.cs`에서 `obj/`, `bin/`을 제외한 줄 수다.

상태: ✅ 구현+검증, 🟡 부분, ⬜ 미구현(UI/플랫폼 전용 포함).

## 프로젝트 요약

| 프로젝트 | 파일 | LOC | Rust 대응 | 상태 |
|---|---:|---:|---|---|
| WzComparerR2.WzLib | 54 | 13,513 | `wz-core`, `wz-image`, `wz-pool` | ✅ (핵심) |
| WzComparerR2.Common | 146 | 23,950 | `wz-db`, `wz-avatar`, `wz-render` | 🟡 |
| WzComparerR2.MapRender | 124 | 29,291 | `wz-db::map`, `wz-gui`, `wz-render` | 🟡 |
| WzComparerR2.Avatar | 13 | 7,055 | `wz-avatar` | ✅ |
| WzComparerR2 (main) | 150 | 61,035 | `wz-cli`, `wz-gui` | 🟡 |
| WzComparerR2.DB2 | 12 | 6,292 | — | ⬜ |
| CharaSimResource | 2 | 7,490 | — (WinForms 리소스) | ⬜ |
| WzComparerR2.Network | 28 | 1,954 | — | ⬜ |
| WzComparerR2.LuaConsole | 10 | 1,451 | — | ⬜ |
| WzComparerR2.PluginBase | 10 | 693 | — | ⬜ |
| WzComparerR2.Updater | 3 | 276 | `wz patch` (부분) | 🟡 |

## 디렉터리별

### WzComparerR2 (main)

| 디렉터리 | 파일 | LOC | 대응 | 상태 |
|---|---:|---:|---|---|
| (root) WinForms 메인/비교 창 | 48 | 26,281 | `wz-gui` 최소 PoC | ⬜/🟡 |
| CharaSimControl | 40 | 21,299 | `render_tooltip_bitmap`, `gear_tooltip` | 🟡 |
| AvatarCommon | 12 | 4,086 | `wz-avatar` | ✅ |
| Comparer | 6 | 3,080 | `wz-diff` + `virtual_node` | 🟡 |
| Patcher | 16 | 2,547 | `wz-core::patch`, `wz patch` | ✅ core/CLI, UI ⬜ |
| CharaSim | 5 | 1,957 | `wz-avatar::stats`, `gear_tooltip` | 🟡 |
| Config | 16 | 1,086 | — | ⬜ |
| SoundPlayer | 5 | 400 | `wz-gui::sound_player` | ✅ |

### WzComparerR2.Common

| 디렉터리 | 파일 | LOC | 대응 | 상태 |
|---|---:|---:|---|---|
| CharaSim | 54 | 10,121 | `wz-avatar::stats` | 🟡 (Gear/Set/Potential/MaxStar) |
| (root) | 15 | 2,832 | `wz-db` | 🟡 |
| Rendering | 25 | 2,726 | `wz-render`, `wz-image` | 🟡 (light/water/sprite) |
| Animation | 15 | 2,415 | `wz-avatar` frame loader | 🟡 |
| Controls | 16 | 2,138 | — | ⬜ |
| OpenAPI | 4 | 2,086 | — | ⬜ |
| Text | 4 | 707 | `wz-core::text` | ✅ |
| Encoders | 6 | 534 | `wz-image` | ✅ (PNG/BC7) |
| Config | 6 | 355 | — | ⬜ |

### WzComparerR2.MapRender

| 디렉터리 | 파일 | LOC | 대응 | 상태 |
|---|---:|---:|---|---|
| (root) MapData/Mesh/Scene | 53 | 16,268 | `map.rs`, `wz-gui` | 🟡 |
| UI | 31 | 8,753 | minimap/worldmap 패널 일부 | 🟡 |
| Patches2 | 20 | 1,573 | background/life/reactor/portal/particle | ✅ |
| Patches | 10 | 855 | 대부분 Patches2로 대체 | 🟡 |
| Effects | 3 | 685 | `water.rs`, `light.rs` | ✅/🟡 |
| Animation | 4 | 410 | `wz-avatar`/frame | 🟡 |

### WzComparerR2.WzLib

| 디렉터리 | 파일 | LOC | 대응 | 상태 |
|---|---:|---:|---|---|
| (root) | 30 | 6,152 | `wz-core` | ✅ |
| Compatibility | 9 | 3,185 | MS/PKG2 일부 | 🟡 |
| Utilities | 12 | 3,005 | `wz-pool`/`wz-core` | ✅ |
| Cryptography | 2 | 1,135 | `wz-core::crypto` | ✅ |

## 가장 큰 미구현/부분 모듈

1. `WzComparerR2` root WinForms UI (26,281 LOC) — Rust egui PoC만 존재.
2. `CharaSimControl` (21,299 LOC) — 비트맵 툴팁 일부만.
3. `WzComparerR2.Common/CharaSim` (10,121 LOC) — Gear/Set/Potential/MaxStar만.
4. `MapRender/UI` (8,753 LOC) — Minimap/WorldMap 일부.
5. `WzComparerR2.DB2` (6,292 LOC) — WZ 데이터 그리드/아이콘/이미지 뷰어 미구현.
6. `WzComparerR2.Common/Controls` (2,138 LOC), `OpenAPI` (2,086 LOC) — 미구현.
7. `LuaConsole` (1,451), `Network` (1,954), `PluginBase` (693) — 미구현.

## 이번 턴 추가 구현

- `wz-diff::virtual_node`: 원본 `WzVirtualNode.Combine` 병합 규칙
  (동일 이름 디렉터리 재귀 병합, 값/디렉터리 충돌 오류).
- C# `WzLib` 실행 하네스 `tools/csharp-worldmap`:
  - WorldMap QuestLimit/Fog fixture JSON 일치
  - 실제 WorldMap 85 images / QuestLimit 0 / Fog 0
  - `000010000.img` C# 4721 nodes, Rust oracle 4722 = outer Directory wrapper.
    per-node path multiset는 wrapper 제거 후 4721 = 4721로 일치.

## 남은 작업

- DB2/CharaSimControl/Comparer HTML/EasyComparer 등 UI·리포팅 모듈.
- Common Controls/OpenAPI, LuaConsole, Network, PluginBase.
- WorldMap 실제 QuestLimit/Fog 데이터는 클라이언트에 없음.
- weather는 원본 C# 로더/에셋에 없음.
