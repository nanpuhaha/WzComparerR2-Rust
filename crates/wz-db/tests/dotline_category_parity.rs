//! `RenderHelper.DrawV6SkillDotline` 기하와 `GetItemCategoryName`을 C# 참조와 비교.
//!
//! 점선은 GDI+ `TextureBrush` 타일링을 옮긴 전사 구현과 비교하고,
//! 카테고리 이름은 실제 `ItemStringHelper.GetItemCategoryName` 실행 결과와 비교한다.

use serde::Deserialize;
use wz_db::draw_math::skill_dotline_tiles;
use wz_db::item_category_name;

#[derive(Deserialize)]
struct DotCase {
    x1: i32,
    x2: i32,
    y: i32,
    dot_width: i32,
    dot_height: i32,
}

#[derive(Deserialize)]
struct DotExpected {
    tiles: Vec<Tile>,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
struct Tile {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

#[derive(Deserialize)]
struct CatExpected {
    name: String,
    text: Option<String>,
}

#[test]
fn dotline_tiles_match_csharp() {
    let cases: Vec<DotCase> =
        serde_json::from_str(include_str!("fixtures/dotline_cases.json")).unwrap();
    let expected: Vec<DotExpected> =
        serde_json::from_str(include_str!("fixtures/dotline_expected.json")).unwrap();
    assert_eq!(cases.len(), expected.len());
    for (case, want) in cases.iter().zip(expected.iter()) {
        let got: Vec<Tile> =
            skill_dotline_tiles(case.x1, case.x2, case.y, case.dot_width, case.dot_height)
                .into_iter()
                .map(|(x, y, width, height)| Tile {
                    x,
                    y,
                    width,
                    height,
                })
                .collect();
        assert_eq!(
            got, want.tiles,
            "x1 {} x2 {} dot {}x{}",
            case.x1, case.x2, case.dot_width, case.dot_height
        );
    }
}

#[test]
fn item_category_names_match_csharp() {
    let expected: Vec<CatExpected> =
        serde_json::from_str(include_str!("fixtures/item_category_expected.json")).unwrap();
    for want in expected {
        assert_eq!(
            item_category_name(&want.name).map(str::to_string),
            want.text,
            "{}",
            want.name
        );
    }
}
