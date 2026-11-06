//! `ToThousandsNumberExpr` 두 모드와 `GetMobSkillName`을 실제 C# 실행과 비교한다.

use serde::Deserialize;
use wz_db::{get_mob_skill_name, to_thousands_number_expr};

#[derive(Deserialize)]
struct ThousandsExpected {
    value: i64,
    kms: String,
    msea: String,
}

#[derive(Deserialize)]
struct MobSkillExpected {
    id: i32,
    name: Option<String>,
}

#[test]
fn thousands_number_matches_csharp() {
    let expected: Vec<ThousandsExpected> =
        serde_json::from_str(include_str!("fixtures/thousands_expected.json")).unwrap();
    assert!(expected.len() >= 140);
    for want in expected {
        assert_eq!(
            to_thousands_number_expr(want.value, false),
            want.kms,
            "kms {}",
            want.value
        );
        assert_eq!(
            to_thousands_number_expr(want.value, true),
            want.msea,
            "msea {}",
            want.value
        );
    }
}

#[test]
fn mob_skill_names_match_csharp() {
    let expected: Vec<MobSkillExpected> =
        serde_json::from_str(include_str!("fixtures/mob_skill_expected.json")).unwrap();
    assert_eq!(expected.len(), 63);
    for want in expected {
        assert_eq!(
            get_mob_skill_name(want.id).map(str::to_string),
            want.name,
            "id {}",
            want.id
        );
    }
}
