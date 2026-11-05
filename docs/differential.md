# 차분 비교 스크립트 (`scripts/differential.sh`)

`docs/oracle-spec.md` 의 `## 차이 비교 스크립트` 절이 이 문서를 가리킨다.

한 줄 요약: **양쪽 오라클에 같은 인자를 주고, 출력을 키 정렬 정규화한 뒤 바이트
비교하고, 어긋난 이유를 사람이 바로 찾을 수 있는 형태로 보고한다.**

---

## 선행 조건

| 필요 | 비고 |
|---|---|
| Rust 툴체인 | `wz` 가 없으면 스크립트가 `cargo build -p wz-cli --bin wz` 로 만든다 |
| .NET SDK | C# 오라클이 없으면 `dotnet build … -c Debug` 로 만든다 (8.0 이상) |
| `jq` | 스펙이 요구한다. **없어도 돌아가지만 폴백 정규화기를 쓰며 경고를 낸다** (§`jq`) |
| POSIX 셸 | Windows 는 Git Bash 또는 WSL 로 실행 |

## 실행

```bash
# 도구 옵션은 반드시 <op> 앞에 둔다. 뒤에 두면 오라클로 넘겨버린다.
scripts/differential.sh --csharp-repo /path/to/WzComparerR2 tree --wz A.wz
scripts/differential.sh --csharp-repo /path/to/WzComparerR2 expr --expr "1/3 + 2*3"
```

`<op>` 뒤의 인자는 **양쪽에 그대로** 전달된다. 한쪽에만 줄 플래그를 넣는 것은
두 CLI 사이의 계약 위반이며, 그 결과는 C# 쪽 `exit 2` → 하네스 오류(exit 4)로
보고된다("argv 계약이 어긋났다"는 사실이 드러나야 하므로 조용히 넘기지 않는다).

### 도구 옵션

| 옵션 | 환경 변수 | 뜻 |
|---|---|---|
| `--csharp-repo <경로>` | `WZ_CSHARP_REPO` | C# 저장소 체크아웃 루트 |
| `--csharp-dll <경로>` | `WZ_CSHARP_DLL` | 이미 빌드된 `WzComparerR2.Oracle.dll` |
| `--wz-bin <경로>` | `WZ_BIN` | 이미 빌드된 `wz` 실행 파일 |
| `--normalizer <모드>` | — | `auto`(기본) \| `jq`(스펙 그대로) \| `builtin` |
| `--out-dir <경로>` | `WZ_DIFF_OUT` | 산출물 디렉터리 |
| `--replay <경로>` | — | 저장된 산출물로만 다시 판정. 양쪽을 실행하지 않는다 |
| `--keep` | — | 일치해도 산출물을 남긴다 |
| `--no-build-csharp` | — | C# 오라클을 자동으로 빌드하지 않는다 |
| `-q`, `--quiet` | — | 리포트를 파일에만 쓴다 |

## C# 오라클은 어디로 지정하나

**경로는 저장소에 하드코딩하지 않는다.** C# 참조는 커밋으로 핀되고
(`docs/oracle-spec.md` 의 `## C# 참조 핀`), 사용자는 C# 를 Rust 저장소로
복사하지 않기로 했다. 그래서 둘 중 하나를 반드시 준다.

* `--csharp-repo <루트>` — `<루트>/WzComparerR2.Oracle/WzComparerR2.Oracle.csproj` 를
  찾아 `bin/` 아래 빌드된 dll 을 찾는다(`Debug` + `net8.0-windows` 우선).
  없으면 `dotnet build <csproj> -c Debug` 로 만든다.
* `--csharp-dll <경로>` — 이미 빌드돼 있으면 이것만 준다.

둘 다 없으면 어떤 절대 경로도 시도하지 않고, 방법 두 개를 나열하며 종료 코드 4로
끝난다.

리포트에는 C# 저장소의 현재 HEAD 를 짧게 찍는다(`git -C … rev-parse --short HEAD`).
실측과 대조한 커밋이 리포트 한 줄로 드러나야 재현이 된다.

### 재핀 절차

C# 를 새 커밋으로 옮길 때는 핀 표 네 곳만 고치면 된다 — 절차는
`docs/oracle-spec.md` 의 `## C# 오라클 구현 지침` 절에 있다.
재핀 후에는 **비교 불가 op 목록도 다시 확인**해야 한다. C# 가 `diff` 나
`mapscene` 을 구현하면 `tools/oracle-diff/src/lib.rs` 의 `expectation()` 표에서
`Uncomparable` 을 `Gate` 로 바꿔야 한다. 안 바꾸면 표가 exit 3 을 내므로
진전이 안 보인다 — 그게 일부러 "표는 로컬 관측"이라는 뜻이다.

## `jq` 상황

스펙이 `jq -S .` 를 요구하는 이유는 맞다. 두 직렬화기의 키 순서가 다르므로 정규화
없이 diff 하면 100% 틀린 이유로 실패한다. 그래서 정규화를 **버리지 않는다.**

| `--normalizer` | 동작 |
|---|---|
| `jq` | **스펙 그대로.** `jq` 가 없으면 설치 방법을 찍고 exit 4 |
| `auto` (기본) | `jq` 있으면 `jq -S .`, 없으면 내장 정규화기 + **경고** |
| `builtin` | 내장 정규화기 강제. 경고 없음 |

Windows 에서는:

```powershell
winget install --id jqlang.jq -e     # 또는 choco install jq
```

### 내장 폴백이 `jq -S .` 와 같은 일을 하는 이유

`serde_json` 은 `preserve_order` 를 켜지 않으면 `Map` 이 `BTreeMap` 이므로
**파싱만 해도 모든 중첩 객체의 키가 정렬된다.** 출력은 `to_string_pretty`
(2칸 들여쓰기)로 `jq` 기본 출력과 같은 모양이고, 중복 키 처리와 비표준 입력 거절도
같다. 숫자는 원문 표기를 유지한다(`1.0` 은 `1.0`).

폴백을 **조용히 쓰지 않는다.** 리포트에 `normalizer warn` 항목과 설치 명령이
항상 찍힌다. 정규화가 없는 것처럼 보이면 안 되니까.

> 폴백은 `--normalizer jq` 로 언제든 되돌아갈 수 있고, 되돌아가야 한다.
> `jq` 가 있는 CI 에서는 `jq` 경로를 검증하는 편이 정확하다.

## 불일치 리포트

스펙 `docs/oracle-spec.md:136-137` — *"차이 나는 이유는 항상 원인 파악 가능한
형태여야 한다."* 그래서 `FAIL` 한 줄로 끝내지 않는다. 불일치일 때 나가는 것:

1. op, 종료 코드, 양쪽에 **실제로 실행한 명령 전체**
2. **값 차이 경로** (JSON Pointer) — `/result/node_count: rust=8 csharp=5`
3. **통합 diff** (정규화본 기준, 3줄 context)
4. **산출물 디렉터리**

2번과 3번을 같이 주는 이유는 둘이 다른 일을 하기 때문이다. 통합 diff 는 사람이
읽을 때 좋지만 "어느 필드" 를 알려주지 않고, 값 경로는 필드를 알려주지만 문맥을
잃는다.

산출물 디렉터리 하나가 리플레이 디렉터리 하나다. 같은 형식(`rust.json`,
`rust.stderr`, `rust.code`, `rust.normalized.json`, `csharp.*`, `report.txt`)이라서
지난번 실패를 그대로 다시 판정할 수 있다.

```bash
scripts/differential.sh --replay target/oracle-diff/tree-1792... tree --wz A.wz
```

## op 등급과 현재 비교 가능 목록

`expectation()` 표는 `tools/oracle-diff/src/lib.rs` 에 있고, 근거 문구도 거기에 있다.

| 등급 | op | 비고 |
|---|---|---|
| `Gate` | `tree` `image` `stats` `expr` | 불일치면 exit 1 |
| `Advisory` | `tooltip` | 일치해도 exit 3. 아래 참조 |
| `Uncomparable` | `diff` `mapscene` `patch` `avatar` `timeline` | C# 가 `exit 3` |

### 현재 비교 불가 5개

| op | 사유 | 근거 |
|---|---|---|
| `diff` | `Comparer/WzFileComparer` 가 핀 커밋에서 빌드 불가한 `WzComparerR2.csproj` 안에 있다 | #28 실측 |
| `mapscene` | `WzComparerR2.MapRender` 가 같은 이유로 빌드 불가 | #28 실측 |
| `patch` | 도메인 불일치 — 이 포크의 `WzPatcher` 는 컨테이너 포맷을 읽지 않는다 | #28 실측 |
| `avatar` | Rust `wz oracle` 에 op 가 없다 | #28 |
| `timeline` | Rust `wz oracle` 에 op 가 없다 | #28 |

이 다섯 개는 표뿐 아니라 **C# 오라클이 실제로 `exit 3` 으로 돌려준다** — 그래서
판정 순서에서 스텁(exit 3)을 먼저 본다. 표는 추정, 종료 코드는 실측이다.

### `tooltip` 이 게이트가 아닌 이유

C# 에 **텍스트 툴팁이 없다.** `ItemTooltipRender.renderItem` 은 `Bitmap` 을 반환하고
문자열을 돌려주지 않는다 (#25 KNOWN GAP 1). 오라클이 양쪽에 공통인
`name` [+ `\n` + `desc`] 만 남긴 것은 대조가 가능하도록 축소한 것이다.
따라서 **일치해도 "툴팁 계약이 검증됐다"는 뜻이 아니다** — 그래서 `Advisory` 다.

`tooltip` 이 **불일치** 하면 그건 진짜 불일치이므로 exit 1 이다. `Advisory` 는
"일치를 통과로 승격하지 않는다"는 뜻이지 "불일치를 숨긴다"는 뜻이 아니다.

## 판정 순서

의도적인 순서다. 순서가 틀리면 오해가 나는 판정이 나온다.

1. 모르는 op → 2
2. **C# 가 `exit 3`** → 3. stdout 이 비어 있으므로 정규화를 먼저 시도하면
   "stdout 이 JSON 이 아니다" 하고 하네스 오류(4)로 잘못 떨어진다
3. op 표가 `Uncomparable` → 3
4. C# 가 `exit 2` / 그 밖의 0 아닌 코드 → 4. 비교가 일어나지 않았다
5. Rust 가 0 이 아님 → 4
6. 정규화 실패 → 4
7. 문자열 비교 → 0 / 1 / 3(`Advisory`)

## 테스트

판정 로직은 전부 Rust 에 있고 (`tools/oracle-diff`), 인라인 `#[cfg(test)]` 테스트가
붙어 있다. 셸 테스트 프레임워크가 이 저장소에 없다는 전제다.

```bash
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

`tools/oracle-diff` 의 테스트는 **C# 오라클도 `jq` 도 필요로 하지 않는다.**
`--replay` 경로로 두 쪽의 실행 결과를 파일로 넣어 `run()` 을 직접 구동한다.
커버되는 것:

* 종료 코드 5종의 매핑, op 등급 판정
* 정규화(키 정렬, 숫자 표기 보존, 비-JSON 거절)
* 스텁(`exit 3`) / 사용법 오류(`exit 2`) / op 실패(`exit 1`) / Rust 실패 구분
* `tooltip` 의 advisory 승격 금지
* 값 차이 경로 산출, 통합 diff hunks
* 산출물 쓰기와 리플레이 읽기가 같은 파일 이름을 쓴다
* C# dll 탐색 우선순위, `PATH` 탐색, 저장소 루트 추정

`scripts/differential.sh` 자체는 테스트 대상이 아니다. 하는 일은 "저장소 루트를
찾고 실행 파일을 `exec`" 하나뿐이고, 셸로 테스트하면 그 셸 구현에 의존하게 된다.

## 스펙이 틀렸던 곳

원래 `## 차이 비교 스크립트` 의 5줄 스크립트에서 실제로 틀린 것들.

### 1. `--format json` 은 어느 쪽도 받지 않는다 🔴

스펙 스크립트는 `wz oracle "$OP" "$@" --format json` 이라고 한다. 그런데:

* Rust `wz oracle` 의 플래그를 보면 `image` 에만 `--format` 이 있고 뜻이
  **해시 알고리즘**(`sha256` 등)이다 (`crates/wz-cli/src/main.rs`, `OracleAction::Image`).
  다른 op 에는 `--format` 이 없다.
* C# `OracleArgs.KnownFlags` 에도 `--format` 은 있고 뜻이 똑같이 해시 알고리즘이다.

즉 `--format json` 을 덧붙이면 `image` op 이 깨지고, 나머지 op 은 clap/hand-rolled
파서에서 "unknown flag" 로 죽는다. **JSON 이 기본 출력이라 플래그가 필요 없다.**

### 2. `../WzComparerR2.Oracle` 상대경로는 존재하지 않는다 🔴

스펙은 `dotnet run --project ../WzComparerR2.Oracle` 라고 한다. `scripts/` 기준
`../WzComparerR2.Oracle` 은 저장소 루트의 `WzComparerR2.Oracle` 이고 그런 디렉터리는
없고(사용자 결정: C# 는 복사하지 않는다), C# 저장소 기준으로 가도 `scripts/` 가
저장소 밖에 있다. **어느 기준으로 해도 경로가 성립하지 않는다.**

### 3. `wz` 와 `dotnet run` 을 전제로 한 실행 방식

`wz` 는 PATH 에 없고(빌드 산출물이다) `dotnet run` 은 매번 빌드 판단을 다시 한다.
출력까지 섞이면 정규화 전에 잡아야 할 노이즈가 생긴다. → 빌드된 실행 파일을
직접 `exec` 한다.

### 4. `set -e` + `diff -u` 만으로는 "왜 안 됐는지" 를 말할 수 없다

스펙의 스크립트는 `diff` 의 종료 코드(=1)만 준다. 그런데 필요한 것은:

* 키 순서만 달라도 통과해야 하는데, `jq` 부재/오작동 시 **전부 실패**한다
  (=틀린 이유로 실패하는 최악의 상태)
* C# 스텁(exit 3)이 "차이가 났다" 와 "아직 없다" 를 구분되지 않는다
* 불일치해도 산출물이 사라져 다음 사람이 같은 일을 다시 해야 한다
* 137행의 "차이 나는 이유는 항상 원인 파악 가능한 형태" 요구를 충족하지 못한다

### 5. 판정 로그직에 테스트가 없다

5줄 스크립트에는 검증 수단이 없다. → 판정을 Rust 로 옮겼다.

### 6. `jq` 는 필수가 아니다 (이 문서가 추가한 것)

스펙의 전제 — "`jq` 가 반드시 있다" — 가 이 머신에서 거짓이다. 정규화를 없애면
더 나쁘고, 정규화를 대체하면 그 사실을 숨기게 되므로, **폴백 + loud warning +
strict 모드** 로 셋 다 가능하게 했다.

### 7. 🔴 새로 발견된 스펙 문제 — 정수 값 `expr` 결과의 표기

#29 를 실제로 돌려서 발견했다. #28 리포트의 6건에는 없었다.

```
$ wz          oracle expr --expr "1+1"      →  "value": 2.0
$ WzComparerR2.Oracle expr --expr "1+1"      →  "value": 2
$ wz          oracle expr --expr "7/2"      →  "value": 3.5   (양쪽 같음, exit 0)
```

`1+1`, `2*3` 처럼 결과가 정수인 경우만 어긋난다. Rust 는 `f64` 를 그대로
`serde_json` 에 넘기고, 정수값 `f64` 는 `2.0` 으로 직렬화된다. C# 는
`decimal → double` 변환 후 정수면 `2` 로 쓴다.

**이것은 정규화 문제가 아니다.** `jq -S .` 도 `2.0` 을 `2` 로 접지 않는다.
정규화가 제대로 동작해도 남는, 진짜 계약 위반이다. 스펙이 정한 계약이
("`decimal` 반올림") 어느 쪽이 옳은지는 정해 두지 않았다. 고치지 않고 여기서
**드러내는 것**이 #28/#29 의 원칙이다. 해결은 별도 티켓 대상이다.
## CI — `.github/workflows/differential-ci.yml` (#33)

**수동 트리거 전용이다.** `push`/`pull_request` 에 붙이지 않는다. C# 오라클은 이
저장소에 없고(`## C# 참조 핀`), .NET Windows 데스크톱 빌드는 분 단위로 비싸다.
상시 회귀 방지는 CI 의 `rust-gates` 잡이 C# 없이 담당한다.

```text
workflow_dispatch
├─ rust-gates    (windows-latest, 항상)
│   ├─ cargo test --workspace
│   ├─ cargo clippy --workspace --all-targets -- -D warnings
│   ├─ cargo deny check
│   ├─ corpus-builder 2회 실행 → 바이트 대조 (결정성)
│   └─ wz oracle tree/image/stats/expr JSON 자기검증 (scripts/check-oracle-json.py)
└─ differential  (csharp_repository 를 준 경우에만, rust-gates 통과 후)
    ├─ .NET 8.0 SDK + C# 오라클 빌드 (ref 는 입력으로 받은 핀)
    └─ Gate op 마다 scripts/differential.sh 실행 → exit code 로 판정
```

### 입력

| 입력 | 기본값 | 뜻 |
|---|---|---|
| `ops` | `tree image stats expr` | 돌릴 Gate op 목록 (공백 구분) |
| `csharp_repository` | (비어 있음) | C# 저장소 `owner/name`. **비우면 차분 잡 생략** |
| `csharp_ref` | `oracle-harness` | 핀 커밋 또는 오라클 브랜치 |
| `csharp_project` | `WzComparerR2.Oracle/WzComparerR2.Oracle.csproj` | C# 저장소 루트 기준 경로 |
| `dotnet_version` | `8.0.x` | `.NET SDK` 버전 |
| `normalizer` | `jq` | 스펙 그대로. `auto`/`builtin` 은 폴백 |
| `fail_on_mismatch` | `true` | 불일치(1)·비교 불가(3)를 실패로 볼지 |

**저장소 URL 을 워크플로우에 하드코딩하지 않는 이유**: 핀은 문서에 있고
(`docs/oracle-spec.md`), CI 는 그 핀을 **입력으로** 받아야 재현 가능한 대조가 된다.
오라클 저장소가 비공개라면 `CSHARP_REPO_TOKEN` 시크릿을 쓰고, 없으면 `github.token` 으로
폴백한다.

### op 별 입력 (합성 코퍼스 고정)

| op | 인자 |
|---|---|
| `tree` | `--wz props_pkg1.wz` |
| `image` | `--wz props_pkg1.wz --path canvas.img/canvas --format sha256` |
| `stats` | `--wz props_pkg1.wz --gear 01000000` |
| `expr` | `--expr "7/2"` |

`tree`/`image`/`stats` 는 `tools/corpus-builder` 가 만드는 `props_pkg1.wz` 를 쓴다.
이 파일은 **실제 이미지 본문**(캔버스 `canvas.img`, 장비 스탯 `01000000.img`)을
가지고 있고, 하위 블록 사이에 패딩이 없어 C# `Wz_File.GetDirTree` 의 순차 읽기와
어긋나지 않는다 (#28 KNOWN GAP 4 해소).

`expr` 은 **`7/2` 만 쓴다.** `1+1` 처럼 결과가 정수인 식은 #29 §7 의 미해결 계약
차이(`2.0` vs `2`)로 반드시 불일치가 난다. 그 차이를 게이트에 넣으면 CI 가
영원히 빨간불이 되므로, **알려진 차이를 예외 목록으로 덮지 않고**(#29 결정)
애초에 일치하는 케이스로 게이트를 세운다.

### 종료 코드 처리

| exit | 뜻 | CI 판정 |
|---|---|---|
| 0 | 일치 | 통과 |
| 1 | **불일치** | `fail_on_mismatch=true` 면 실패 |
| 2 | 도구 사용법 오류 | **항상 실패** — 계약이 아니라 하네스가 깨진 상태다 |
| 3 | 비교 불가 (C# 스텁) | `fail_on_mismatch=true` 면 실패 |
| 4 | 하네스 오류 | **항상 실패** |

산출물(`target/oracle-diff/**`)은 **실패해도 업로드된다.** 다음 사람이 같은
실패를 다시 만들 필요 없이 `--replay` 로 판정만 재실행할 수 있다.

### `tree` 게이트와 이미지 `offset`

`tree` 는 Gate op 인데, 예전에는 이미지 엔트리의 `offset` 키 때문에 항상
불일치였다 — Rust 는 오프셋을 계산하지만 C# `WzLib` 은 PKG1 에서 그 값을
구조적으로 알 수 없다 (#28 리포트 ②). #33 은 **Rust 쪽 `tree` JSON 에서 `offset`
을 뺐다** (`crates/wz-cli`). 스펙(`docs/oracle-spec.md`)에 없던 필드이고, 비교
대상에 둘 수 없는 필드를 남겨 두면 게이트가 무의미해진다. 사람이 보는
`wz tree` 출력은 그대로 `@<offset>` 을 찍는다.
