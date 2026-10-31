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

### op: `avatar` / `timeline` (#26, #27 — 규약 문서 `.scratch/oracle-completion/avatar-timeline-wayfinding.md`)

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

`WzComparerR2.Oracle` 는 **기존 소스 프로젝트에 참조만** 추가한다. 로직 복제는 없다.
`Program.cs` 는 `Program.Main` 이나 `WzComparerR2.Oracle.csproj` 는 디버그/테스트
프로필에서만 빌드되도록 해서 **배포물에 절대 포함되지 않게** 한다.

```xml
<!-- WzComparerR2.Oracle.csproj — Release 빌드에서 매직으로 번들에서 제외 -->
<PropertyGroup>
  <OutputType>Exe</OutputType>
  <TargetFramework>net8.0</TargetFramework>
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

## 차이 비교 스크립트

```bash
#!/usr/bin/env bash
set -euo pipefail
# 사용: scripts/differential.sh <op> [args...]
OP="$1"; shift
wz oracle "$OP" "$@" --format json > /tmp/rust.json
dotnet run --project ../WzComparerR2.Oracle -- "$OP" "$@" --format json > /tmp/csharp.json
# 키 순서 무관 비교 + 줄바꿈 정규화
jq -S . /tmp/rust.json    > /tmp/rust.s.json
jq -S . /tmp/csharp.json > /tmp/csharp.s.json
diff -u /tmp/csharp.s.json /tmp/rust.s.json
```

**jq 정규화(키 정렬)를 반드시 거친다.** C# 과 Rust 의 JSON 직렬화 순서는 다르므로
그대로 diff 하면 100% 실패한다. 비교 대상은 **값**이어야 한다.

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
| 합성 코퍼스 생성기 | ✅ `tools/corpus-builder` (100 파일, 결정성 검증됨) |
| Rust `oracle` 서브커맨드 | ✅ tree/diff/image/tooltip/stats/expr/patch/map-scene + **avatar/timeline (#26/#27)** |
| C# `WzComparerR2.Oracle` | ⏳ tree/image/tooltip/stats/expr 구현, avatar/timeline 은 exit-3 스텁 (#31 규약 문서 참조) |
| `scripts/differential.sh` | ✅ (#29) |
| 실전 코퍼스 벌크 검증 | ✅ 2026 클라이언트 792개 WZ / 캔버스 4,610,672개 전부 (커밋 `b8f38b1`, `cargo run -p wz-cli --example bulk_scan`) |
