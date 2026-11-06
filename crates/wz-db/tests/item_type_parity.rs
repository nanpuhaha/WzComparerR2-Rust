//! `Item.GetItemType` + `GetItemCategoryName`을 실제 C# 실행과 비교한다.
//!
//! 입력은 실제 `Data/Item`의 아이템 ID 598개 + 경계 18개다.

use serde::Deserialize;
use wz_db::{item_category_name, item_type};

#[derive(Deserialize)]
struct Expected {
    id: i32,
    #[serde(rename = "type")]
    type_name: String,
    category: Option<String>,
}

#[test]
fn item_types_and_categories_match_csharp() {
    let expected: Vec<Expected> =
        serde_json::from_str(include_str!("fixtures/item_type_expected.json")).unwrap();
    assert!(expected.len() >= 600);
    for want in expected {
        let kind = item_type(want.id);
        assert_eq!(kind.name(), want.type_name, "type for {}", want.id);
        assert_eq!(
            item_category_name(kind.name()).map(str::to_string),
            want.category,
            "category for {}",
            want.id
        );
    }
}
