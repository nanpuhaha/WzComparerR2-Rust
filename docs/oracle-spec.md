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
| Rust `oracle` 서브커맨드 | ⏳ Phase 1 완료 후 (컨테이너 파서가 필요) |
| C# `WzComparerR2.Oracle` | ⬜ 착수 필요 (Phase 1 병행) |
| `scripts/differential.sh` | ⬜ Phase 1.12 |

## `op: tooltip` 계약 — 확정 사항과 KNOWN GAP

### 1. `StringLinker` 의 `stringRoot` 는 **String.wz 파일 자체의 루트**다

원본 `StringLinker.Load(Wz_File stringWz, ...)` 는 `stringWz.Node` 를
`stringNode` 로 넘긴다 (`StringLinker.cs:44`). 그 노드의 **직접 자식**이
`Consume.img` / `Eqp.img` 같은 `.img` 엔트리다 (`StringLinker.cs:67-71`).

따라서 Rust `StringLinker::load_string_wz(arena, string_root)` 에 넘기는
노드는 "`String.wz` 라는 이름의 노드"가 아니라 **WZ 파일 루트**여야 한다.
`wz oracle tooltip --wz String.wz` 에서 `arena.root()` 가 바로 그 노드다.
한 단계 더 내려간 노드를 넘기면 `_ => {}` 에 걸려 조용히 아무 것도 잡지 않는다.

→ `string_linker_takes_the_wz_file_root_whose_children_are_img_entries` 가 고정하고,
CLI 는 테이블을 하나도 못 읽으면 오류로 끝낸다.

### 2. `Item.wz` / `Etc.wz` 는 **별도 루트**다

원본 `Load` 는 `stringNode, itemNode, etcNode, questNode, reactorNode`
5개를 받는다 (`StringLinker.cs:47`). `0910.img` 은 `Item.wz/Special/` 아래에,
`SetItemInfo.img` 와 `worldArchive.img` 은 `Etc.wz` 아래에 있다. 그래서 Rust 도
`load_item_wz` / `load_etc_wz` 진입점을 따로 갖는다.

### 3. 🔴 KNOWN GAP — C# 에 **텍스트** 툴팁이 없다

`result.text` 와 대조할 C# 쪽 문자열이 **존재하지 않는다**.
`ItemTooltipRender.renderItem` (`ItemTooltipRender.cs:52-117`) 은 `Bitmap` 을
만들어서 GDI `DrawString` 으로 다음을 **좌표에 얹는다**:

1. `sr.Name` — `StringItem` 에 없으면 `"(null)"`
2. 속성 태그 (`quest` / `pquest` / `only` / `tradeBlock` / `accountSharable`) 를 `", "` 로 이은 것
3. 아이콘 비트맵
4. `"要求等级 : " + reqLevel`
5. `sr.Desc + sr.AutoDesc` — **구분자 없이 연결**, `Desc` 가 있을 때만
6. `tradeAvailable` 속성 문자열

돌려주는 문자열이 없다. 그러므로:

- Rust `format_tooltip` 은 `name` + `"\n"` + `desc` 만 돌려주고,
  **`Desc + AutoDesc` 연결과 `"(null)"` 이름 대체를 하지 않는다.**
- 더 풍부한 형식을 **발명하지 않는다.** 발명하면 대조 자체가 불가능해진다.
- C# 오라클을 만들 때는 위 6줄 중 텍스트로 표현 가능한 것만 골라
  **어느 줄까지 포함하는지 먼저 결정하고** 양쪽에 같은 규칙을 적용해야 한다.

### 4. 없는 ID 는 오류다

`format_tooltip` 이 `None` 을 돌려주면 `wz oracle tooltip` 은 종료 코드 1 로
실패한다. 빈 문자열을 출력하면 "차이가 났는데 어디서 났는지" 를 알 수 없다
(§`op: tooltip / stats` 의 마지막 문장).
