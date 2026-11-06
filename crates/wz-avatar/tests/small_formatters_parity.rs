//! `GetMobCategoryName`/`GetAttackSpeedString`/`GetGearGradeString`/
//! `GetGearPropDiffString`을 실제 C# 실행과 비교한다.

use serde::Deserialize;
use wz_avatar::gear_prop_diff_string;
use wz_db::{get_attack_speed_string, get_gear_grade_string, get_mob_category_name};

#[derive(Deserialize)]
struct Expected {
    categories: Vec<Named>,
    speeds: Vec<Named>,
    grades: Vec<NamedText>,
    diffs: Vec<DiffRow>,
}

#[derive(Deserialize)]
struct Named {
    id: i32,
    name: Option<String>,
}

#[derive(Deserialize)]
struct NamedText {
    name: String,
    text: Option<String>,
}

#[derive(Deserialize)]
struct DiffRow {
    key: String,
    value: i64,
    standard: i64,
    text: Option<String>,
}

#[test]
fn small_formatters_match_csharp() {
    let expected: Expected =
        serde_json::from_str(include_str!("fixtures/small_formatters_expected.json")).unwrap();
    assert_eq!(expected.categories.len(), 12);
    assert_eq!(expected.speeds.len(), 13);
    assert_eq!(expected.grades.len(), 7);
    assert_eq!(expected.diffs.len(), 45);

    for want in &expected.categories {
        assert_eq!(
            get_mob_category_name(want.id).map(str::to_string),
            want.name,
            "mob category {}",
            want.id
        );
    }
    for want in &expected.speeds {
        assert_eq!(
            get_attack_speed_string(want.id),
            want.name.clone().unwrap_or_default(),
            "attack speed {}",
            want.id
        );
    }
    for want in &expected.grades {
        assert_eq!(
            get_gear_grade_string(&want.name).map(str::to_string),
            want.text,
            "grade {}",
            want.name
        );
    }
    for want in &expected.diffs {
        assert_eq!(
            gear_prop_diff_string(&want.key, want.value, want.standard),
            want.text,
            "diff {} {} vs {}",
            want.key,
            want.value,
            want.standard
        );
    }
}
