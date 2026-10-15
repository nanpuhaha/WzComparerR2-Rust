//! 차분 러너의 **판정 로직**.
//!
//! 스크립트(`scripts/differential.sh`)는 이 크레이트의 얇은 껍데기다. 그 이유는
//! 이 저장소에 셸 테스트 프레임워크가 없고, 판정 로직이 "어떤 차이가 난 이유를
//! 알아낼 수 있는가"를 결정하는 곳이기 때문이다. 셸에 두면 검증할 방법이 없다.
//!
//! ## 왜 `jq -S .` 를 대체하지 않고 "같은 일을 하는" 함수를 하나 더 두는가
//!
//! 스펙(`docs/oracle-spec.md`, `## 차이 비교 스크립트`)은 `jq -S .` 를 요구한다.
//! 이유는 정확하다 — 두 직렬화기의 키 순서가 달라 정규화 없이 diff 하면 100% 실패한다.
//! 정규화 자체를 버리면 **틀린 이유로** 전부 실패하므로 더 나쁘다.
//!
//! 그래서 정규화는 두 경로로 제공된다.
//!
//! * `NormalizerMode::Jq` — 스펙 그대로. `jq` 가 없으면 **설치 방법을 명시하고 중단**한다.
//! * `NormalizerMode::Builtin` — `serde_json` 기반 폴백. `jq` 가 없는 머신에서 돌아간다.
//!
//! 둘 다 "모든 중첩 객체의 키를 정렬"한다. `jq -S` 와 폴백의 차이는 §폴백 문서를 본다.

use std::collections::BTreeMap;
use std::fmt;

/// 오라클 한쪽의 실행 결과.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SideResult {
    /// 프로세스 종료 코드.
    pub code: i32,
    /// 표준 출력.
    pub stdout: String,
    /// 표준 에러.
    pub stderr: String,
}

impl SideResult {
    /// 성공(종료 코드 0)으로 보이는 실행 결과를 만든다.
    pub fn ok(stdout: impl Into<String>) -> Self {
        Self {
            code: 0,
            stdout: stdout.into(),
            stderr: String::new(),
        }
    }

    /// 실패한 실행 결과를 만든다. `stdout` 은 비어 있어야 한다 — 오라클은 절대
    /// 부분 출력이나 빈 결과를 성공 코드와 함께 내지 않는다.
    pub fn failed(code: i32, stderr: impl Into<String>) -> Self {
        Self {
            code,
            stdout: String::new(),
            stderr: stderr.into(),
        }
    }
}

/// 한쪽이 준 종료 코드가 무엇을 뜻하는가.
///
/// `WzComparerR2.Oracle/Program.cs` 의 종료 코드 계약(핑 `bb98c25`, 커밋 `33828fe`):
/// `0` 성공, `1` op 실패, `2` 사용법 오류, **`3` = 아직 만들지 못한 op**.
pub mod exit_code_contract {
    /// op 이 성공했다.
    pub const SUCCESS: i32 = 0;
    /// op 이 돌았지만 결과를 만들지 못했다.
    pub const FAILURE: i32 = 1;
    /// CLI 인자가 잘못됐다.
    pub const USAGE: i32 = 2;
    /// 계약에는 있지만 도달 가능한 C# 구현이 없다. **차분 아님.**
    pub const NOT_IMPLEMENTED: i32 = 3;
}

/// 판정 결과의 종류. 종료 코드는 [`exit_code`] 가 정한다.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// 양쪽이 정규화 후 바이트 단위로 같다. 게이트.
    Match,
    /// 양쪽이 실제로 돌았고 값이 다르다.
    Mismatch,
    /// 이 도구의 호출 자체가 잘못됐다.
    ToolUsage,
    /// 아직 비교할 짝이 없다. 통과도 실패도 아니다.
    NotComparable,
    /// 비교를 시도하지 못했다 — 빌드 실패, 크래시, 정규화 불가 등.
    HarnessError,
}

/// 종료 코드.
///
/// | 결과 | 코드 |
/// |---|---|
/// | 일치 | 0 |
/// | 불일치 | 1 |
/// | 도구 사용법 오류 | 2 |
/// | 비교 불가(C# 스텁 등) | 3 |
/// | 하네스 오류 | 4 |
pub fn exit_code(outcome: Outcome) -> i32 {
    match outcome {
        Outcome::Match => 0,
        Outcome::Mismatch => 1,
        Outcome::ToolUsage => 2,
        Outcome::NotComparable => 3,
        Outcome::HarnessError => 4,
    }
}

/// op 의 결과가 **게이트인지** 판단할 때 쓰는 기대치.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expectation {
    /// 바이트 일치 자체가 계약이다. 일치가 곧 통과다.
    Gate,
    /// 양쪽이 **의도적으로 좁힌** 계약만 내보낸다. 그래서 일치해도
    /// 전체 계약에 대한 증거가 아니며, 통과로 보고하지 않는다.
    Advisory,
    /// 대조할 짝이 없다. 통과도 실패도 아니다.
    Uncomparable,
}

/// op 하나의 기대치와 그 근거.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpExpectation {
    pub op: &'static str,
    pub expectation: Expectation,
    /// 왜 이 등급인지. 리포트에서 그대로 인용된다.
    pub reason: &'static str,
}

/// `oracle` op 표.
///
/// **"이미 알려진 차이" 예외 목록은 일부러 두지 않는다.** 없으면 mismismatch 는
/// 어떤 것이든 exit 1 이고 그것이 정직하다. #28 리포트가 정리한 스펙 JSON 문제
/// 6건(명령형 `kind`, 존재하지 않는 `profile` 값 등)이 그래서 가려지지 않는다.
pub fn expectation(op: &str) -> Option<OpExpectation> {
    const TABLE: &[OpExpectation] = &[
        OpExpectation {
            op: "tree",
            expectation: Expectation::Gate,
            reason: "포맷 파싱이 전부의 기반. 여기서 어긋나면 전부 어긋난다 (스펙 우선순위 1).",
        },
        OpExpectation {
            op: "image",
            expectation: Expectation::Gate,
            reason: "13종 포맷의 디코딩 결과 해시 (스펙 우선순위 2). \
                     단, C# 은 GDI+ 로 다시 그려서 해시를 내므로 영구 불일치 가능성이 있다 (#28 KNOWN GAP 1).",
        },
        OpExpectation {
            op: "stats",
            expectation: Expectation::Gate,
            reason: "decimal 의미론 + 정수 연산 순서 (스펙 우선순위 5).",
        },
        OpExpectation {
            op: "expr",
            expectation: Expectation::Gate,
            reason: "C# 은 decimal, Rust 는 f64 이므로 값이 다를 수 있다 (#28 스펙 문제 5). \
                     그 차이는 고치는 게 아니라 드러내야 하는 것이다.",
        },
        OpExpectation {
            op: "tooltip",
            expectation: Expectation::Advisory,
            reason: "C# 에 텍스트 툴팁이 없다. `ItemTooltipRender.renderItem` 은 Bitmap 을 반환한다 (#25 KNOWN GAP 1). \
                     오라클이 양쪽에 공통인 `name` [+ `\\n` + `desc`] 만 남긴 것이므로 \
                     일치해도 '툴팁 계약이 검증됐다'는 뜻이 아니다.",
        },
        OpExpectation {
            op: "diff",
            expectation: Expectation::Uncomparable,
            reason: "Comparer/WzFileComparer 는 핀 커밋에서 빌드 불가한 WzComparerR2.csproj 안에 있다 (#28 실측).",
        },
        OpExpectation {
            op: "mapscene",
            expectation: Expectation::Uncomparable,
            reason: "WzComparerR2.MapRender 가 같은 이유로 빌드 불가 (#28 실측).",
        },
        OpExpectation {
            op: "patch",
            expectation: Expectation::Uncomparable,
            reason: "도메인 불일치 — 이 포크의 WzPatcher 는 컨테이너 포맷을 읽지 않는다 (#28 실측).",
        },
        OpExpectation {
            op: "avatar",
            expectation: Expectation::Uncomparable,
            reason: "Rust `wz oracle` 에 아직 op 가 없다. 비교할 짝이 존재하지 않는다 (#28).",
        },
        OpExpectation {
            op: "timeline",
            expectation: Expectation::Uncomparable,
            reason: "Rust `wz oracle` 에 아직 op 가 없다. 비교할 짝이 존재하지 않는다 (#28).",
        },
    ];

    TABLE.iter().find(|e| e.op == op).copied()
}

/// 정규화 모드.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NormalizerMode {
    /// `jq` 가 있으면 `jq -S .`, 없으면 폴백. 폴백을 쓰면 반드시 경고한다.
    Auto,
    /// 스펙 그대로. `jq` 가 없으면 실패한다.
    Jq,
    /// 폴백을 강제.
    Builtin,
}

/// 정규화기가 무엇으로 결정되었는지.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Normalizer {
    /// `jq -S .`
    Jq,
    /// 저장소 내장 정규화기
    Builtin,
}

impl Normalizer {
    /// 사람이 읽을 이름.
    pub fn label(self) -> &'static str {
        match self {
            Self::Jq => "jq -S .",
            Self::Builtin => "builtin (serde_json key-sorted)",
        }
    }
}

/// `jq` 가 없을 때 사용자에게 보여줄 설치 지침.
///
/// 폴백이 조용히 쓰였다면 "이 머신에는 jq 가 없다"는 사실이 사라지고, 그래서는
/// 안 된다. 정규화가 없는 것처럼 보이게 되면 안 되니까.
pub const JQ_INSTALL_HINT: &str = "jq 가 설치되어 있지 않습니다.\n\
     설치:\n  \
     winget install --id jqlang.jq -e\n  \
     choco install jq\n  \
     (Debian/Ubuntu) apt-get install -y jq\n\
     설치하면 스펙이 요구하는 `jq -S .` 경로로 자동 전환됩니다.";

/// 정규화기를 고른다.
///
/// * [`NormalizerMode::Jq`] 는 `jq` 가 없으면 설치 지침과 함께 실패한다.
pub fn resolve_normalizer(
    mode: NormalizerMode,
    jq_available: bool,
) -> Result<Normalizer, String> {
    match mode {
        NormalizerMode::Jq => {
            if jq_available {
                Ok(Normalizer::Jq)
            } else {
                Err(JQ_INSTALL_HINT.to_string())
            }
        }
        NormalizerMode::Auto => Ok(if jq_available {
            Normalizer::Jq
        } else {
            Normalizer::Builtin
        }),
        NormalizerMode::Builtin => Ok(Normalizer::Builtin),
    }
}

/// 저장소 내장 정규화기 — `jq -S .` 와 같은 키 정렬을 한다.
///
/// `serde_json` 은 `preserve_order` 를 켜지 않으면 `Map` 이 `BTreeMap` 이므로,
/// 파싱만 해도 모든 중첩 객체의 키가 정렬된다. pretty 출력(2칸 들여쓰기)도
/// `jq` 기본 출력과 같은 모양이다.
///
/// `jq` 와 **같지 않은** 지점(그래서 명시적으로 "폴백"이라고 부른다):
///
/// * 숫자는 원문 표기를 유지한다(`1.0` 은 `1.0`). `jq` 도 원문 숫자를 유지한다.
/// * 중복 키가 있으면 마지막 값만 남는다. `jq` 와 같다.
/// * 비표준 입력은 거절한다. `jq` 도 거절한다.
pub fn normalize_builtin(src: &str) -> Result<String, String> {
    let value: serde_json::Value =
        serde_json::from_str(src).map_err(|e| format!("stdout 이 JSON 이 아닙니다: {e}"))?;
    serde_json::to_string_pretty(&value).map_err(|e| format!("JSON 재직렬화 실패: {e}"))
}

/// 정규화된 JSON 문자열 두 개의 값 차이를 JSON Pointer 경로로 나열한다.
///
/// 스펙 `docs/oracle-spec.md:136-137` 의 요구 —
/// *"차이 나는 이유는 항상 원인 파악 가능한 형태여야 한다."*
/// 통합 diff 는 사람이 읽을 때 좋지만 "어느 필드" 를 바로 알려주지는 않는다.
/// 그래서 둘 다 낸다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValueDifference {
    /// RFC 6901 스타일 경로. 예: `/result/node_count`.
    pub pointer: String,
    /// Rust 쪽 값. 없으면 `<absent>`.
    pub rust: String,
    /// C# 쪽 값. 없으면 `<absent>`.
    pub csharp: String,
}

impl fmt::Display for ValueDifference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: rust={} csharp={}", self.pointer, self.rust, self.csharp)
    }
}

/// 값 차이를 사람이 읽을 때 한 줄로 자른다.
const MAX_VALUE_CHARS: usize = 160;

fn render_scalar(value: &serde_json::Value) -> String {
    let raw = match value {
        serde_json::Value::String(s) => serde_json::to_string(s).unwrap_or_else(|_| s.clone()),
        other => other.to_string(),
    };
    if raw.chars().count() > MAX_VALUE_CHARS {
        let head: String = raw.chars().take(MAX_VALUE_CHARS).collect();
        format!("{head}…")
    } else {
        raw
    }
}

fn collect(
    a: &serde_json::Value,
    b: &serde_json::Value,
    pointer: &str,
    out: &mut Vec<ValueDifference>,
) {
    use serde_json::Value;

    match (a, b) {
        (Value::Object(x), Value::Object(y)) => {
            // BTreeMap: 키 집합을 정렬된 순서로 순회하려고.
            let keys: BTreeMap<&String, (Option<&Value>, Option<&Value>)> = x
                .keys()
                .chain(y.keys())
                .map(|k| (k, (x.get(k), y.get(k))))
                .collect();
            for (key, (xv, yv)) in keys {
                let child = format!("{pointer}/{}", escape_pointer_token(key));
                match (xv, yv) {
                    (Some(xa), Some(ya)) => collect(xa, ya, &child, out),
                    (Some(_), None) => out.push(ValueDifference {
                        pointer: child,
                        rust: "<present>".to_string(),
                        csharp: "<absent>".to_string(),
                    }),
                    (None, Some(_)) => out.push(ValueDifference {
                        pointer: child,
                        rust: "<absent>".to_string(),
                        csharp: "<present>".to_string(),
                    }),
                    (None, None) => unreachable!("키는 둘 중 하나에서 반드시 나온다"),
                }
            }
        }
        (Value::Array(x), Value::Array(y)) => {
            if x.len() != y.len() {
                out.push(ValueDifference {
                    pointer: format!("{pointer}/length"),
                    rust: x.len().to_string(),
                    csharp: y.len().to_string(),
                });
            }
            for (i, (xa, ya)) in x.iter().zip(y.iter()).enumerate() {
                collect(xa, ya, &format!("{pointer}/{i}"), out);
            }
        }
        _ => {
            if a != b {
                out.push(ValueDifference {
                    pointer: pointer.to_string(),
                    rust: render_scalar(a),
                    csharp: render_scalar(b),
                });
            }
        }
    }
}

/// RFC 6901 이스케이프(`~` → `~0`, `/` → `~1`).
///
/// 순차 치환을 **의도적으로** 유지한다. RFC 6901 이 순서를 지정하고, 1단계가
/// 만들어 낸 `~0` 에 2단계가 손대지 않는다는 사실이 이 형태에서 눈으로 보인다.
#[allow(clippy::collapsible_str_replace)]
fn escape_pointer_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

/// 두 정규화된 JSON 문자열의 값 차이를 경로와 함께 나열한다.
///
/// 문자열이 JSON 이 아니면 빈 목록을 돌려준다 — 호출자가 통합 diff 로 넘어가게 된다.
pub fn value_differences(rust_json: &str, csharp_json: &str) -> Vec<ValueDifference> {
    let a: serde_json::Value = match serde_json::from_str(rust_json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let b: serde_json::Value = match serde_json::from_str(csharp_json) {
        Ok(v) => v,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    collect(&a, &b, "", &mut out);
    out.sort_by(|x, y| x.pointer.cmp(&y.pointer));
    out
}

/// 판정 결과.
///
/// 생성은 [`compare`] 만 한다. 모든 [`Outcome`] 이 여기서 한 번씩 나온다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Comparison {
    /// 정규화 후 바이트 단위로 같다. 게이트 op 이면 통과다.
    Match { op: String },
    /// 정규화 후 바이트 단위로 같다. 다만 op 이 Advisory 라 통과가 아니다.
    AdvisoryMatch { op: String, reason: String },
    /// 양쪽이 돌았고 값이 다르다.
    Mismatch { op: String },
    /// 아직 비교할 짝이 없다. 통과도 실패도 아니다.
    NotComparable { op: String, reason: String },
    /// `oracle` op 가 아니다.
    UnknownOp { op: String },
    /// C# 쪽이 사용법 오류(exit 2). argv 계약이 어긋났다는 뜻이므로 하네스 오류다.
    CsharpUsage { op: String, stderr: String },
    /// C# 쪽이 돌았지만 실패(exit 1). 비교가 일어나지 않았다.
    CsharpFailed { op: String, stderr: String },
    /// Rust 쪽이 실패. 비교가 일어나지 않았다.
    RustFailed { op: String, stderr: String },
    /// 한쪽의 stdout 가 JSON 이 아니다. 정규화가 불가능하므로 비교가 아니다.
    UnparsableOutput {
        op: String,
        side: &'static str,
        reason: String,
    },
}

impl Comparison {
    /// 이 판정이 속하는 결과 종류.
    pub fn outcome(&self) -> Outcome {
        match self {
            Self::Match { .. } => Outcome::Match,
            Self::AdvisoryMatch { .. } | Self::NotComparable { .. } => Outcome::NotComparable,
            Self::Mismatch { .. } => Outcome::Mismatch,
            Self::UnknownOp { .. } => Outcome::ToolUsage,
            Self::CsharpUsage { .. }
            | Self::CsharpFailed { .. }
            | Self::RustFailed { .. }
            | Self::UnparsableOutput { .. } => Outcome::HarnessError,
        }
    }

    /// 판정을 한 줄로 요약한다.
    pub fn headline(&self) -> String {
        let op = match self {
            Self::Match { op }
            | Self::AdvisoryMatch { op, .. }
            | Self::Mismatch { op }
            | Self::NotComparable { op, .. }
            | Self::UnknownOp { op }
            | Self::CsharpUsage { op, .. }
            | Self::CsharpFailed { op, .. }
            | Self::RustFailed { op, .. }
            | Self::UnparsableOutput { op, .. } => op,
        };
        let verdict = match self {
            Self::Match { .. } => "MATCH",
            Self::AdvisoryMatch { .. } => "ADVISORY MATCH (게이트 아님)",
            Self::Mismatch { .. } => "MISMATCH",
            Self::NotComparable { .. } => "NOT COMPARABLE",
            Self::UnknownOp { .. } => "UNKNOWN OP",
            Self::CsharpUsage { .. } => "CSHARP USAGE ERROR",
            Self::CsharpFailed { .. } => "CSHARP FAILURE",
            Self::RustFailed { .. } => "RUST FAILURE",
            Self::UnparsableOutput { .. } => "UNPARSABLE OUTPUT",
        };
        format!("{verdict}  op={op}")
    }
}

/// 판정 결과와 정규화본을 함께 돌려준다.
///
/// 리포트가 통합 diff 를 그려야 하는데 정규화본이 있어야 그릴 수 있다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Verdict {
    pub comparison: Comparison,
    /// 정규화에 성공했을 때만 `Some`. 정규화 실패면 `None`.
    pub rust_normalized: Option<String>,
    pub csharp_normalized: Option<String>,
}

/// 양쪽 실행 결과로부터 판정을 만든다.
///
/// 판정 순서는 **의도적인** 것이다. 스텁(exit 3)을 가장 먼저 봐야 stdout 이 빈
/// 상태에서 "JSON 이 아니다" 하고 조용히 하네스 오류로 떨어지지 않는다.
///
/// 1. 모르는 op
/// 2. C# 가 exit 3 — 아직 못 만들었다. 비교가 아니다.
/// 3. op 표가 Uncomparable — 짝이 없다.
/// 4. C# 가 exit 2 / 그 밖의 0 아닌 코드 — 비교가 아니라 하네스 문제.
/// 5. Rust 가 0 이 아님 — 비교가 아니라 하네스 문제.
/// 6. 정규화 — 안 되면 하네스 문제.
/// 7. 문자열 비교.
pub fn compare<N>(op: &str, rust: &SideResult, csharp: &SideResult, normalize: &N) -> Verdict
where
    N: Fn(&str) -> Result<String, String>,
{
    let spec = match expectation(op) {
        Some(s) => s,
        None => {
            return Verdict {
                comparison: Comparison::UnknownOp {
                    op: op.to_string(),
                },
                rust_normalized: None,
                csharp_normalized: None,
            }
        }
    };

    let unverifiable = |comparison: Comparison| Verdict {
        comparison,
        rust_normalized: None,
        csharp_normalized: None,
    };

    // 2. C# 스텁. 도달 가능한 구현이 없다는 **실측** 사실이 표의 추정보다 강하다.
    if csharp.code == exit_code_contract::NOT_IMPLEMENTED {
        return unverifiable(Comparison::NotComparable {
            op: op.to_string(),
            reason: first_line(&csharp.stderr),
        });
    }

    // 3. 표가 짝이 없다고 말하는 op.
    if spec.expectation == Expectation::Uncomparable {
        return unverifiable(Comparison::NotComparable {
            op: op.to_string(),
            reason: spec.reason.to_string(),
        });
    }

    // 4. C# 쪽 하네스 문제.
    if csharp.code == exit_code_contract::USAGE {
        return unverifiable(Comparison::CsharpUsage {
            op: op.to_string(),
            stderr: first_line(&csharp.stderr),
        });
    }
    if csharp.code != exit_code_contract::SUCCESS {
        return unverifiable(Comparison::CsharpFailed {
            op: op.to_string(),
            stderr: first_line(&csharp.stderr),
        });
    }

    // 5. Rust 쪽 하네스 문제.
    if rust.code != exit_code_contract::SUCCESS {
        return unverifiable(Comparison::RustFailed {
            op: op.to_string(),
            stderr: first_line(&rust.stderr),
        });
    }

    // 6. 정규화.
    let rust_normalized = match normalize(&rust.stdout) {
        Ok(v) => v,
        Err(reason) => {
            return unverifiable(Comparison::UnparsableOutput {
                op: op.to_string(),
                side: "rust",
                reason,
            })
        }
    };
    let csharp_normalized = match normalize(&csharp.stdout) {
        Ok(v) => v,
        Err(reason) => {
            return Verdict {
                comparison: Comparison::UnparsableOutput {
                    op: op.to_string(),
                    side: "csharp",
                    reason,
                },
                rust_normalized: Some(rust_normalized),
                csharp_normalized: None,
            }
        }
    };

    // 7. 값 비교.
    let comparison = if rust_normalized == csharp_normalized {
        match spec.expectation {
            Expectation::Gate => Comparison::Match { op: op.to_string() },
            _ => Comparison::AdvisoryMatch {
                op: op.to_string(),
                reason: spec.reason.to_string(),
            },
        }
    } else {
        Comparison::Mismatch {
            op: op.to_string(),
        }
    };

    Verdict {
        comparison,
        rust_normalized: Some(rust_normalized),
        csharp_normalized: Some(csharp_normalized),
    }
}

/// stderr 첫 줄만 남긴다. 오라클은 여러 줄짜리 사유를 주지만 판정 표시는 짧아야 한다.
fn first_line(stderr: &str) -> String {
    let trimmed = stderr.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut lines = trimmed.lines();
    let head = lines.next().unwrap_or_default().to_string();
    let rest = lines.count();
    if rest == 0 {
        head
    } else {
        format!("{head} …(+{rest}줄)")
    }
}

/// 통합 diff 를 그릴 수 있는 입력 크기의 상한.
///
/// LCS 를 DP 표로 구하므로 O(n·m) 이다. 이 크기를 넘으면 전체 교체 hunks 로
/// 내려간다 — 오라클 JSON 은 수천 줄이 아니라 수십 줄이므로 현실적으로 걸리지 않는다.
const MAX_DIFF_LINES: usize = 2000;

/// 두 텍스트의 line diff 를 계산한다. 같으면 빈 문자열.
fn line_diff_ops(left: &[&str], right: &[&str]) -> Vec<(char, String)> {
    let (n, m) = (left.len(), right.len());

    if n == 0 {
        return right.iter().map(|l| ('+', (*l).to_string())).collect();
    }
    if m == 0 {
        return left.iter().map(|l| ('-', (*l).to_string())).collect();
    }

    if n.max(m) > MAX_DIFF_LINES {
        let mut ops: Vec<(char, String)> =
            left.iter().map(|l| ('-', (*l).to_string())).collect();
        ops.extend(right.iter().map(|l| ('+', (*l).to_string())));
        return ops;
    }

    // LCS 길이 DP 표.
    let mut dp = vec![0u32; (n + 1) * (m + 1)];
    let idx = |i: usize, j: usize| i * (m + 1) + j;
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[idx(i, j)] = if left[i] == right[j] {
                dp[idx(i + 1, j + 1)] + 1
            } else {
                dp[idx(i + 1, j)].max(dp[idx(i, j + 1)])
            };
        }
    }

    let mut ops = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < n && j < m {
        if left[i] == right[j] {
            ops.push((' ', left[i].to_string()));
            i += 1;
            j += 1;
        } else if dp[idx(i + 1, j)] >= dp[idx(i, j + 1)] {
            ops.push(('-', left[i].to_string()));
            i += 1;
        } else {
            ops.push(('+', right[j].to_string()));
            j += 1;
        }
    }
    while i < n {
        ops.push(('-', left[i].to_string()));
        i += 1;
    }
    while j < m {
        ops.push(('+', right[j].to_string()));
        j += 1;
    }
    ops
}

/// GNU `diff -u` 형태의 통합 diff.
///
/// 라벨은 호출자가 준다. 정규화된(키 정렬된) 문서에 대해서만 호출해야 한다.
pub fn unified_diff(
    left_label: &str,
    right_label: &str,
    left: &str,
    right: &str,
    context: usize,
) -> String {
    let left_lines: Vec<&str> = left.lines().collect();
    let right_lines: Vec<&str> = right.lines().collect();
    if left_lines == right_lines {
        return String::new();
    }

    let ops = line_diff_ops(&left_lines, &right_lines);

    // 바뀌지 않은 줄 중 연속 구간을 하나로 묶고, 주변 context 만 남긴다.
    let mut keep = vec![false; ops.len()];
    for (k, (tag, _)) in ops.iter().enumerate() {
        if *tag == ' ' {
            continue;
        }
        let lo = k.saturating_sub(context);
        let hi = (k + context).min(ops.len() - 1);
        for slot in keep.iter_mut().take(hi + 1).skip(lo) {
            *slot = true;
        }
    }

    // ops 인덱스 → 원본 줄 번호(1-based).
    let mut left_no = vec![0usize; ops.len()];
    let mut right_no = vec![0usize; ops.len()];
    let (mut ln, mut mut_rn) = (0usize, 0usize);
    for (k, (tag, _)) in ops.iter().enumerate() {
        match tag {
            ' ' => {
                ln += 1;
                mut_rn += 1;
                left_no[k] = ln;
                right_no[k] = mut_rn;
            }
            '-' => {
                ln += 1;
                left_no[k] = ln;
            }
            _ => {
                mut_rn += 1;
                right_no[k] = mut_rn;
            }
        }
    }

    let mut out = String::new();
    out.push_str(&format!("--- {left_label}\n"));
    out.push_str(&format!("+++ {right_label}\n"));

    let mut k = 0usize;
    while k < ops.len() {
        if !keep[k] {
            k += 1;
            continue;
        }
        let start = k;
        while k < ops.len() && keep[k] {
            k += 1;
        }
        let end = k - 1;

        let left_start = left_no[start].max(1);
        let right_start = right_no[start].max(1);
        let left_len = (start..=end).filter(|i| ops[*i].0 != '+').count();
        let right_len = (start..=end).filter(|i| ops[*i].0 != '-').count();

        out.push_str(&format!(
            "@@ -{},{} +{},{} @@\n",
            left_start,
            if left_len == 0 { 0 } else { left_len },
            right_start,
            if right_len == 0 { 0 } else { right_len }
        ));
        for (tag, text) in &ops[start..=end] {
            out.push(*tag);
            out.push_str(text);
            out.push('\n');
        }
    }
    out
}

/// 산출물을 어디에 남겼는지.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ArtifactPaths {
    pub directory: String,
}

/// 리포트를 그릴 때 필요한 실행 맥락.
#[derive(Clone, Debug, Default)]
pub struct ReportContext<'a> {
    pub op: &'a str,
    /// 양쪽에 넘긴 인자(양쪽 동일).
    pub argv: &'a [String],
    /// 실제로 실행한 Rust 쪽 명령 전체.
    pub rust_command: &'a [String],
    /// 실제로 실행한 C# 쪽 명령 전체.
    pub csharp_command: &'a [String],
    pub normalizer: Option<Normalizer>,
    /// 폴백 정규화기가 쓰였을 때 경고 문구.
    pub normalizer_warning: Option<&'a str>,
    /// 감지한 C# 저장소 HEAD (알 수 있을 때만).
    pub csharp_head: Option<&'a str>,
    pub artifacts: Option<&'a ArtifactPaths>,
}

/// 판정을 사람이 바로 행동할 수 있는 리포트로 그린다.
///
/// 스펙 `docs/oracle-spec.md:136-137` — *"차이 나는 이유는 항상 원인 파악 가능한
/// 형태여야 한다."* 그래서 불일치일 때는 ① op ② 실제 명령 ③ 값 차이 경로 ④ 통합 diff
/// ⑤ 산출물 경로를 모두 낸다. "FAIL" 한 줄로 끝내지 않는다.
pub fn render_report(verdict: &Verdict, ctx: &ReportContext<'_>) -> String {
    let mut out = String::new();
    out.push_str("================================================================\n");
    out.push_str(&format!("{}\n", verdict.comparison.headline()));
    out.push_str("================================================================\n");
    out.push_str(&format!(
        "exit code       : {}\n",
        exit_code(verdict.comparison.outcome())
    ));

    match &verdict.comparison {
        Comparison::AdvisoryMatch { reason, .. } => {
            out.push_str(&format!("advisory reason : {reason}\n"));
        }
        Comparison::NotComparable { reason, .. } => {
            out.push_str(&format!("reason          : {reason}\n"));
            out.push_str("                  이 op 는 아직 통과도 실패도 아니다.\n");
        }
        Comparison::CsharpUsage { stderr, .. } | Comparison::CsharpFailed { stderr, .. } => {
            out.push_str(&format!("stderr          : {stderr}\n"));
        }
        Comparison::RustFailed { stderr, .. } => {
            out.push_str(&format!("stderr          : {stderr}\n"));
        }
        Comparison::UnparsableOutput { side, reason, .. } => {
            out.push_str(&format!("side            : {side}\n"));
            out.push_str(&format!("reason          : {reason}\n"));
        }
        Comparison::UnknownOp { .. } => {
            out.push_str("known ops       : tree image tooltip stats expr diff mapscene patch avatar timeline\n");
        }
        Comparison::Match { .. } | Comparison::Mismatch { .. } => {}
    }

    out.push('\n');
    out.push_str(&format!("op              : {}\n", ctx.op));
    out.push_str(&format!("args            : {}\n", ctx.argv.join(" ")));
    out.push_str(&format!("rust            : {}\n", ctx.rust_command.join(" ")));
    out.push_str(&format!("csharp          : {}\n", ctx.csharp_command.join(" ")));
    if let Some(head) = &ctx.csharp_head {
        out.push_str(&format!("csharp HEAD     : {head}\n"));
    }
    if let Some(n) = ctx.normalizer {
        out.push_str(&format!("normalizer      : {}\n", n.label()));
    }
    if let Some(warning) = &ctx.normalizer_warning {
        for line in warning.lines() {
            out.push_str(&format!("normalizer warn : {line}\n"));
        }
    }

    if let Some(paths) = &ctx.artifacts {
        out.push_str(&format!("artifacts       : {}\n", paths.directory));
    }

    if let (Some(rust_norm), Some(csharp_norm)) =
        (&verdict.rust_normalized, &verdict.csharp_normalized)
    {
        if !rust_norm.is_empty() && rust_norm != csharp_norm {
            let diffs = value_differences(rust_norm, csharp_norm);
            out.push_str("\nvalue differences (normalised, key-sorted):\n");
            if diffs.is_empty() {
                out.push_str("  (없음 — 구조는 같고 표기만 다르거나, JSON 이 아니다)\n");
            } else {
                for d in &diffs {
                    out.push_str(&format!("  {d}\n"));
                }
            }
            out.push_str("\nunified diff:\n");
            out.push_str(&unified_diff(
                "csharp (normalised)",
                "rust (normalised)",
                csharp_norm,
                rust_norm,
                3,
            ));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_is_zero_and_mismatch_is_one() {
        assert_eq!(exit_code(Outcome::Match), 0);
        assert_eq!(exit_code(Outcome::Mismatch), 1);
    }

    #[test]
    fn not_comparable_is_three_and_harness_error_is_four() {
        // 3 은 C# 오라클이 "아직 못 만들었다" 에 쓰는 코드와 같다. 같은 의미여야 한다.
        assert_eq!(exit_code(Outcome::NotComparable), 3);
        assert_eq!(exit_code(Outcome::HarnessError), 4);
    }

    #[test]
    fn tool_usage_is_two() {
        assert_eq!(exit_code(Outcome::ToolUsage), 2);
    }

    #[test]
    fn tooltip_is_advisory_and_never_a_gate() {
        let e = expectation("tooltip").expect("tooltip 은 표에 있다");
        assert_eq!(e.expectation, Expectation::Advisory);
        assert!(e.reason.contains("Bitmap"), "근거가 있어야 한다: {}", e.reason);
    }

    #[test]
    fn tree_image_stats_expr_are_gates() {
        for op in ["tree", "image", "stats", "expr"] {
            let e = expectation(op).expect("표에 있어야 한다");
            assert_eq!(e.expectation, Expectation::Gate, "{op} 는 게이트");
        }
    }

    #[test]
    fn diff_mapscene_patch_avatar_timeline_are_uncomparable() {
        for op in ["diff", "mapscene", "patch", "avatar", "timeline"] {
            let e = expectation(op).expect("표에 있어야 한다");
            assert_eq!(e.expectation, Expectation::Uncomparable, "{op}");
            assert!(!e.reason.is_empty());
        }
    }

    #[test]
    fn unknown_op_has_no_expectation() {
        assert_eq!(expectation("bogus"), None);
        assert_eq!(expectation(""), None);
    }

    #[test]
    fn jq_mode_without_jq_fails_with_an_install_command() {
        let err = resolve_normalizer(NormalizerMode::Jq, false).unwrap_err();
        assert!(err.contains("winget install"), "설치 방법이 있어야 한다: {err}");
        assert!(
            err.contains("jq 가 설치되어 있지 않습니다"),
            "{err}"
        );
    }

    #[test]
    fn jq_mode_with_jq_uses_jq() {
        assert_eq!(
            resolve_normalizer(NormalizerMode::Jq, true).unwrap(),
            Normalizer::Jq
        );
    }

    #[test]
    fn auto_mode_falls_back_to_builtin_and_says_so_in_its_label() {
        let n = resolve_normalizer(NormalizerMode::Auto, false).unwrap();
        assert_eq!(n, Normalizer::Builtin);
        assert!(
            n.label().contains("builtin"),
            "폴백임을 라벨이 드러내야 한다"
        );
    }

    #[test]
    fn auto_mode_prefers_jq_when_present() {
        assert_eq!(
            resolve_normalizer(NormalizerMode::Auto, true).unwrap(),
            Normalizer::Jq
        );
    }

    #[test]
    fn builtin_normalizer_sorts_keys_at_every_depth() {
        let src = r#"{"b":1,"a":{"z":[{"q":1,"p":2}],"y":3}}"#;
        let out = normalize_builtin(src).unwrap();
        let a = out.find("\"a\"").unwrap();
        let b = out.find("\"b\"").unwrap();
        assert!(a < b, "최상위 키 정렬:\n{out}");
        assert!(
            out.find("\"p\"").unwrap() < out.find("\"q\"").unwrap(),
            "배열 안 객체:\n{out}"
        );
        assert!(
            out.find("\"y\"").unwrap() < out.find("\"z\"").unwrap(),
            "중첩:\n{out}"
        );
    }

    #[test]
    fn builtin_normalizer_is_order_independent() {
        let a = normalize_builtin(r#"{"x":1,"y":2}"#).unwrap();
        let b = normalize_builtin(r#"{"y":2,"x":1}"#).unwrap();
        assert_eq!(a, b, "키 순서만 다른 JSON 은 정규화 후 같아야 한다");
    }

    #[test]
    fn builtin_normalizer_rejects_non_json() {
        let err = normalize_builtin("not json at all").unwrap_err();
        assert!(err.contains("JSON 이 아닙니다"), "{err}");
    }

    #[test]
    fn builtin_normalizer_preserves_number_shape() {
        // `1.0` 을 `1` 로 접으면 값 비교가 아니라 표기 비교가 된다.
        assert_eq!(
            normalize_builtin("[1.0, 1e3, 0.1]")
                .unwrap()
                .replace([' ', '\n'], ""),
            "[1.0,1000.0,0.1]"
        );
    }

    #[test]
    fn value_differences_reports_a_pointer_not_just_a_diff() {
        let a = r#"{"result":{"node_count":8,"items":[1,2,3]}}"#;
        let b = r#"{"result":{"node_count":5,"items":[1,2,3]}}"#;
        let diffs = value_differences(a, b);
        assert_eq!(diffs.len(), 1, "{diffs:?}");
        assert_eq!(diffs[0].pointer, "/result/node_count");
        assert_eq!(diffs[0].rust, "8");
        assert_eq!(diffs[0].csharp, "5");
    }

    #[test]
    fn value_differences_reports_absent_keys_on_either_side() {
        let diffs = value_differences(r#"{"a":1}"#, r#"{"a":1,"b":2}"#);
        assert_eq!(diffs.len(), 1);
        assert_eq!(diffs[0].pointer, "/b");
        assert_eq!(diffs[0].rust, "<absent>");
        assert_eq!(diffs[0].csharp, "<present>");
    }

    #[test]
    fn value_differences_flags_array_length_before_elements() {
        let diffs = value_differences(r#"{"c":[1,2,3]}"#, r#"{"c":[1,2]}"#);
        assert_eq!(diffs.len(), 1, "{diffs:?}");
        assert_eq!(diffs[0].pointer, "/c/length");
    }

    #[test]
    fn value_differences_escapes_pointer_tokens() {
        let diffs = value_differences(r#"{"a/b":1}"#, r#"{"a/b":2}"#);
        assert_eq!(diffs[0].pointer, "/a~1b");
    }

    #[test]
    fn value_differences_is_empty_for_equal_json() {
        assert!(value_differences(r#"{"a":[1,{"b":2}]}"#, r#"{"a":[1,{"b":2}]}"#).is_empty());
    }

    #[test]
    fn value_differences_tolerates_non_json_without_panicking() {
        assert!(value_differences("<<<", "{}").is_empty());
    }

    #[test]
    fn value_differences_is_sorted_by_pointer() {
        let diffs = value_differences(r#"{"z":1,"a":1}"#, r#"{"z":2,"a":2}"#);
        assert_eq!(diffs[0].pointer, "/a");
        assert_eq!(diffs[1].pointer, "/z");
    }

    // ── 판정 (compare) ────────────────────────────────────────────────

    fn verdict(op: &str, rust: SideResult, csharp: SideResult) -> Verdict {
        compare(op, &rust, &csharp, &normalize_builtin)
    }

    #[test]
    fn identical_normalised_json_on_a_gate_op_is_a_match() {
        let v = verdict(
            "expr",
            SideResult::ok(r#"{"op":"expr","schema_version":1,"result":{"value":1}}"#),
            SideResult::ok(r#"{"op":"expr","schema_version":1,"result":{"value":1}}"#),
        );
        assert_eq!(
            v.comparison,
            Comparison::Match {
                op: "expr".into()
            }
        );
        assert_eq!(v.comparison.outcome(), Outcome::Match);
        assert_eq!(exit_code(v.comparison.outcome()), 0);
    }

    #[test]
    fn different_key_order_is_not_a_mismatch_because_normalisation_happens_first() {
        let a = r#"{"op":"expr","schema_version":1}"#;
        let b = r#"{"schema_version":1,"op":"expr"}"#;
        let v = verdict("expr", SideResult::ok(a), SideResult::ok(b));
        assert_eq!(v.comparison.outcome(), Outcome::Match, "{:?}", v.comparison);
    }

    #[test]
    fn different_values_are_a_mismatch_with_exit_one() {
        let v = verdict(
            "expr",
            SideResult::ok(r#"{"result":{"value":6.333333333333333}}"#),
            SideResult::ok(r#"{"result":{"value":6.333333333333334}}"#),
        );
        assert_eq!(
            v.comparison,
            Comparison::Mismatch {
                op: "expr".into()
            }
        );
        assert_eq!(exit_code(v.comparison.outcome()), 1);
    }

    #[test]
    fn csharp_exit_three_is_not_comparable_never_a_pass_or_a_fail() {
        let v = verdict(
            "diff",
            SideResult::ok("{}"),
            SideResult::failed(
                exit_code_contract::NOT_IMPLEMENTED,
                "not-implemented: WzComparerR2.Comparer.WzFileComparer …\nmore detail\n",
            ),
        );
        match &v.comparison {
            Comparison::NotComparable { op, reason } => {
                assert_eq!(op, "diff");
                assert!(reason.starts_with("not-implemented:"), "{reason}");
            }
            other => panic!("스텁은 비교 불가여야 한다: {other:?}"),
        }
        assert_eq!(exit_code(v.comparison.outcome()), 3);
        assert_ne!(exit_code(v.comparison.outcome()), 0);
        assert_ne!(exit_code(v.comparison.outcome()), 1);
    }

    #[test]
    fn csharp_stub_beats_the_op_table_and_beats_json_parsing() {
        // C# 은 스텁일 때 stdout 이 비어 있다. JSON 정규화를 먼저 시도하면
        // "stdout 이 JSON 이 아니다" 라고 하네스 오류로 잘못 떨어진다.
        let v = verdict(
            "mapscene",
            SideResult::ok("{}"),
            SideResult::failed(3, "not-implemented: x"),
        );
        assert_eq!(v.comparison.outcome(), Outcome::NotComparable);
        assert_eq!(v.rust_normalized, None, "스텁이면 정규화하지 않는다");
    }

    #[test]
    fn uncomparable_op_stays_uncomparable_even_if_csharp_were_to_answer() {
        let v = verdict("patch", SideResult::ok("{}"), SideResult::ok("{}"));
        assert_eq!(v.comparison.outcome(), Outcome::NotComparable);
    }

    #[test]
    fn csharp_exit_two_is_a_harness_error_not_a_mismatch() {
        let v = verdict(
            "expr",
            SideResult::ok("{}"),
            SideResult::failed(2, "사용법 오류: unknown flag: --format"),
        );
        assert_eq!(v.comparison.outcome(), Outcome::HarnessError);
        assert_eq!(exit_code(v.comparison.outcome()), 4);
    }

    #[test]
    fn csharp_exit_one_is_a_harness_error_because_no_comparison_happened() {
        let v = verdict(
            "tree",
            SideResult::ok("{}"),
            SideResult::failed(1, "오류: 파일 접근 실패"),
        );
        assert_eq!(v.comparison.outcome(), Outcome::HarnessError);
    }

    #[test]
    fn rust_failure_is_a_harness_error() {
        let v = verdict(
            "tree",
            SideResult::failed(1, "오류: 파일 접근 실패"),
            SideResult::ok("{}"),
        );
        assert_eq!(
            v.comparison,
            Comparison::RustFailed {
                op: "tree".into(),
                stderr: "오류: 파일 접근 실패".into()
            }
        );
        assert_eq!(v.comparison.outcome(), Outcome::HarnessError);
    }

    #[test]
    fn unknown_op_is_a_tool_usage_error() {
        let v = verdict("bogus", SideResult::ok("{}"), SideResult::ok("{}"));
        assert_eq!(
            v.comparison,
            Comparison::UnknownOp {
                op: "bogus".into()
            }
        );
        assert_eq!(exit_code(v.comparison.outcome()), 2);
    }

    #[test]
    fn tooltip_match_is_reported_as_advisory_and_never_as_a_gate() {
        let v = verdict("tooltip", SideResult::ok("{}"), SideResult::ok("{}"));
        assert_eq!(v.comparison.outcome(), Outcome::NotComparable);
        assert_ne!(exit_code(v.comparison.outcome()), 0, "통과로 보이면 안 된다");
        assert!(v.comparison.headline().contains("ADVISORY"));
    }

    #[test]
    fn tooltip_mismatch_is_still_a_mismatch_with_exit_one() {
        // Advisory 등급은 "일치를 통과로 승격하지 않는다"는 뜻이지
        // "진짜 불일치를 숨긴다"는 뜻이 아니다.
        let v = verdict(
            "tooltip",
            SideResult::ok(r#"{"result":{"text":"a\nb"}}"#),
            SideResult::ok(r#"{"result":{"text":"a\nc"}}"#),
        );
        assert_eq!(v.comparison.outcome(), Outcome::Mismatch);
        assert_eq!(exit_code(v.comparison.outcome()), 1);
    }

    #[test]
    fn non_json_stdout_is_a_harness_error_naming_the_side() {
        let v = verdict("expr", SideResult::ok("not json"), SideResult::ok("{}"));
        match &v.comparison {
            Comparison::UnparsableOutput { side, .. } => assert_eq!(*side, "rust"),
            other => panic!("{other:?}"),
        }
        assert_eq!(v.comparison.outcome(), Outcome::HarnessError);
    }

    #[test]
    fn multi_line_stderr_is_collapsed_to_one_line_plus_a_count() {
        let v = verdict(
            "diff",
            SideResult::ok("{}"),
            SideResult::failed(3, "line one\nline two\nline three\n"),
        );
        match &v.comparison {
            Comparison::NotComparable { reason, .. } => {
                assert_eq!(reason, "line one …(+2줄)");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn verdict_keeps_both_normalised_documents_for_the_report() {
        let v = verdict(
            "expr",
            SideResult::ok(r#"{"b":1,"a":2}"#),
            SideResult::ok(r#"{"a":3,"b":1}"#),
        );
        assert!(v.rust_normalized.is_some() && v.csharp_normalized.is_some());
        assert_ne!(v.rust_normalized, v.csharp_normalized);
    }

    // ── 진단 가능한 리포트 ────────────────────────────────────────────

    #[test]
    fn unified_diff_is_empty_for_identical_text() {
        assert_eq!(unified_diff("a", "b", "x\ny\n", "x\ny\n", 3), "");
    }

    #[test]
    fn unified_diff_marks_a_single_changed_line() {
        let d = unified_diff("l", "r", "a\nb\nc\n", "a\nB\nc\n", 3);
        assert!(d.contains("-b"), "{d}");
        assert!(d.contains("+B"), "{d}");
        assert!(d.contains("@@"), "{d}");
    }

    #[test]
    fn unified_diff_far_apart_changes_produce_two_hunks() {
        let left: String = (0..40).map(|i| format!("line{i}\n")).collect();
        let right = left
            .replace("line1\n", "LINE1\n")
            .replace("line38\n", "LINE38\n");
        let d = unified_diff("l", "r", &left, &right, 2);
        assert_eq!(d.matches("@@ -").count(), 2, "{d}");
        assert!(d.contains("-line1"), "{d}");
        assert!(d.contains("-line38"), "{d}");
    }

    #[test]
    fn unified_diff_of_pure_addition_keeps_context() {
        let d = unified_diff("l", "r", "a\nz\n", "a\nb\nz\n", 1);
        assert!(d.contains("+b"), "{d}");
        assert!(
            !d.contains("\n-a\n"),
            "삭제 없는 줄이 - 로 나오면 안 된다: {d}"
        );
    }

    #[test]
    fn report_names_the_op_the_commands_and_the_value_paths() {
        let v = verdict(
            "expr",
            SideResult::ok(r#"{"result":{"value":6.333333333333333}}"#),
            SideResult::ok(r#"{"result":{"value":6.333333333333334}}"#),
        );
        let ctx = ReportContext {
            op: "expr",
            argv: &["--expr".to_string(), "1/3 + 2*3".to_string()],
            rust_command: &["wz".into(), "oracle".into(), "expr".into()],
            csharp_command: &["dotnet".into(), "Oracle.dll".into(), "expr".into()],
            normalizer: Some(Normalizer::Builtin),
            normalizer_warning: None,
            csharp_head: Some("33828fe"),
            artifacts: None,
        };
        let report = render_report(&v, &ctx);
        assert!(report.contains("MISMATCH"), "{report}");
        assert!(report.contains("op=expr"), "{report}");
        assert!(report.contains("op              : expr"), "{report}");
        assert!(report.contains("1/3 + 2*3"), "{report}");
        assert!(report.contains("dotnet"), "{report}");
        assert!(report.contains("33828fe"), "{report}");
        assert!(
            report.contains("/result/value"),
            "어느 필드가 틀렸는지 나와야 한다:\n{report}"
        );
        assert!(report.contains("6.333333333333333"), "{report}");
        assert!(report.contains("6.333333333333334"), "{report}");
        assert!(report.contains("exit code       : 1"), "{report}");
    }

    #[test]
    fn report_for_a_match_stays_short_and_does_not_print_a_diff() {
        let v = verdict("expr", SideResult::ok("{}"), SideResult::ok("{}"));
        let ctx = ReportContext {
            op: "expr",
            ..Default::default()
        };
        let report = render_report(&v, &ctx);
        assert!(report.contains("MATCH"), "{report}");
        assert!(!report.contains("unified diff"), "{report}");
        assert!(report.contains("exit code       : 0"), "{report}");
    }

    #[test]
    fn report_for_a_csharp_stub_says_it_is_neither_pass_nor_fail() {
        let v = verdict(
            "diff",
            SideResult::ok("{}"),
            SideResult::failed(3, "not-implemented: nope"),
        );
        let report = render_report(&v, &ReportContext {
            op: "diff",
            ..Default::default()
        });
        assert!(report.contains("NOT COMPARABLE"), "{report}");
        assert!(report.contains("아직 통과도 실패도 아니다"), "{report}");
        assert!(report.contains("exit code       : 3"), "{report}");
        assert!(report.contains("not-implemented: nope"), "{report}");
    }

    #[test]
    fn report_surfaces_the_fallback_normalizer_warning() {
        let v = verdict("expr", SideResult::ok("{}"), SideResult::ok("{}"));
        let ctx = ReportContext {
            op: "expr",
            normalizer: Some(Normalizer::Builtin),
            normalizer_warning: Some("jq 가 설치되어 있지 않습니다 (폴백 사용)"),
            ..Default::default()
        };
        let report = render_report(&v, &ctx);
        assert!(report.contains("normalizer warn"), "{report}");
        assert!(report.contains("폴백 사용"), "{report}");
    }

    #[test]
    fn report_for_tooltip_says_advisory_and_not_a_gate() {
        let v = verdict("tooltip", SideResult::ok("{}"), SideResult::ok("{}"));
        let report = render_report(&v, &ReportContext {
            op: "tooltip",
            ..Default::default()
        });
        assert!(report.contains("ADVISORY MATCH"), "{report}");
        assert!(report.contains("Bitmap"), "근거가 붙어야 한다:\n{report}");
        assert!(report.contains("exit code       : 3"), "{report}");
    }

    #[test]
    fn report_lists_known_ops_when_the_op_is_unknown() {
        let v = verdict("bogus", SideResult::ok("{}"), SideResult::ok("{}"));
        let report = render_report(&v, &ReportContext {
            op: "bogus",
            ..Default::default()
        });
        assert!(report.contains("known ops"), "{report}");
        assert!(report.contains("mapscene"), "{report}");
    }
}