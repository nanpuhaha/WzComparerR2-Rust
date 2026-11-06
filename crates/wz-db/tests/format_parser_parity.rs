//! `WzComparerR2.Text.Parser.Parse`의 run 분해를 **원본 C# 실행 결과**와 비교한다.
//!
//! C# 하네스는 `WzComparerR2.Common`의 실제 `Parser.Parse`를 호출한다(GDI 비의존).
//! 입력은 `Data/String/String_000.wz`에서 `#` 토큰이 있는 실제 문자열 151개로,
//! `#t`/`#fc`/`#$`/`#@`/`#k`/`#c`와 개행 이스케이프를 모두 포함한다.

use serde::Deserialize;
use wz_db::format_parser::{parse, DocElement};

#[derive(Deserialize)]
struct Input {
    path: String,
    text: String,
}

#[derive(Deserialize)]
struct Expected {
    path: String,
    runs: Vec<Run>,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
struct Run {
    kind: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    font: Option<String>,
    #[serde(default)]
    image_id: Option<String>,
    #[serde(default)]
    image_width: Option<i32>,
    #[serde(default)]
    image_height: Option<i32>,
}

fn expected_run(element: &DocElement) -> Run {
    match element {
        DocElement::LineBreak => Run {
            kind: "break".into(),
            text: None,
            color: None,
            font: None,
            image_id: None,
            image_width: None,
            image_height: None,
        },
        DocElement::Span(span) => Run {
            kind: "span".into(),
            text: Some(span.text.clone()),
            color: Some(span.color_id.clone()),
            font: Some(span.font_id.clone()),
            image_id: span.image_id.clone(),
            image_width: Some(span.image_width),
            image_height: Some(span.image_height),
        },
    }
}

#[test]
fn format_parser_matches_csharp() {
    let inputs: Vec<Input> =
        serde_json::from_str(include_str!("fixtures/format_parser_input.json")).unwrap();
    let expected: Vec<Expected> =
        serde_json::from_str(include_str!("fixtures/format_parser_expected.json")).unwrap();
    assert_eq!(inputs.len(), expected.len());
    assert!(inputs.len() >= 150);

    let mut total_runs = 0usize;
    for (input, want) in inputs.iter().zip(expected.iter()) {
        assert_eq!(input.path, want.path);
        let runs: Vec<Run> = parse(&input.text).iter().map(expected_run).collect();
        total_runs += runs.len();
        assert_eq!(
            runs, want.runs,
            "runs for {} ({:?})",
            input.path, input.text
        );
    }
    assert!(total_runs >= 800, "too few runs: {total_runs}");
}
