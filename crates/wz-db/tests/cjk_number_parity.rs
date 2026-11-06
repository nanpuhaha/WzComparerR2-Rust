//! `ItemStringHelper.ToCJKNumberExpr`(plain/detailed)을 실제 C# 실행과 비교한다.
//!
//! 입력은 실제 아이템 `info/price` 값 120여 개 + 경계 15개다.

use serde::Deserialize;
use wz_db::to_cjk_number_expr;

#[derive(Deserialize)]
struct Expected {
    value: i64,
    plain: String,
    detailed: String,
}

#[test]
fn cjk_number_matches_csharp() {
    let expected: Vec<Expected> =
        serde_json::from_str(include_str!("fixtures/cjk_number_expected.json")).unwrap();
    assert!(expected.len() >= 125);
    for want in expected {
        assert_eq!(
            to_cjk_number_expr(want.value, false),
            want.plain,
            "plain {}",
            want.value
        );
        assert_eq!(
            to_cjk_number_expr(want.value, true),
            want.detailed,
            "detailed {}",
            want.value
        );
    }
}
