//! `Potential` 요약/문자열/옵션타입/레벨을 실제 C# 실행과 비교한다.
//!
//! 입력은 실제 `Item/ItemOption.img`에서 뽑은 600개 `(code, level)`이고,
//! C# 하네스가 실제 `Potential.CreateFromNode` + `ConvertSummary` + `ToString`
//! + `CheckOptionType` + `GetPotentialLevel`을 실행한다.

use serde::Deserialize;
use std::collections::BTreeMap;
use wz_db::potential::{
    check_option_type, convert_summary, is_potential_ex, potential_level, potential_to_string,
};

#[derive(Deserialize)]
struct Fixture {
    code: i32,
    level: i32,
    option_type: i32,
    req_level: i32,
    weight: i32,
    summary: String,
    props: BTreeMap<String, i64>,
    props_f: BTreeMap<String, f64>,
}

#[derive(Deserialize)]
struct Expected {
    rows: Vec<Row>,
    option_checks: Vec<GearChecks>,
    levels: Vec<LevelRow>,
}

#[derive(Deserialize)]
struct Row {
    code: i32,
    level: i32,
    created: bool,
    summary: Option<String>,
    to_string: Option<String>,
    is_ex: Option<bool>,
    option_type: Option<i32>,
    req_level: Option<i32>,
    weight: Option<i32>,
}

#[derive(Deserialize)]
struct GearChecks {
    gear_type: String,
    checks: Vec<Check>,
}

#[derive(Deserialize)]
struct Check {
    option_type: i32,
    allowed: bool,
}

#[derive(Deserialize)]
struct LevelRow {
    req_level: i32,
    level: i32,
}

#[test]
fn potential_summary_and_checks_match_csharp() {
    let cases: Vec<Fixture> =
        serde_json::from_str(include_str!("fixtures/potential_cases.json")).unwrap();
    let expected: Expected =
        serde_json::from_str(include_str!("fixtures/potential_expected.json")).unwrap();
    assert_eq!(cases.len(), expected.rows.len());
    assert_eq!(cases.len(), 600);

    for (case, want) in cases.iter().zip(expected.rows.iter()) {
        assert_eq!(case.code, want.code);
        assert_eq!(case.level, want.level);
        assert!(
            want.created,
            "C# failed to create {} level {}",
            case.code, case.level
        );
        assert_eq!(
            Some(case.option_type),
            want.option_type,
            "optionType {}",
            case.code
        );
        assert_eq!(
            Some(case.req_level),
            want.req_level,
            "reqLevel {}",
            case.code
        );
        assert_eq!(Some(case.weight), want.weight, "weight {}", case.code);

        let summary = convert_summary(&case.summary, &case.props, &case.props_f);
        assert_eq!(
            summary, want.summary,
            "summary {} level {}",
            case.code, case.level
        );
        assert_eq!(
            potential_to_string(case.code, summary.as_deref(), case.weight),
            want.to_string.clone().expect("to_string"),
            "to_string {} level {}",
            case.code,
            case.level
        );
        assert_eq!(
            Some(is_potential_ex(case.code)),
            want.is_ex,
            "is_ex {}",
            case.code
        );
    }

    for gear in &expected.option_checks {
        for check in &gear.checks {
            assert_eq!(
                check_option_type(check.option_type, &gear.gear_type),
                check.allowed,
                "optionType {} gear {}",
                check.option_type,
                gear.gear_type
            );
        }
    }

    for row in &expected.levels {
        assert_eq!(
            potential_level(row.req_level),
            row.level,
            "potential level for {}",
            row.req_level
        );
    }
}
