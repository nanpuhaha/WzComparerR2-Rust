//! ItemTooltip 세로 배치(`picH`/`splitterH`)를 C# 참조 구현과 비교한다.
//!
//! 입력은 실제 `Data/Item/Consume` 18,376개에서 기능별로 뽑은 320개
//! (하단 속성 100, 사용기간 113, 레시피 60, 설명만 60)이고, 텍스트 높이는
//! `wrapped_lines`(40칸 접기)로 결정적으로 계산해 양쪽에 같은 값을 넣는다.

use serde::Deserialize;
use wz_db::item_tooltip_layout::{layout_item, ItemLayoutInput};

#[derive(Deserialize)]
struct Case {
    id: i32,
    wonder_grade: i64,
    bts_label: i64,
    blackpink_label: i64,
    expire: bool,
    no_revive: bool,
    req_level_height: Option<i32>,
    pet_stats_height: Option<i32>,
    desc_height: Option<i32>,
    incline_height: Option<i32>,
    recipe_height: Option<i32>,
    flat_rate_height: Option<i32>,
    top_attr_heights: Vec<i32>,
    bottom_attr_heights: Vec<i32>,
}

#[derive(Deserialize)]
struct Expected {
    id: i32,
    pic_h: i32,
    icon_y: i32,
    splitter_h: Vec<i32>,
}

#[test]
fn item_tooltip_layout_matches_csharp() {
    let cases: Vec<Case> =
        serde_json::from_str(include_str!("fixtures/item_layout_input.json")).unwrap();
    let expected: Vec<Expected> =
        serde_json::from_str(include_str!("fixtures/item_layout_expected.json")).unwrap();
    assert_eq!(cases.len(), expected.len());
    assert!(cases.len() >= 300);

    for (case, want) in cases.iter().zip(expected.iter()) {
        assert_eq!(case.id, want.id);
        let layout = layout_item(&ItemLayoutInput {
            wonder_grade: case.wonder_grade,
            bts_label: case.bts_label,
            blackpink_label: case.blackpink_label,
            top_attr_heights: case.top_attr_heights.clone(),
            expire: case.expire,
            no_revive: case.no_revive,
            req_level_height: case.req_level_height,
            pet_stats_height: case.pet_stats_height,
            desc_height: case.desc_height,
            incline_height: case.incline_height,
            recipe_height: case.recipe_height,
            flat_rate_height: case.flat_rate_height,
            bottom_attr_heights: case.bottom_attr_heights.clone(),
        });
        assert_eq!(layout.pic_h, want.pic_h, "pic_h for {}", case.id);
        assert_eq!(layout.icon_y, want.icon_y, "icon_y for {}", case.id);
        assert_eq!(
            layout.splitter_h, want.splitter_h,
            "splitter_h for {}",
            case.id
        );
    }
}
