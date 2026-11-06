//! `GetGearPropString22(propType, value, signFlag)` 2열 포맷을 실제 C#과 비교한다.
//!
//! 입력은 `gear_prop_cases.json`(실제 장비 54케이스)이고, C# 하네스가 실제
//! 메서드를 signFlag 0/1/2로 호출해 162행을 만든다.

use serde::Deserialize;
use wz_avatar::gear_prop_pair;

#[derive(Deserialize)]
struct Case {
    key: String,
    value: i64,
}

#[derive(Deserialize)]
struct Expected {
    key: String,
    value: i64,
    flag: i32,
    pair_null: bool,
    label: Option<String>,
    value_text: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[test]
fn gear_prop_pairs_match_csharp() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/gear_prop_cases.json")).unwrap();
    let expected: Vec<Expected> =
        serde_json::from_str(include_str!("fixtures/gear_prop_pairs_expected.json")).unwrap();
    assert_eq!(cases.len() * 3, expected.len());

    let mut index = 0usize;
    for case in &cases {
        for flag in 0..=2 {
            let want = &expected[index];
            index += 1;
            assert_eq!(case.key, want.key);
            assert_eq!(case.value, want.value);
            assert_eq!(flag, want.flag);
            assert!(want.error.is_none(), "{} unknown", case.key);
            let got = gear_prop_pair(&case.key, case.value, flag);
            assert_eq!(
                got.is_none(),
                want.pair_null,
                "pair null {} {} {}",
                case.key,
                case.value,
                flag
            );
            if let Some((label, value_text)) = got {
                assert_eq!(
                    label, want.label,
                    "label {} {} {}",
                    case.key, case.value, flag
                );
                assert_eq!(
                    value_text, want.value_text,
                    "value {} {} {}",
                    case.key, case.value, flag
                );
            }
        }
    }
}
