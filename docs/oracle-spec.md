# C# 오라클 하네스 — 스펙

리포트 §10.2. **이 프로젝트 성공 여부를 결정하는 문서.**

## 왜 이게 최우선순위인가

원본 C# 저장소에 테스트가 **0개**다 (132k LOC, 11개 프로젝트 스캔 결과).
Rust 재구성이 35~45주짜리 작업이라면, 그 기간 동안 "잘못된 것 같지만 되는" 코드가
천 개씩 쌓인다. 수학적으로는 **성공 확률이 0** 이다.

그래서 포트 전에 **C# 을 테스트 오라클로** 세운다. 두 구현이 같은 입력에 대해
**바이트 단위로 같은 출력을 내는지**를 기계적으로 비교한다.

## 아키텍처

```
┌──────────────────────────┐          ┌──────────────────────────┐
│  WzComparerR2.Oracle     │          │  wz-cli oracle           │
│  (C#, 개발 중 유지)       │          │  (Rust, 신규)            │
│  배포물에 포함하지 않음    │          │                          │
└───────────┬──────────────┘          └──────────────┬───────────┘
            │ JSON (stdout)                          │ JSON (stdout)
            └──────────────┬─────────────────────────┘
                           ▼
                 ┌───────────────────┐
                 │  바이트 단위 비교   │ ← 100% 일치해야 통과
                 │  (jq -S 또는 difft)│
                 └───────────────────┘
```

## 명령 형태

양쪽 모두 **같은 인자**를 받고 **같은 JSON**을 stdout 에 낸다.

```bash
# 노드 트리 덤프
wz oracle tree   --wz A.wz [--filter <regex>]   > rust.json
WzComparerR2.Oracle tree   --wz A.wz [--filter <regex>] > csharp.json

# 이미지 픽셀
wz oracle image  --wz A.wz --path Character/00002000.img --format sha256
WzComparerR2.Oracle image  --wz A.wz --path Character/00002000.img --format sha256

# diff
wz oracle diff   --base A.wz --mod B.wz
WzComparerR2.Oracle diff   --base A.wz --mod B.wz

# 툴팁
wz oracle tooltip --wz A.wz --item 2000000
WzComparerR2.Oracle tooltip --wz A.wz --item 2000000

# 스탯
wz oracle stats  --wz A.wz --gear 01000000
WzComparerR2.Oracle stats  --wz A.wz --gear 01000000
```

## JSON 스키마 (버전 고정)

스키마가 바뀌면 비교가 무의미해지므로 **`schema_version`** 을 넣고 불일치 시 실패.

```jsonc
{
  "schema_version": 1,          // 양쪽이 반드시 같아야 한다
  "op": "tree",
  "profile": "PKG2_64B_V2",     // 감지된 포맷 프로파일 — 이것도 검증 대상
  "hash1": "0x...",             // PKG2 헤더
  "hash2": "0x...",
  "result": { /* op 별 */ }
}
```

### op: `tree`

```jsonc
{
  "result": {
    "root": {
      "name": "",
      "type": "Image",                 // 노드 타입 이름
      "children": [
        { "name": "info", "type": "Property", "children": [
            { "name": "level", "type": "Int", "value": 250 }
        ]},
        { "name": "0", "type": "Canvas", "canvas": {
            "w": 100, "h": 100, "format": 2, "scale": 0, "pages": 1,
            "sha256": "..."             // 픽셀 해시
        }},
        { "name": "link", "type": "UOL", "value": "../other.img/info" }
      ]
    },
    "node_count": 12345                // 정렬/비교 무관하게 검증
  }
}
```

**정렬 규칙**: 이름은 `natural_cmp` (Windows `StrCmpLogicalW` 등가) 로.
근사하면 안 된다 (리포트 §12 R4).

이미지 노드의 디렉터리 메타데이터는 `image` 객체로 나간다:

```jsonc
{ "name": "00002000.img", "type": "Image",
  "image": { "len": 231, "cs32": 305419896 } }
```

`offset` 은 **넣지 않는다.** C# `Wz_File.ReadDirTree` 는 `ReadContext.OffsetCalc`
가 있을 때만 `img.Offset` 을 채우는데 그 컨텍스트는 PKG2 경로에서만 설정된다 —
PKG1 이미지의 오프셋은 C# 쪽에서 구조적으로 알 수 없다 (#28 리포트 ②).
한쪽만 낼 수 있는 필드를 비교 대상에 두면 `tree` 게이트가 영원히 빨간불이 된다.

### op: `image`

```jsonc
{
  "result": { "w": 100, "h": 100, "format": 2, "scale": 0, "pages": 1,
              "sha256": "..." }
}
```

### op: `diff`

```jsonc
{
  "result": [
    { "path": "Character/00002000.img/info/level", "kind": "changed",
      "old": "250", "new": "270" },
    { "path": "String/Item.img/2000000.img", "kind": "added", "new": null },
    { "path": "Etc/Old.img", "kind": "removed", "old": null }
  ],
  "summary": { "changed": 120, "added": 3, "removed": 1 }
}
```

`kind` 는 `not_changed | append | remove | changed`
(`Comparer/DifferenceType.cs`). **기본 출력은 변경분만** —
C# 은 `IEnumerable<CompareDifference>` 스트림이고 Rust 도 스트림이어야 한다
(메모리 차이).

### op: `tooltip` / `stats`

```jsonc
{
  "result": { "text": "..." },                 // 툴팁 문자열 (개행 그대로)
  "result": { "prop": "str", "value": 420 },  // 스탯
}
```

**차이 나는 이유는 항상 원인 파악 가능한 형태여야 한다.**
`"0.1% off"` 같은 반올림 차이가 나면 어디서 틀렸는지 찾을 수 없다.

### op: `avatar` / `timeline` (#26, #27)

```bash
wz oracle timeline --wz A.wz --node walk1 [--path 이미지명]
                   [--canvas-wz CanvasWZ] [--interpol-ms 90]
WzComparerR2.Oracle timeline --wz A.wz --node walk1 [--path 이미지명]
                   [--canvas-wz CanvasWZ] [--interpol-ms 90]

wz oracle avatar --body-wz A.wz [--body 00002000.img] --canvas-wz C.wz
                 --zmap-wz B.wz [--zmap zmap.img] --action walk1 [--frame 0] [--elapsed 0]
WzComparerR2.Oracle avatar   --body-wz A.wz ...   # 동일 인자
```

```jsonc
// op: timeline
{
  "result": {
    "node": "walk1",
    "frame_count": 4,
    "cycle_ms": 720,                       // Σ 정규화 delay
    "frames": [
      {
        "frame": 0,
        "delay_ms": 180,                   // 0 → 120, abs (T16)
        "a0": 255, "a1": 255,              // 없으면 255 (T17)
        "children": [
          { "name": "body", "type": "canvas",
            "outlink": "Character/_Canvas/00002000.img/walk1/0/body",
            "w": 27, "h": 32, "sha256": "…" },          // 실캔버스 RGBA8 해시
          { "name": "face", "type": "short", "value": 1 },
          { "name": "delay", "type": "int",   "value": 180 }
        ]
      }
    ],
    "samples": [                            // interpol-ms > 0 일 때만
      { "frame": 0, "elapsed_ms": 0,   "alpha": 255 },
      { "frame": 0, "elapsed_ms": 90,  "alpha": 255 }
    ]
  }
}
```

```jsonc
// op: avatar
{
  "result": {
    "image":   { "w": 27, "h": 41 },           // 레이어 rect 유니온 바운딩 박스
    "sha256":  "…",                            // 합성 RGBA8 (w*h*4) 해시
    "layers":  [
      {
        "name": "body",
        "z": "body",                           // 문자열 | null (정수 z 이면 숫자)
        "z_index": 99,                         // 규칙: IndexOf, miss → Count;
                                               // 정수 z: <0 → Count-z, ≥0 → -1-z
        "outlink": "…",
        "w": 27, "h": 32,
        "origin": [19, 32],
        "sha256": "…"                          // 레이어 RGBA8 해시
      }
      // z_index 오름차순 순서 = 합성 순서 (2D painter's algorithm)
    ],
    "frames": { "delay_ms": 180, "a0": 255, "a1": 255 }
  }
}
```

**합성 수학 (양측 동일 구현 필수 — C# 도 문자 그대로):**

```text
eff = (sa * ea + 127) / 255
rgb = (src*eff + 127)/255 + (dst*(255-eff) + 127)/255   # 채널별, 255 포화
a   = eff + (da*(255-eff) + 127)/255
그림 위치 = move(프레임 move 벡터) - origin(canvas origin)
```

- GDI+/wgpu 를 **사용하지 않는다** — 정수 수학 소프트웨어 합성기만.
- SHA-256 는 엄격 비교 (퍼지 해시 금지).
- outlink 해석 규칙: 문자열의 `.img` 로 끝나는 마지막 세그먼트가 이미지,
  이후 경로가 img 내부 노드.

## 우선순위 (리포트 §10.2 표 그대로)

| 순위 | op | 왜 |
|---:|---|---|
| 1 | `tree` | 포맷 파싱이 전부의 기반. 여기서 어긋나면 전부 어긋난다 |
| 2 | `image` (픽셀 SHA-256) | 포맷 13종 전부. `&~3` 같은 quirk 포함 |
| 3 | `diff` | `natural_cmp` 정합성 |
| 4 | `tooltip` (StringLinker 결과) | 로케일 인덱스 |
| 5 | `stats` | `decimal` 의미론 + 정수 연산 순서 |
| 6 | `expr` (Calculator) | `decimal` 반올림 |
| 7 | `mapscene` | 물리 상수 |
| 8 | `avatar` (합성 픽셀) | 골격·프리즘 수학 |
| 9 | `timeline` | 병합/보간 |
| 10 | `patch` (결과 파일 SHA-256) | CRC32-BZIP2 정밀도 |

## C# 오라클 구현 지침

원본 C# 저장소는 공개되어 있다: <https://github.com/seotbeo/WzComparerR2>
(`seotbeo_main`). 다만 `WzComparerR2.Oracle` 하네스 프로젝트는 이 공개 트리에
포함되어 있지 않으므로, 차분 CI 에는 하네스를 포함한 저장소/ref 를 입력으로 준다.

`WzComparerR2.Oracle` 는 **기존 소스 프로젝트에 참조만** 추가한다. 로직 복제는 없다.
`Program.cs` 는 `Program.Main` 이나 `WzComparerR2.Oracle.csproj` 는 디버그/테스트
프로필에서만 빌드되도록 해서 **배포물에 절대 포함되지 않게** 한다.

```xml
<!-- WzComparerR2.Oracle.csproj — Release 빌드에서 매직으로 번들에서 제외 -->
<PropertyGroup>
  <OutputType>Exe</OutputType>
  <TargetFramework>net8.0-windows</TargetFramework>
  <IsPublishable>false</IsPublishable>
</PropertyGroup>
<ItemGroup>
  <ProjectReference Include="..\WzComparerR2.WzLib\WzComparerR2.WzLib.csproj" />
  <ProjectReference Include="..\WzComparerR2.Common\WzComparerR2.Common.csproj" />
  <ProjectReference Include="..\WzComparerR2\WzComparerR2.csproj" />  <!-- CharaSim/Comparer 등 -->
</ItemGroup>
```

출력은 `System.Text.Json` + `JavaScriptEncoder.UnsafeRelaxedJsonEscaping`
(한글/이모지 라운드트립에 필수 — 원본 `Utils.ToJson` 과 동일).

### TFM 은 `net8.0-windows`

`WzComparerR2.Common` 의 TFM 이 `net462;net6.0-windows;net8.0-windows` 이므로
오라클 콘솔은 `net8.0` 이 아니라 **`net8.0-windows`** 여야 한다. 또한 핀 커밋에서
`WzComparerR2.csproj` / `WzComparerR2.MapRender.csproj` 는 `CharaSimResource` 누락으로
빌드되지 않아 `diff` / `mapscene` 의 C# 짝이 없다.

## C# 참조 핀

오라클은 **명시적 커밋에 고정**되어야 differential 이 재현 가능하다.
C# 를 Rust 저장소에 벤더링하지 않고, 서브모듈로 붙이지도 않는다.

| 항목 | 값 |
|---|---|
| 저장소 | <https://github.com/seotbeo/WzComparerR2> |
| 핀 커밋 | `bb98c257f3a175a98bce84a38ef3bb65e8b93b75` |
| 제목 | `StringLinker: fix StringLinker Update for GMS format` |
| 날짜 | 2026-10-13 |
| 오라클 브랜치 | `oracle-harness` (이 커밋에서 만든 하네스 작업 브랜치) |

**업스트림이 움직였을 때 재핀하는 법**

```bash
git -C <C# 저장소> fetch origin
git -C <C# 저장소> log --oneline bb98c25..HEAD   # 무엇이 바뀌었는지 먼저 본다
git -C <C# 저장소> checkout -b oracle-harness-<새 sha> <새 sha>
# 1) 이 표의 커밋/날짜 갱신
# 2) docs/known-bugs.md 상단의 커밋 참조 갱신
# 3) 오라클 하네스 저장소/ref 갱신
# 4) cargo run -p wz-cli --bin wz -- oracle <op> … && dotnet run --project … 로 재대조
```

재핀할 때는 **StringLinker 를 먼저 본다.** `bb98c25` 자체가 StringLinker 대수정이고,
#25에서 발견한 파리티 갭의 직접 원인이었다.

## 차이 비교 스크립트

```bash
# 도구 옵션은 <op> 앞에 둔다. <op> 뒤의 인자는 양쪽 오라클에 그대로 전달된다.
scripts/differential.sh --csharp-repo /path/to/WzComparerR2 tree --wz A.wz
scripts/differential.sh --csharp-repo /path/to/WzComparerR2 expr --expr "7/2"
```

**jq 정규화(키 정렬)를 반드시 거친다.** C# 과 Rust 의 JSON 직렬화 순서는 다르므로
그대로 diff 하면 100% 실패한다. 비교 대상은 **값**이어야 한다.

종료 코드: `0` 일치 / `1` 불일치 / `2` 도구 사용법 오류 / `3` 비교 불가 /
`4` 하네스 오류. `3` 은 "돌리지 못했다"이고 `1` 은 "돌렸는데 다르다"다 — 이 둘을
섞으면 스텁이 영원히 실패로 보인다.

동작 방식·불일치 리포트 형식·`jq` 폴백은 `docs/differential.md` 에 있다.
CI 실행은 `.github/workflows/differential-ci.yml` (`workflow_dispatch` 전용, #33).

## 결정성 요구

같은 입력 → 같은 출력이 **반드시** 성립해야 한다 (리포트 §4.1 원칙 6).
그래야 CI 에서 회귀를 잡을 수 있다.

위험 요소와对策:

| 위험 |对策 |
|---|---|
| `Dictionary` 순회 순서 | `IndexMap` / 정렬된 키 사용 |
| float 포맷 | Rust: `{}` 대신 명시적 소수점 자리수. C#: 불변 문화(`InvariantCulture`) |
| 타임스탬프 포함 | 출력에 시간 넣지 말 것 |
| 경로 구분자 | 항상 `/` |
| 정렬 | `natural_cmp` 한 곳에서만 |

## 실资产 없이 돌아가는 부분

실제 WZ 파일은 리포에 없다. CI 는 **합성 WZ** 로 돈다
(`tools/corpus-builder`, §10.1). C# 오라클도 **같은 합성 파일**을 읽으면
차분 테스트가 CI 에서 100% 돌아간다.

실 파일 검증은 개발자 로컬 전용 스크립트로 한다
(`scripts/differential-local.sh`, `.gitignore` 에서 `tests/corpus/wz/` 제외).

## 상태

| 항목 | 상태 |
|---|---|
| 스키마 정의 | ✅ 본 문서 |
| 합성 코퍼스 생성기 | ✅ `tools/corpus-builder` (106 파일, 결정성 검증됨). `dirtree_pkg1.wz`(디렉터리 트리) + `props_pkg1.wz`(캔버스/장비 이미지 본문) |
| Rust `oracle` 서브커맨드 | ✅ tree/diff/image/tooltip/stats/expr/patch/map-scene + **avatar/timeline (#26/#27)** |
| C# `WzComparerR2.Oracle` | ⏳ tree/image/tooltip/stats/expr 구현, avatar/timeline 은 exit-3 스텁 (#31 규약 문서 참조) |
| `scripts/differential.sh` | ✅ (#29) 판정은 `tools/oracle-diff` |
| GitHub Actions 차분 CI | ✅ `.github/workflows/differential-ci.yml` (#33, 수동 `workflow_dispatch` 전용) |
| 실전 코퍼스 벌크 검증 | ✅ 2026 클라이언트 792개 WZ / 캔버스 4,610,672개 전부 (커밋 `b8f38b1`, `cargo run -p wz-cli --example bulk_scan`) |
