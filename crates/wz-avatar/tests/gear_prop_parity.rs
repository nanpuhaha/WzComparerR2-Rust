//! `GetGearPropString(propType, value, signFlag)`의 signFlag 0/1/2를 실제 C# 실행과 비교한다.
//!
//! 입력은 실제 `Data/Character` 장비 `info`의 서로 다른 (키, 값) 33개 +
//! 음수/0/양수 경계 24개(6키 × 4값)이고, C# 하네스가 실제 메서드를 호출한다.

use serde::Deserialize;
use wz_avatar::gear_prop_line_with_sign;

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
    text: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[test]
fn gear_prop_sign_flags_match_csharp() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/gear_prop_cases.json")).unwrap();
    let expected: Vec<Expected> =
        serde_json::from_str(include_str!("fixtures/gear_prop_expected.json")).unwrap();
    assert_eq!(cases.len() * 3, expected.len());
    assert!(cases.len() >= 50);

    let mut index = 0usize;
    for case in &cases {
        for flag in 0..=2 {
            let want = &expected[index];
            index += 1;
            assert_eq!(case.key, want.key);
            assert_eq!(case.value, want.value);
            assert_eq!(flag, want.flag);
            assert!(want.error.is_none(), "{} unknown", case.key);
            assert_eq!(
                gear_prop_line_with_sign(&case.key, case.value, flag),
                want.text,
                "key {} value {} flag {}",
                case.key,
                case.value,
                flag
            );
        }
    }
}
