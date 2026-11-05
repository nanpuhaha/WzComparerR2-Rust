//! C# `ItemStringHelper.GetFifthJobName` 실행 결과와 비교한다.

use serde::Deserialize;

#[derive(Deserialize)]
struct Row {
    skill: i32,
    job_id: Vec<i32>,
    text: Option<String>,
}

#[test]
fn fifth_job_names_match_csharp() {
    let rows: Vec<Row> = serde_json::from_str(include_str!("fifth_job_names.json")).unwrap();
    for row in rows {
        let got = wz_avatar::get_fifth_job_name(row.skill, &row.job_id);
        assert_eq!(got, row.text, "skill={} job_id={:?}", row.skill, row.job_id);
    }
}
