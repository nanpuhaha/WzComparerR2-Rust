//! `GetGearPropDiffString22`, `GetExtraJobReqString(int)`,
//! `GetExtraJobReqStringList`을 실제 C# 실행과 비교한다.

use serde::Deserialize;
use wz_avatar::gear_prop_diff_pair;
use wz_db::{extra_job_req_string, extra_job_req_string_list};

#[derive(Deserialize)]
struct Expected {
    diffs: Vec<DiffRow>,
    ids: Vec<IdRow>,
    lists: Vec<ListRow>,
}

#[derive(Deserialize)]
struct DiffRow {
    key: String,
    value: i64,
    standard: i64,
    label: Option<String>,
    value_text: Option<String>,
    suffix: Option<String>,
    pair_null: bool,
}

#[derive(Deserialize)]
struct IdRow {
    id: i32,
    text: Option<String>,
}

#[derive(Deserialize)]
struct ListRow {
    jobs: Vec<i32>,
    normal: Option<Vec<String>>,
    msn: Option<Vec<String>>,
}

#[test]
fn diff22_and_job_requirements_match_csharp() {
    let expected: Expected =
        serde_json::from_str(include_str!("fixtures/diff22_jobreq_expected.json")).unwrap();
    assert_eq!(expected.diffs.len(), 45);
    assert_eq!(expected.ids.len(), 35);
    assert_eq!(expected.lists.len(), 12);

    for want in &expected.diffs {
        let got = gear_prop_diff_pair(&want.key, want.value, want.standard);
        assert_eq!(got.is_none(), want.pair_null, "{}", want.key);
        if let Some((label, value_text, suffix)) = got {
            assert_eq!(label, want.label, "label {} {}", want.key, want.value);
            assert_eq!(
                value_text, want.value_text,
                "value {} {}",
                want.key, want.value
            );
            assert_eq!(
                suffix, want.suffix,
                "suffix {} {} vs {}",
                want.key, want.value, want.standard
            );
        }
    }

    for want in &expected.ids {
        assert_eq!(
            extra_job_req_string(want.id).map(str::to_string),
            want.text,
            "id {}",
            want.id
        );
    }

    for want in &expected.lists {
        assert_eq!(
            extra_job_req_string_list(&want.jobs, false),
            want.normal,
            "normal {:?}",
            want.jobs
        );
        assert_eq!(
            extra_job_req_string_list(&want.jobs, true),
            want.msn,
            "msn {:?}",
            want.jobs
        );
    }
}
