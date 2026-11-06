//! `ItemStringHelper.GetGearTypeString`을 실제 C# 실행과 비교한다.
//!
//! C# 하네스가 `Enum.GetNames(typeof(GearType))` 전체(161개)에 대해 실제
//! 메서드를 호출한 결과와 비교한다.

use serde::Deserialize;
use wz_db::gear_type_string;

#[derive(Deserialize)]
struct Expected {
    name: String,
    text: Option<String>,
}

#[test]
fn gear_type_names_match_csharp() {
    let expected: Vec<Expected> =
        serde_json::from_str(include_str!("fixtures/gear_type_names_expected.json")).unwrap();
    assert_eq!(expected.len(), 161);
    for want in expected {
        assert_eq!(
            gear_type_string(&want.name).map(str::to_string),
            want.text,
            "{}",
            want.name
        );
    }
}
