# Phase 8 데스크톱 GUI 아키텍처 설계 (`docs/gui-architecture.md`)

> 이슈 #32 — "Phase 8 데스크톱 GUI 레이어 (WzComparerR2 데스크톱 앱) 기획 및 아키텍처 설계"
> 산출물: `docs/gui-architecture.md` (본 문서) + PoC 프로토타입 계획.
>
> 전제: 헤드리스 CLI 코어(`wz-cli`)와 `wz-render`(wgpu 2D 렌더러)는 완성됐고,
> `docs/render-spike.md` 가 **egui + wgpu 착수 정당성**을 실측으로 확인했다.
> 본 문서는 그 위에 얹을 **데스크톱 GUI 의 기술 스택과 구조**를 고정한다.

---

## 1. 목적과 범위

### 1.1 목적

WzComparerR2 의 Rust 포트를 **헤드리스 CLI 도구에서 데스크톱 GUI 애플리케이션**으로
확장한다. GUI 는 CLI 를 **대체하지 않는다** — CLI 의 모든 기능(오라클, 차분, 코퍼스)은
그대로 두고, 시각적 브라우징/검증 UX 를 더한다.

### 1.2 범위 (In-scope)

- WZ 노드 트리 브라우저 (가상 스크롤)
- 이미지/스프라이트 뷰어 + 프로퍼티 인스펙터
- 맵 렌더러 뷰포트 (wgpu 서페이스, 카메라 팬/줌)
- 아바타 코디네이터 패널 (29슬롯 + 실시간 렌더)
- 비동기 파싱/로딩, 채널 기반 백그라운드 작업
- PNG 내보내기, 클립보드 복사, 오라클 JSON 내보내기

### 1.3 비범위 (Out-of-scope)

- GUI 에서 WZ **편집/저장**. 이 포트는 읽기 전용이다(`WzArchive` 에 쓰기 경로 없음).
- 웹/Electron 포트. 네이티브 데스크톱만.
- 멀티 윈도우/도킹 레이아웃의 완전한 IDE 스타일. v1 은 단일 윈도우 + 패널 분할.
- C# `WzComparerR2` 와의 픽셀 1:1 재현. GUI 는 Rust 코어의 의미론을 따른다.

---

## 2. GUI 툴킷 선정

### 2.1 후보 비교

| 항목 | egui (eframe) + wgpu | iced | slint |
|---|---|---|---|
| 아키텍처 | 즉시 모드 (immediate) | Elm (반응형) | 선언형 (DSL) |
| wgpu 통합 | 네이티브 (egui-wgpu) | wgpu 백엔드 내장 | 자체 렌더러, wgpu 옵션 |
| Rust 생태 | 성숙, 대규모 사용 | 성장 중 | 상용/라이선스 |
| 학습 곡선 | 낮음 | 중간 | DSL 별도 |
| 커스텀 wgpu 서페이스 임베딩 | ✅ `egui_wgpu::Callback` / `PaintCallback` | ⚠️ possible | ⚠️ 제한적 |
| 한국어 IME | egui 0.29+ 에서 개선 중 (§10.2) | 제한적 | 양호 |
| 라이선스 | MIT/Apache-2.0 | MIT | GPLv3/상용 |
| Cargo.lock 부담 | +egui/eframe/winit | +iced | +slint |

### 2.2 결정: **eframe(egui) + wgpu (native backend)**

근거:
1. **wgpu 서페이스 임베딩이 1급 시민**이다. 맵 뷰어는 `wz-render` 의 `RenderContext` 를
   그대로 써야 하고, egui 의 `PaintCallback` 으로 서브렉트에 wgpu 드로우를 끼워 넣을 수 있다.
   iced/slint 는 이 경로가 우회적이다.
2. **즉시 모드가 WZ 브라우저에 맞는다.** 노드 트리는 매 프레임 다시 그리는 모델이
   가상 스크롤과 잘 맞고, 상태는 `Arena`/`NodeId` 가 이미 들고 있다.
3. **라이선스가 MIT** 이고, 렌더 스파이크가 이미 wgpu 30 파이프라인을 검증했다.
4. 보고서 §9.3 와 `docs/render-spike.md` 가 이미 이 방향을 권고했다.

### 2.3 위험

- egui + wgpu 30 조합은 **실측 미검증**이다 (render-spike 는 wgpu 만 검증).
  PoC 0 단계에서 먼저 확인한다 (§9).
- egui 의 텍스트 레이아웃이 한국어/한자 폰트(메이플스토리 폰트)에서
  깨질 수 있다. 폰트 임베딩 전략이 필요하다 (§10.2).
- egui 는 "네이티브 룩" 이 아니다. 메뉴바/다이얼로그는 egui 의 `MenuBar`/`Window` 로
  충분하지만, OS 네이티브 파일 다이얼로그는 `rfd` (native-dialog) 로 별도 처리한다.

---

## 3. 크레이트 구조

### 3.1 새 크레이트 `wz-gui`

```
crates/wz-gui/
├── Cargo.toml
├── src/
│   ├── main.rs              # eframe::run_native 진입점
│   ├── app.rs               # WzApp (eframe::App)
│   ├── state.rs             # AppState (모든 패널 공유)
│   ├── panels/              # 각 패널 모듈
│   │   ├── wz_tree.rs
│   │   ├── image_viewer.rs
│   │   ├── map_viewport.rs
│   │   ├── avatar_panel.rs
│   │   ├── property_inspector.rs
│   │   └── status_bar.rs
│   ├── async_io.rs          # 채널/스레드 풀
│   ├── render_bridge.rs     # wz-render ↔ egui PaintCallback
│   └── fonts.rs             # 폰트 임베딩
└── assets/                  # 아이콘, 폰트
```

### 3.2 의존성 (workspace 에 추가)

```toml
# crates/wz-gui/Cargo.toml
[dependencies]
wz-core.workspace = true
wz-image.workspace = true
wz-render.workspace = true
wz-db.workspace = true
wz-avatar.workspace = true
wz-diff.workspace = true
eframe = { version = "0.30", features = ["wgpu"] }   # PoC 0 에서 버전 고정
egui = "0.30"
egui-wgpu = "0.30"
wgpu.workspace = true
rfd = "0.15"          # 네이티브 파일 다이얼로그
crossbeam-channel = "0.5"
```

> **버전 고정은 PoC 0 (§9) 에서 먼저 한다.** egui 0.30 + wgpu 30 조합이
> 컴파일되는지 실측으로 확인한 뒤 `Cargo.lock` 에 박는다. render-spike 가 밝힌
> "미검증 항목 #1" 이다.

### 3.3 ADR-4 준수

`wz-render` 는 wgpu 타입을 크레이트 경계 밖으로 내보내지 않는다 (`GpuKey`/`GpuSlot`).
`wz-gui` 는 `wz-render` 의 공개 API 만 쓰고, egui 의 wgpu 컨텍스트는
`render_bridge.rs` 한 파일에 격리한다. wgpu 타입이 `panels/` 로 새지 않게 한다.

### 3.4 워크스페이스 등록

`Cargo.toml` 의 `members` 에 `"crates/wz-gui"` 추가. `default-members` 에는 넣지
않는다 — GUI 는 `cargo build -p wz-gui` 로만 빌드하고, CI `rust-gates` 는
헤드리스(`--no-default-features` 또는 `wz-gui` 제외) 로 돌린다. wgpu GUI 빌드는
CI 매트릭스를 비싸게 만들지 않기 위함이다.

---

## 4. 코어 서브시스템 연동

### 4.1 WZ 노드 트리 브라우저 (가상 스크롤)

**데이터 소스**: `wz_db::arena::Arena` + `NodeId`.

- `Arena::from_wz_node` 로 WZ 트리를 평탄 슬롯 배열로 변환한다. `NodeId` 는
  `{ index, generation }` 이라 노드가 release/reuse 되어도 핸들이 안전하다.
- GUI 는 `Arena` 를 **읽기 전용 뷰**로 쓴다. 노드 선택/확장 상태는 GUI 가
  `HashSet<NodeId>` / `BTreeMap<NodeId, bool>` 로 들고, `Arena` 는 불변으로 유지한다.
- `read_tree_with_mode(shallow)` 로 최상위만 먼저 로드하고, 사용자가 노드를
  펼칠 때 `descend_image`/`ingest_image` 로 하위를 지연 로드한다 (§5).

**가상 스크롤 구현**:
- egui 의 `ScrollArea::vertical().show_rows(ui, row_height, total_rows, |ui, range| …)` 를 쓴다.
- `total_rows` 는 현재 확장 상태 기준으로 `Arena` 를 DFS 하지 않고, GUI 가 별도로
  유지하는 `Vec<NodeId>` (보이는 행만) 로 계산한다. 노드 expand/collapse 시 이 벡터를
  재구성한다 (O(변경된 서브트리)).
- 행 높이는 고정(예: 18px)으로 단순화한다. 가변 높이는 v2.

**검색**: `wz_db::query::QueryEngine` 을 그대로 쓴다. `DEFAULT_MAX_RESULTS = 5000` 을
GUI 는 1000 으로 낮춰서 UI 를 보호한다. 결과는 `NodeId` 목록으로 받아 트리에서
해당 노드로 스크롤/선택한다.

### 4.2 이미지/스프라이트 뷰어 + 프로퍼티 인스펙터

**이미지 뷰어**:
- 선택된 노드가 `ImageValue::Png(PngRef)` 이면 `WzArchive::parse_image_at(offset)` 로
  `Node` 를 파싱하고, 캔버스 노드를 찾아 `wz_image` 로 디코딩 → RGBA8 →
  `wz_render::RenderContext::create_texture` 로 GPU 텍스처 업로드 → egui `Image` 위젯.
- 다중 페이지(`pages > 1`)는 좌/우 화살표 또는 스크롤로 페이지 전환. 페이지 인덱스는
  GUI 상태.
- 스케일(`scale`) 은 표시 크기에 반영한다 (디코딩된 픽셀 크기 × scale).

**프로퍼티 인스펙터**:
- 선택 노드의 `ImageValue` 를 재귀적으로 펼쳐 보여준다. `Property` 노드는 children 을
  중첩으로, 스칼라는 값 그대로, `Uol` 은 링크 대상 노드로 점프 버튼.
- `StringLinker` 로 문자열 ID 를 해석해 `tooltip` 텍스트를 옆에 표시한다
  (`docs/oracle-spec.md` 의 tooltip 계약과 동일한 `name` + `\n` + `desc`).

### 4.3 맵 렌더러 뷰포트

**wgpu 서페이스 임베딩**:
- `wz-render` 의 `RenderContext` 는 현재 `new_headless()` 만 제공한다 (오프스크린).
  GUI 에서는 egui 의 `PaintCallback` 안에서 `wgpu::Surface` 를 만들고, `RenderContext` 의
  내부 device/queue 를 **재사용**한다. 이를 위해 `RenderContext` 에 `new_for_surface(surface)`
  생성자 또는 `device()`/`queue()` 접근자를 **추가**해야 한다 (§9 PoC 0 의 첫 작업).
- 맵 렌더링 파이프라인은 C# `WzComparerR2.MapRender` 의 EffectResources (light/waterBack/
  waterFront 셰이더) 를 이미 `wz-render` 가 포팅했다 (render-spike §2-3). 맵 뷰어는
  이 파이프라인을 그대로 쓴다.

**카메라**:
- `wz_render::camera::Camera2D` (ortho, zoom, follow/lerp, screen↔world) 를 재사용한다.
- 마우스 휠 = 줌, 드래그 = 팬. `Camera2D` 의 `screen_to_world` 로 클릭 picking.
- 줌 범위/초기 뷰는 맵의 `info` 노드에서 읽는다 (C# 과 동일).

**렌더 루프**:
- egui 는 매 프레임 `update()` 를 부른다. 맵 뷰어가 활성일 때만 `RenderContext::render_to_surface`
  (신규) 를 호출한다. 비활성 시 CPU 비용 0.
- `SpriteBatch` 빌드는 맵 데이터가 바뀔 때만 다시 한다 (캐시). 카메라만 바뀌면
  uniform buffer 만 업데이트한다.

### 4.4 아바타 코디네이터 패널

**슬롯 UI**: 29개 `AvatarSlot` 을 2열 그리드로 표시. 각 슬롯은 드롭다운(장비 ID 입력
또는 WZ 트리에서 드래그 앤 드롭) + 썸네일(장비 아이콘).

**렌더링**:
- `AvatarScene::generate_layers` → `AvatarBatch` → `into_sprite_batch(texture_key)` →
  `RenderContext` 오프스크린 렌더 → egui `Image`. 이 경로는 `wz oracle avatar` 가 이미
  쓰는 경로와 동일하므로 **오라클과 GUI 가 같은 픽셀을 본다**.
- 프리즘/프레임 타이밍은 `FrameTiming` + `apply_prism_rgba8` 로 처리한다. GUI 에서는
  애니메이션 프레임을 `std::time::Instant` 기반으로 자동 진행하고, 일시정지/프레임
  스텝 버튼을 둔다.
- Z-Map 정렬은 `ZMap::compute_priority` 가 처리한다. GUI 는 정렬 결과를 건드리지 않는다.

**상호작용**: 슬롯 변경 → `AvatarScene` 재빌드 → 렌더. 디바운스(100ms)로 빠른
연속 변경 시 중간 렌더를 건너뛴다.

---

## 5. 비동기 I/O 및 백그라운드 파싱

### 5.1 문제

WZ 아카이브는 수백 MB ~ 수 GB. `WzArchive::open` + `read_tree` 는 수 초 걸린다.
UI 스레드에서 돌리면 프리즈 + "응답 없음" 으로 OS 가 앱을 죽인다.

### 5.2 모델: `crossbeam-channel` + 전용 스레드 풀

```
┌────────────┐  LoadRequest   ┌──────────────┐  LoadResult   ┌────────────┐
│  UI thread │ ─────────────► │ worker pool  │ ─────────────► │  UI thread │
│  (egui)    │   (mpsc)       │ (rayon or    │   (mpsc)      │  (egui)    │
└────────────┘                │  std::thread)│               └────────────┘
```

- **요청 채널**: `crossbeam_channel::unbounded::<LoadRequest>()`.
- **결과 채널**: `crossbeam_channel::unbounded::<LoadResult>()`.
- `LoadRequest` 는 enum: `OpenArchive(path)`, `ReadTree(shallow)`, `ParseImage(offset)`,
  `DecodeTexture(...)`, `RenderMap(...)`, `RenderAvatar(...)`.
- `LoadResult` 는 `Result<LoadedData, LoadError>` + `request_id` (u64) 로 stale 결과를
  버린다.

### 5.3 워커 풀

- `std::thread::spawn` 으로 2~4개 워커 (CPU 코어 수의 절반, 최소 2).
- `rayon` 은 **쓰지 않는다** — rayon 의 글로벌 풀은 GUI 의 짧은 요청과 충돌하고,
  취소/우선순위가 어렵다. 전용 스레드가 더 예측 가능하다.
- 각 워커는 `recv()` 로 요청을 받고, 처리 후 결과 채널로 보낸다. `select!` 로
  취소 토큰도 함께 본다.

### 5.4 취소와 stale 결과

- 사용자가 다른 파일을 열면 이전 요청은 **취소**해야 한다. `CancellationToken` (또는
  요청 ID 기반 필터링) 로 처리한다. 워커는 취소를 폴링 지점(청크 경계)에서 확인한다.
- 결과 채널에서 `request_id` 가 현재 세대와 다르면 **무시**한다. UI 는 stale 데이터를
  절대 적용하지 않는다.
- `WzArchive` 는 `Send + Sync` 여야 한다 (`wz-pool` 의 byte source 는 `Send+Sync`).
  `WzArchive` 가 `Send` 인지 확인 필요 — 아니라면 `Arc<Mutex<WzArchive>>` 로 감싼다.

### 5.5 진행률 / 에러 표시

- `LoadResult::Progress(f32)` 변형으로 진행률을 스트리밍한다.
- 에러는 `LoadError` enum 으로 분류: `Io`, `Format`, `Crypto`, `Cancelled`, `NotFound`.
- 상태바(`status_bar` 패널) 가 현재 작업과 진행률을 표시한다.

---

## 6. 상태 관리

### 6.1 `AppState` (단일 진실 소스)

```rust
pub struct AppState {
    pub archive: Option<Arc<WzArchive>>,          // 열린 WZ
    pub arena: Option<Arena>,                     // 평탄 노드 저장소
    pub selected: Option<NodeId>,                // 선택된 노드
    pub expanded: BTreeSet<NodeId>,               // 펼친 노드
    pub visible_rows: Vec<NodeId>,                // 가상 스크롤 행
    pub image_cache: LruCache<NodeId, Arc<RgbaImage>>,  // 디코딩된 이미지
    pub avatar: AvatarState,                     // 29슬롯 + 렌더 결과
    pub map_viewport: MapViewportState,           // 카메라, 선택된 맵
    pub jobs: JobTracker,                        // 진행 중 요청
    pub search_query: String,
    pub search_results: Vec<NodeId>,
}
```

### 6.2 데이터 흐름 (단방향)

```
User input (egui)
  → AppState mutation intent
  → LoadRequest enqueue (async_io)
  → worker 처리
  → LoadResult (mpsc)
  → AppState 갱신 (UI thread, egui update())
  → egui 렌더
```

UI 스레드는 **절대** 블로킹 I/O 를 하지 않는다. 모든 파싱/디코딩/렌더링은
워커 또는 `RenderContext` (GPU) 에서 한다. GPU 호출은 UI 스레드에서 하되
(egui 의 render pass 와 같은 스레드), 시간이 오래 걸리지 않는다 (§4.3).

---

## 7. 렌더링 통합

### 7.1 오프스크린 vs 서페이스

| 용도 | 방법 |
|---|---|
| 아바타 썸네일/미리보기 | `RenderContext::render_to_rgba` (오프스크린) → PNG/egui Image |
| 맵 뷰포트 | `wgpu::Surface` 에 직접 렌더 (신규 `render_to_surface`) |
| 이미지 뷰어 | CPU 디코딩 → egui Texture (GPU 업로드는 egui 가 관리) |

### 7.2 아틀라스/배치

- `AtlasShelfPacker` 로 아바타/맵 스프라이트를 아틀라스 페이지로 패킹한다.
- `SpriteBatch` 로 드로우 콜을 모은다. 텍스처/블렌드 단위 batch break 는
  render-spike §3 의 결론과 정렬된다.
- 셰이더 전환은 bind group 1 교체 한 번으로 끝난다 (render-spike §3).

### 7.3 셰이더

- `wz-render` 의 WGSL (`common.wgsl` + `sprite.wgsl` + `maplestory.wgsl`) 을 그대로 쓴다.
- `inter_stage_layout()` 기계 검증을 CI 에서 계속 돌린다.

---

## 8. 키바인딩 / UX / IME

### 8.1 키바인딩 (초안, v1)

| 키 | 동작 |
|---|---|
| `Ctrl+O` | WZ 파일 열기 (rfd 네이티브 다이얼로그) |
| `Ctrl+F` | 노드 검색 포커스 |
| `F5` | 현재 뷰 새로고침 (재파싱) |
| `Space` | 아바타 애니메이션 재생/일시정지 |
| `←/→` | 아바타 프레임 스텝 |
| `Ctrl+S` | 현재 이미지 PNG 저장 |
| `Ctrl+Shift+J` | 선택 노드를 JSON (오라클 형식) 으로 클립보드 복사 |

### 8.2 한국어/IME

- egui 의 IME 지원은 0.29+ 에서 개선됐지만 **메이플스토리 폰트(한글/한자)** 는
  시스템에 없을 수 있다. `fonts.rs` 에서 Noto Sans KR + 메이플스토리 폰트를
  임베딩한다 (바이너리 크기 +~10MB).
- IME 조합 문자열이 egui `TextEdit` 에 제대로 들어오는지는 PoC 0 에서 실측한다
  (render-spike 의 "미검증 항목" 목록에 있음).

### 8.3 접근성

- 모든 인터랙티브 요소에 `ui.label` + tooltip.
- 키보드만으로 트리 탐색 가능 (↑/↓/←/→/Enter/Space).
- 고대비 모드는 egui 의 `Visuals::dark()`/`light()` 토글로 지원.

---

## 9. PoC 프로토타입 계획

### 9.0 PoC 0 — "egui + wgpu 30 가 컴파일되고 뜨는가" (1~2일)

**목표**: 빈 윈도우 + wgpu 서페이스가 뜨는 최소 바이너리.

- [ ] `crates/wz-gui` 생성, `Cargo.toml` 에 eframe/egui/wgpu 추가
- [ ] `eframe::run_native` + `wgpu` backend 로 빈 윈도우
- [ ] `RenderContext` 에 `new_for_surface` 또는 `device()`/`queue()` 접근자 추가
- [ ] egui `PaintCallback` 안에서 `wz-render` 의 삼각형 1개 그리기 (render-spike 코드 재사용)
- [ ] **게이트**: `cargo build -p wz-gui` 성공, 윈도우가 뜨고 삼각형이 보임
- [ ] IME 테스트: `TextEdit` 에 한글 입력

**실패 시 대안**: egui-wgpu 가 wgpu 30 과 안 맞으면 `winit` + `wgpu` 직접 + egui 를
`egui-wgpu` 없이 수동 통합. (가능하지만 작업량 +50%.)

### 9.1 PoC 1 — WZ 트리 브라우저 (3~5일)

- [ ] `rfd` 로 WZ 파일 열기 → `WzArchive::open` (워커 스레드)
- [ ] `Arena::from_wz_node` → 가상 스크롤 트리 뷰
- [ ] 노드 선택 → 프로퍼티 인스펙터
- [ ] 이미지 노드 → 이미지 뷰어 (CPU 디코딩 + egui Texture)
- **게이트**: 792개 WZ 중 하나를 열어 트리를 스크롤/검색할 수 있다.

### 9.2 PoC 2 — 맵 뷰포트 (5~7일)

- [ ] 맵 선택 → `MapData` 파싱 → `wz-render` 로 서페이스 렌더
- [ ] 카메라 팬/줌, foothold 오버레이
- [ ] 셰이더 3종(light/waterBack/waterFront) 임베딩
- **게이트**: 실제 맵 1개를 60fps 로 렌더하고 팬/줌이 부드럽다.

### 9.3 PoC 3 — 아바타 패널 (3~5일)

- [ ] 29슬롯 UI + 장비 선택
- [ ] `AvatarScene` → 오프스크린 렌더 → egui Image
- [ ] 프리즘/프레임 애니메이션
- **게이트**: 장비 교체 시 100ms 이내에 새 아바타가 보인다.

### 9.4 v1 출시 기준

- PoC 0~3 모두 통과
- `cargo test --workspace` + `cargo clippy` 가 GUI 크레이트 포함해도 통과
- CI `rust-gates` 에 GUI 빌드를 **추가하지 않는다** (wgpu GUI 빌드 비용). 대신
  `wz-gui` 는 `--no-default-features` (헤드리스) 로 CI 에서 컴파일만 확인한다.

---

## 10. 위험과 미검증 항목

### 10.1 렌더 스파이크에서 이미 확인된 것

- ✅ wgpu 30 어댑터/파이프라인/드로우/되읽기 (Vulkan, AMD)
- ✅ inter-stage ABI (`common.wgsl` + `assemble()`)
- ✅ bind group 분리 (group 0 정점 / group 1 픽셀)
- ✅ 버텍스 스트라이드 40 바이트

### 10.2 미검증 (PoC 0 에서 반드시 확인)

1. **egui 0.30 + wgpu 30 조합 컴파일** — render-spike 는 wgpu 만 썼다.
2. **IME/한글 입력** — egui 의 IME 가 이 플랫폼에서 동작하는지.
3. **메이플스토리 폰트 렌더링** — 폰트 폴백, 한자/특수문자.
4. **`RenderContext` 서페이스 모드** — `new_headless()` 만 있어서 `new_for_surface`
   또는 `device()`/`queue()` 접근자를 추가해야 한다. wgpu 30 의 `Surface` 생성은
   `winit` 의 `Window` 핸들이 필요하므로, `wz-render` 가 `winit` 에 의존하지
   않게 설계해야 한다 (ADR-4).
5. **GPU 메모리 예산** — 맵 뷰어에서 아틀라스 페이지가 누적될 때. `wz-pool` 의
   512MB 바이트 예산 개념을 GPU 텍스처에도 적용할지 결정 필요.
6. **CI 매트릭스** — GUI 빌드는 macOS/Linux/Windows 각각에서 wgpu 백엔드가
   다르다(Vulkan/Metal/DX12). 빌드만이라도 3 OS 에서 확인해야 한다.

### 10.3 아키텍처 위험

- **`WzArchive: Send` 여부 미확인.** 워커 스레드로 넘기려면 `Send` 가 필요하다.
  아니라면 `Arc<Mutex<>>` 로 감싸거나, 워커가 `WzArchive` 를 소유하고 결과만
  보낸다(후자가 더 단순).
- **Arena 의 mutable 공유.** UI 가 `Arena` 를 읽는 동안 워커가 ingest 하면 data race.
  `Arc<RwLock<Arena>>` 또는 "워커가 새 Arena 를 만들어 통째로 교체" 패턴.
  후자를 권장한다 (lock contention 없음).
- **egui 즉시 모드 + 대용량 트리.** 매 프레임 전체 트리를 순회하면 10만 노드에서
  느려진다. 가상 스크롤(§4.1)이 필수이며, `visible_rows` 캐시를 반드시 쓴다.
- **맵 렌더링 60fps 유지.** `SpriteBatch` 재빌드가 매 프레임 일어나면 안 된다.
  카메라만 바뀔 때는 uniform buffer 만 업데이트한다.

---

## 11. 수용 기준 (v1)

1. `cargo build -p wz-gui` 가 macOS/Linux/Windows 에서 통과한다.
2. `cargo test --workspace` 와 `cargo clippy --workspace --all-targets -- -D warnings` 가
   GUI 크레이트 포함해도 통과한다 (GUI 는 `--no-default-features` 로 CI 에서 컴파일만).
3. PoC 0~3 (§9) 의 게이트가 모두 통과한다.
4. WZ 파일 1개를 열어 트리 탐색 + 이미지 뷰 + 맵 뷰 + 아바타 렌더가 동작한다.
5. 100MB 이상 WZ 파일을 열 때 UI 가 100ms 이상 블로킹되지 않는다 (§5).
6. 오라클(`wz oracle`) 과 GUI 가 같은 픽셀을 본다 (아바타 SHA-256 일치).

---

## 12. ADR 후보 (별도 ADR 로 확정 필요)

- **ADR-5**: GUI 툴킷 = eframe(egui)+wgpu. (본 문서 §2 의 결정을 ADR 로 고정)
- **ADR-6**: `RenderContext` 의 서페이스 모드 추가 방식 (`new_for_surface` vs
  `device()`/`queue()` 노출). ADR-4 의 "wgpu 타입이 경계를 넘지 않는다" 와의
  정합성.
- **ADR-7**: 워커 모델 (전용 스레드 + crossbeam-channel) vs rayon. (§5.3)
- **ADR-8**: GUI 크레이트를 CI `rust-gates` 에서 제외하는 정책. (§9.4)

---

## 13. 참고 문서

- `docs/render-spike.md` — wgpu 실측 결과, 결함 2개와 수정
- `docs/shader-audit.md` — 셰이더 ABI/바인딩 그룹 계약
- `docs/known-bugs.md` — GUI 관련 남은 작업 (§"남은 작업" 5번)
- `docs/oracle-spec.md` — 오라클 JSON 계약 (GUI 의 "JSON 으로 복사" 기능이 따름)
- `crates/wz-render/src/lib.rs` — ADR-4 레이어링 규칙, 공개 타입 목록
- `crates/wz-avatar/src/scene.rs`, `crates/wz-db/src/arena.rs` — GUI 가 바인딩할 코어 API
- 이슈 #33 — CI 파이프라인 (GUI 빌드는 CI 에서 제외)
MD
echo "written"; wc -l docs/gui-architecture.md