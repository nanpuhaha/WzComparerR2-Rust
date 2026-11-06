//! `GetExtraJobReqString(GearType, ...)` / `GetAstraExtraJobReqString`을 실제 C#과 비교한다.
//!
//! GearType 161개 이름에 대해 4가지 시나리오(아스트라 없음, id=0 표, id=7 표,
//! hasReqSpecJobs=true)를 실행한다.

use serde::Deserialize;
use std::collections::BTreeMap;
use wz_avatar::{extra_job_req_string_for_gear, AstraSubWeaponInfo};

#[derive(Deserialize)]
struct Expected {
    name: String,
    none: Option<String>,
    astra0: Option<String>,
    astra7: Option<String>,
    astra0_has: Option<String>,
}

fn table0() -> BTreeMap<i32, AstraSubWeaponInfo> {
    let mut map = BTreeMap::new();
    map.insert(
        0,
        AstraSubWeaponInfo {
            id: 0,
            index: 0,
            job: 114,
        },
    );
    map
}

fn table7() -> BTreeMap<i32, AstraSubWeaponInfo> {
    let mut map = BTreeMap::new();
    map.insert(
        7,
        AstraSubWeaponInfo {
            id: 7,
            index: 0,
            job: 112,
        },
    );
    map
}

#[test]
fn extra_job_req_for_gear_matches_csharp() {
    let expected: Vec<Expected> =
        serde_json::from_str(include_str!("fixtures/astra_job_expected.json")).unwrap();
    assert_eq!(expected.len(), 161);
    let t0 = table0();
    let t7 = table7();
    for want in expected {
        let name = &want.name;
        assert_eq!(
            extra_job_req_string_for_gear(name, false, None, 0),
            want.none,
            "none {name}"
        );
        assert_eq!(
            extra_job_req_string_for_gear(name, false, Some(&t0), 0),
            want.astra0,
            "astra0 {name}"
        );
        assert_eq!(
            extra_job_req_string_for_gear(name, false, Some(&t7), 7),
            want.astra7,
            "astra7 {name}"
        );
        assert_eq!(
            extra_job_req_string_for_gear(name, true, Some(&t0), 0),
            want.astra0_has,
            "astra0_has {name}"
        );
    }
}
