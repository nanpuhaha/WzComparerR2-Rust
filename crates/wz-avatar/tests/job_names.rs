//! C# `ItemStringHelper.GetJobName` 실행 결과와 비교한다.

use serde::Deserialize;

#[derive(Deserialize, PartialEq, Debug)]
struct Row {
    code: i32,
    name: String,
}

#[test]
fn job_names_match_csharp() {
    let expected: Vec<Row> = serde_json::from_str(include_str!("job_names.json")).unwrap();
    let mut got = Vec::new();
    for code in 0..=60000 {
        if let Some(name) = wz_avatar::get_job_name(code) {
            got.push(Row {
                code,
                name: name.to_string(),
            });
        }
    }
    assert_eq!(got, expected);
}
