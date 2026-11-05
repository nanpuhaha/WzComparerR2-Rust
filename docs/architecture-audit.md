# 아키텍처 감사 — mattpocock improve-codebase-architecture 적용 (#32 후속)

> 기준 스킬: `codebase-design` (deep modules, seams, deletion test, dependency categories).
> 대상: 워크스페이스 9개 크레이트 + 2개 도구 + GUI.
> 목적: 얕은(shallow) 모듈을 찾아 깊게(deep) 만들 계획을 세우고, 우선순위가 높은 것부터 실행한다.

---

## 1. 용어 (스킬 어휘)

- **Module**: 인터페이스+구현을 가진 것 (함수/클래스/패키지/슬라이스).
- **Interface**: 호출자가 알아야 할 모든 것 (시그니처 + 불변식/순서/에러/성능).
- **Depth**: 인터페이스 대비 구현의 양. 깊은 모듈 = 작은 인터페이스 + 많은 구현.
- **Seam**: 구현을 바꾸지 않고 동작을 바꿀 수 있는 위치.
- **Adapter**: seam 에서 인터페이스를 만족하는 구체물.
- **Deletion test**: 모듈을 삭제했을 때 복잡도가 사라지면 pass-through, N 호출자에 재출현하면 가치 있음.

---

## 2. 크레이트별 depth 평가

| 크레이트 | 라인 | Depth | 평가 |
|---|---:|---|---|
| `wz-error` | 203 | 깊음 | 단일 에러 어휘. 인터페이스 작음, 구현(변환 impl) 많음. ✅ |
| `wz-pool` | 1402 | 깊음 | ByteSource trait + Memory/File/Mmap/Window 어댑터. Seam 이 명확 (trait). ✅ 4개 어댑터 → 실제 seam. |
| `wz-core` | 11389 | 깊음 | WzArchive::open/read_tree/parse_image_at 가 작은 인터페이스 뒤에 포맷 전체를 숨김. ✅ |
| `wz-image` | 1910 | 중간 | decode_* 함수들이 개별. `TextureFormat::from_u32` + decode_raw_page_to_rgba8 조합이 호출자에 노출됨. ⚠️ |
| `wz-render` | 2491 | 깊음 | RenderContext 하나의 인터페이스 뒤에 wgpu 전체. ADR-4 로 경계 엄격. ✅ |
| `wz-db` | 2967 | 깊음 | Arena + QueryEngine + StringLinker + MapData. 인터페이스가 NodeId 핸들로 작음. ✅ |
| `wz-diff` | 487 | 중간 | DiffEngine. 작지만 얕음 — CLI 가 직접 호출. ⚠️ |
| `wz-avatar` | 1267 | 중간 | AvatarScene/Canvas. **WZ→avatar 로더가 없음** (기능 갭, 아키텍처 아님). |
| `wz-cli` | 3026 | **얕음** | `main.rs` 한 파일에 30+ `cmd_*` 함수. 모듈 경계 없음. 🔴 |
| `wz-gui` | 853 | **얕음** | `main.rs` 한 파일에 상태+패널+IO+렌더. 🔴 |

---

## 3. 얕은 모듈 / 개선 후보

### 3.1 `wz-cli/src/main.rs` (3026줄) — 가장 큰 문제

**증상**: `main.rs` 가 파서(clap), 디스패치(match), 30+ `cmd_*` 핸들러, 헬퍼(`find_image_entry_in_dirtree`, `build_arena_from_archive`, `extract_png_ref`, `extract_gear_stats_from_node`), 테스트를 한 파일에 담는다.

**Deletion test**: `cmd_avatar`, `cmd_oracle_*` 를 삭제하면 CLI 가 기능을 잃는다 → 가치 있음. 하지만 *파일 분할*은 가치 있음 (locality).

**권장 deepening**: `commands/` 디렉터리로 분할.
```
crates/wz-cli/src/
├── main.rs          (clap 파서 + dispatch 만, ~100줄)
├── commands/
│   ├── mod.rs
│   ├── tree.rs, image.rs, diff.rs, dump.rs
│   ├── avatar.rs, query.rs, oracle.rs
│   └── corpus.rs
├── helpers.rs       (find_image_entry_in_dirtree, build_arena_from_archive, extract_*)
└── oracle/          (이미 oracle 서브커맨드가 흩어짐 → 모듈화)
```
- 인터페이스: `pub fn run(cli: Cli) -> Result<(), Box<dyn Error>>`.
- 각 command 모듈은 `pub fn execute(args) -> Result`.
- **Seam**: command trait? 한 어댑터(CLI)만 있으므로 seam 은 가상 — 도입하지 않는다 (스킬 원칙: "One adapter means a hypothetical seam"). 그냥 함수 분할.

### 3.2 `wz-gui/src/main.rs` (853줄) — 단일 파일

**증상**: `AppState`, `LoadMsg`, `TreeRow`, `PocApp`, `render_*`, `load_*`, `find_*` 가 한 파일.

**권장 deepening**: `docs/gui-architecture.md` §3.1 의 구조로 분할.
```
crates/wz-gui/src/
├── main.rs          (eframe 진입점, ~30줄)
├── app.rs           (PocApp: eframe::App)
├── state.rs         (AppState, LoadMsg, TreeRow)
├── panels/          (tree_panel, inspector, viewport, avatar_panel)
├── async_io.rs      (open_file_dialog, poll_load_result, load_map_*, load_back_image)
└── render_bridge.rs (render_offscreen_triangle, render_viewport)
```
- 인터페이스: `AppState` 가 단일 진실 소스 (이미 설계됨). 패널은 `&mut AppState` 를 받는 함수.

### 3.3 `wz-image` decode API — 얕은 인터페이스

**증상**: 호출자가 `TextureFormat::from_u32`, `extract_raw_bytes`, `decode_raw_page_to_rgba8` 를 조합해야 한다. GUI 의 `load_back_image` 가 이 조합을 다시 구현했다.

**권장 deepening**: `wz-image` 에 고수준 함수 추가.
```rust
pub fn decode_png_ref_to_rgba(png_ref: &PngRef, src: &Arc<dyn ByteSource>) -> Result<(u32,u32,Vec<u8>), ImageError>;
```
- 이 함수 하나가 raw 추출 + 포맷 분기 + 디코딩을 숨긴다.
- GUI 의 `load_back_image` 가 이 함수를 쓰도록 → 중복 제거, locality 향상.

### 3.4 `wz-avatar` — WZ 로더 부재 (기능 갭)

**증상**: `AvatarScene`/`Bone`/`SkinLayer` 가 데이터 구조만 있고, WZ → avatar 변환 로더가 없다. CLI `cmd_avatar` 는 합성 레이어를 쓴다.

**권장**: `wz-avatar` 에 `AvatarLoader` 모듈 추가 (또는 `wz-db` 에). 이는 아키텍처 개선이 아니라 기능 추가. 별도 티켓.

### 3.5 `wz-core` DirTree 파싱 — depth 양호

`DirTreeParser::parse` 의 `blocks` + 커서 규칙이 호출자(CLI, GUI)에 누출된다 (`build_arena_from_archive` 가 커서를 재구현). 

**권장 deepening**: `WzArchive` 에 `read_tree_flat()` 또는 `Arena::from_archive()` 를 추가해 커서 규칙을 한 곳에 가둔다. `build_arena_from_archive` 는 `wz-db` 로 이동.

### 3.6 `wz-db::Arena` — find_child 의 "마지막 매치" 규칙

문서화는 잘 되어 있다. 인터페이스가 작다. ✅ 깊음.

---

## 4. 의존성 카테고리 (DEEPENING.md 기준)

| 의존성 | 카테고리 | seam 전략 |
|---|---|---|
| `wz-pool::ByteSource` | in-process (trait) | 이미 seam. 4 어댑터. ✅ |
| `wgpu` (render) | true external | `RenderContext` 가 어댑터. GUI 는 `new_headless` 만 씀 — 서페이스 모드 추가 시 seam. |
| `eframe/egui` | true external | GUI 크레이트가 직접 의존. PoC 단계라 OK. |
| `rfd` (파일 다이얼로그) | true external | `open_file_dialog` 가 직접 호출. 얕음 — `FilePicker` port 후보 (2 어댑터: native + headless test). |
| 파일시스템 (WZ 로딩) | local-substitutable | `WzArchive::open` 이 직접 fs. 테스트는 합성 코퍼스로 대체. ✅ |

---

## 5. 실행 계획 (우선순위)

### P0 (즉시, 가치 높음)
1. **`wz-image::decode_png_ref_to_rgba` 추가** + GUI `load_back_image` 를 이 함수로 교체. (§3.3)
2. **`WzArchive` 에 `build_arena()` / `read_tree_flat()` 추가**, CLI/GUI 의 `build_arena_from_archive` 를 `wz-db` 로 이동. (§3.5)

### P1 (GUI 완성 전)
3. `wz-gui` 를 `panels/` + `state.rs` + `async_io.rs` + `render_bridge.rs` 로 분할. (§3.2)
4. `wz-cli` 를 `commands/` 로 분할. (§3.1)

### P2 (기능)
5. `wz-avatar` 에 WZ→avatar 로더 추가 (별도 티켓).
6. `FilePicker` port (native + headless) — 테스트 가능하게.

### P3 (CI/품질)
7. GUI 빌드를 CI `rust-gates` 에 `--no-default-features` 로 추가.
8. `cargo clippy` 0 경고 유지 (현재 wz-gui 에 dead_code 경고 1개).

---

## 6. 삭제 테스트 결과

- `cmd_avatar` 의 합성 레이어 로직: 삭제하면 avatar 데모가 사라짐 → 가치 있음. 하지만 `wz-avatar` 크레이트가 WZ 로더를 갖추면 이 합성 로직은 테스트 픽스처로 이동해야 한다.
- `build_arena_from_archive`: CLI/GUI 양쪽에서 중복. 삭제하면 둘 다 깨짐 → 가치 있음. `wz-db` 로 이동이 맞다.
- `load_back_image` (GUI): `wz-image` 고수준 함수가 생기면 삭제 가능 → 현재는 pass-through 에 가까움. P0 에서 제거.

---

## 7. PoC 3.5 (아바타) 상태

아바타 WZ 로더가 코드베이스에 **존재하지 않는다**. `AvatarScene`/`Bone`/`SkinLayer` 는 데이터 구조만 있고, WZ → avatar 변환은 미구현. CLI `cmd_avatar` 는 합성 레이어를 쓴다.

따라서 PoC 3.5 의 "실제 장비 로딩"은 새 기능 구현이지 GUI 와이어링이 아니다. 별도 티켓으로 분리하고, GUI 쪽은 데모 렌더(합성 아바타)를 표시하도록 유지한다. 이는 아키텍처 감사 §3.4 와 일치한다.

---

## 8. 결론

- **깊은 모듈**: `wz-core`, `wz-pool`, `wz-render`, `wz-db`, `wz-error` — 인터페이스가 작고 구현이 많다. ✅
- **얕은 모듈**: `wz-cli` (3026줄 단일 파일), `wz-gui` (853줄 단일 파일), `wz-image` (decode 조합 노출).
- **P0 2건** (`decode_png_ref_to_rgba`, `WzArchive::build_arena`) 을 이번 세션에서 실행한다.
- P1~P3 는 후속 세션.
MD
echo "written"
---

## 9. PoC 3.9 / WZ→avatar loader — 블로커 기록

`Character/_Canvas/_Canvas_000.wz` 의 이미지 노드 (예: `00002000.img`) 는
`FS_Guitar_Stand_M` → `0` → `body` (Canvas) 구조만 가진다. **origin/anchor/z/map
프로퍼티가 없다.** C# `CharaSim`/`Avatar` 렌더러가 이 값을 별도로 계산하거나
다른 WZ (Base/zmap.img) 에서 가져온다.

따라서 "앵커/프리즘/zmap 정밀도" 는 C# 참조 구현 없이 구현할 수 없다.
현재 로더 (`wz-avatar::loader`) 는 PoC 수준으로 이미지 캔버스를 로드해
`SkinLayer` (width/height) + `Bone` + `ZMap` + `AvatarScene` 을 조립하고
렌더한다. 정밀도 향상은 C# 소스 확보 후 별도 티켓.

### PoC 3.9 (29슬롯 선택 UI) 상태

GUI 아바타 패널에 29슬롯 목록 + 10개 로드된 파트에 체크박스 (렌더 대상 선택) +
'아바타 로드 (loader, 10 파트)' + '아바타 렌더 (오프스크린, 10 파트)' 버튼 +
Body 이미지/렌더 결과 나란히 표시. 29슬롯 드롭다운(장비 선택) 은 장비 ID →
이미지 매핑 로직 (C# CharaSim) 이 필요해 미구현.

---

## 10. WZ→avatar 로더 — WZ 포맷 조사 결과 (2026-11-03)

`Character/_Canvas/_Canvas_000.wz` 의 이미지 노드 (예: `00002001.img`) 구조:

```
00002001.img (Property)
  alert (Property)
    0 (Property)
      arm (Canvas, WxH)
      body (Canvas, WxH)
      lHand (Canvas, WxH)
      rHand (Canvas, WxH)
    1 (Property)
      ...
```

**origin/map/z 프로퍼티가 없다.** 캔버스 노드에 children 이 없고, 프레임 노드에도
origin/map/z 가 없다. CanvasHeader.scale 도 0이다.

C# `WzComparerR2` 의 `CharaSim`/`Avatar` 렌더러가 앵커(navel, brow, neck)와
z-index, prism 을 **별도로 계산**하거나 `Base/zmap.img` 에서 가져온다.

### 결론

- WZ 포맷은 **픽셀 데이터만** 저장한다. 앵커/프리즘/zmap 은 WZ 에 없다.
- C# `CharaSim` 소스 (`WzComparerR2/CharaSim/`, `WzComparerR2/AvatarCommon/`) 가
  이 계산 로직을 구현한다.
- **현재 Rust 포트에 이 로직이 없고**, C# 참조 소스가 없어 포팅 불가.
- 현재 로더 (`wz-avatar::loader`) 는 PoC 수준: 이미지 캔버스를 로드해
  `SkinLayer` (width/height) + `Bone` + `ZMap` + `AvatarScene` 을 조립하고 렌더한다.
  앵커/프리즘/zmap 정밀도는 C# 참조 확보 후 별도 티켓.

### 사용 가능한 C# 참조 (공개)

[`seotbeo/WzComparerR2`](https://github.com/seotbeo/WzComparerR2) (`seotbeo_main` 브랜치):
- `WzComparerR2/AvatarCommon/` — Bone, Skin, AvatarCanvas, AvatarCanvasManager, Action, ActionFrame, Prism, PrismData
- `WzComparerR2/CharaSim/` — CharaProp, CharaSim 등
- `WzComparerR2.Avatar/` — UI 레이어

이 소스를 Rust 포트에 통합하면 앵커/프리즘/zmap 정밀도를 구현할 수 있다.

---

## 11. 2026-11-05 후속 — 공개 전 심화 결과

이번 패스에서 실제로 적용한 deepening 과 남은 후보를 기록한다.

### 적용됨

| 후보 | 변경 | 검증 |
|---|---|---|
| `StringLinker` 다섯 루트 | `StringRoot`/`StringRoots` + `load_roots` 한 진입점. tooltip 이 `--item-wz`/`--etc-wz` 로 Item/Etc 를 실제 로드 | `load_roots_loads_optional_quest_root_in_one_call`, 54 wz-db tests |
| Quest/Reactor 루트 | `load_quest_wz` (QuestData/QuestInfo.img, name/0/demand.base), `load_reactor_wz` (info/viewName/name/info) | 합성 트리 테스트 2개 |
| `expr` JSON 숫자 | `json_number` 로 2^63 경계를 배타적으로 처리, NaN/Infinity → null | 단위 테스트 5개 |
| oracle replay | `.code` 누락을 exit 0 이 아니라 하네스 오류로 처리 | 회귀 테스트 1개 |
| `WorldArchiveEntry` | 8-인자 data clump → struct 하나 | `cargo clippy -D warnings` |
| `AvatarScene` | `Deref`/`DerefMut` 제거, `canvas` 필드 private, `canvas()`/`canvas_mut()` 단일 접근 경로 | workspace 테스트 |
| `mapscene` 출력 | `ladders_ropes`/`backgrounds`/`objects`/`tiles` 및 `kind` 필드 추가. `LadderRopeKind`/`LifeKind` 가 실제 소비됨 | `cargo test --workspace` |
| `StringDomain` | 도메인별 `get`/`format_tooltip_in` + `tooltip --domain`, 충돌 회귀 테스트 | 56 wz-db tests |
| GUI 다중 루트 | GUI StringLinker 가 String/Item/Etc/Quest/Reactor 를 `WZ_DATA_ROOT` 에서 로드 | GUI build/clippy |
| 장비/무기/믹스/이펙트 | `load_item_frame`·`merge_bone_tree`, Hair/Face `MixOptions`, `TamingMob`, `ItemEff` 첫 프레임, GUI 프리즘/믹스 컨트롤 | 실데이터 `WZ_DATA_ROOT` 26 tests, GUI build |

### 남은 후보

- **WZ-only customChair 타입 동작** — C# `CustomChairType` 은 `Unsupported`/`ScaleAvatarChair` 두 개뿐이다. Rust 로더는 C# 공통 규칙(`sitAction`, `removeBody`, `fixFrameIdx`, `tamingMob`, `info/scale`)을 포팅했고, C#에 없는 `flipXYChair`/`rotatedSleepingBagChair`/`dummyAvatarChair`는 WZ 스키마 기반 확장으로 플립/회전/더미 taming을 적용한다. `rollingHouseChair`는 `_outlink` 로 8개 파츠를 해석하고, 모든 프레임/딜레이를 로드해 GUI 에서 슬라이더/자동 재생한다. `androidChair`는 `customEffect` 16프레임을 같은 체어 애니메이션 경로로 재생하고, `petChair`는 슬롯/타이밍을 파싱한다(펫 스프라이트는 장착 펫 데이터 필요). `eventPointChair`는 `index`/`point`/`term`을 GUI에 표시하고, `hashTagChair`는 첫 action 포즈를 적용하며, `mirrorChair`는 무기를 숨긴다. `2019rollingHouseChair`/`yetiChair`는 `part/N` 애니메이션을 재생한다. `identityChair`는 레벨 숫자 스프라이트를 렌더하고, `scoreChair`는 점수 구간별 이펙트를 재생한다. `popChair`/`popButtonChair`는 pop 수치를, `timeChair`는 `HH:MM`을 내장 5x7 폰트로 렌더한다. `outfitVoteChair`는 투표 점수를 폰트로 렌더하고, `bgmSelectionChair`는 버튼 영역/taming 메타데이터를 로드한다. `showcaseChair`는 GUI에서 받은 장비 ID를 `Character/<slot>`에서 로드해 4개 슬롯에 배치한다. 이름/직업/성별/길드는 WZ `Font.img/NotoSansMP-Medium` TTF를 `fontdue`로 래스터화해 렌더한다(폰트는 사용자 Data에서 읽고 번들하지 않음).
- **오라클 하네스 CI** — 공개 C# 트리에 `WzComparerR2.Oracle` 프로젝트가 없어 수동 workflow 입력이 필요.

### 산출물

- `GLOSSARY.md` — WZ/문자열/아바타/맵/검증 도메인 용어.
- `docs/adr/0001`~`0006` — 오라클 외부 핀, Gate/Advisory/Uncomparable, GUI 워크스페이스 제외, 게임 데이터 미번들, 결정론, 도메인별 문자열 조회.
