//! `ItemStringHelper.GetSkillReqAmount`을 실제 C# 실행과 비교한다.

use serde::Deserialize;
use wz_db::skill_req_amount;

#[derive(Deserialize)]
struct Expected {
    skill_id: i32,
    amount_7: String,
    amount_1: String,
}

#[test]
fn skill_req_amount_matches_csharp() {
    let expected: Vec<Expected> =
        serde_json::from_str(include_str!("fixtures/skill_req_expected.json")).unwrap();
    assert_eq!(expected.len(), 13);
    for want in expected {
        assert_eq!(
            skill_req_amount(want.skill_id, 7),
            want.amount_7,
            "id {} amount 7",
            want.skill_id
        );
        assert_eq!(
            skill_req_amount(want.skill_id, 1),
            want.amount_1,
            "id {} amount 1",
            want.skill_id
        );
    }
}
