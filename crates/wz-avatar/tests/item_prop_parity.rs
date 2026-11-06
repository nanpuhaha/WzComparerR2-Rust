//! `ItemStringHelper.GetItemPropString`을 실제 C# 실행 결과와 비교한다.
//!
//! 입력은 실제 아이템 `info`에서 뽑은 300개(대부분 tradeBlock) + 경계 3개이고,
//! C# 하네스가 실제 `ItemStringHelper.GetItemPropString`을 호출한다.

use serde::Deserialize;
use wz_avatar::item_prop_string;

#[derive(Deserialize)]
struct Case {
    id: i32,
    key: String,
    value: i64,
}

#[derive(Deserialize)]
struct Expected {
    id: i32,
    key: String,
    value: i64,
    text: Option<String>,
    #[serde(default)]
    error: Option<String>,
}

#[test]
fn item_prop_strings_match_csharp() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/item_prop_cases.json")).unwrap();
    let expected: Vec<Expected> =
        serde_json::from_str(include_str!("fixtures/item_prop_expected.json")).unwrap();
    assert_eq!(cases.len(), expected.len());
    assert!(cases.len() >= 300);

    for (case, want) in cases.iter().zip(expected.iter()) {
        assert_eq!(case.id, want.id);
        assert_eq!(case.key, want.key);
        assert_eq!(case.value, want.value);
        assert!(want.error.is_none(), "{}: {:?}", want.key, want.error);
        assert_eq!(
            item_prop_string(&case.key, case.value),
            want.text,
            "id {} key {} value {}",
            case.id,
            case.key,
            case.value
        );
    }
}
