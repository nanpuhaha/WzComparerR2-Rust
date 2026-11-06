//! `GetExtraJobReqString(GearType, bool, astra, id)` / `GetAstraExtraJobReqString`.

use std::collections::BTreeMap;

/// 원본 `AstraSubWeaponInfo` (readonly struct).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AstraSubWeaponInfo {
    pub id: i32,
    pub index: i32,
    /// `GetJobName`에 쓰는 직업 코드.
    pub job: i32,
}

/// `Regex.Replace(jobName, @"\s*\(\d{1,2}차\)$", "")` — 끝의 `(N차)` 제거.
pub fn strip_grade_suffix(name: &str) -> String {
    let trimmed = name.trim_end();
    let Some(open) = trimmed.rfind('(') else {
        return name.to_string();
    };
    let tail = &trimmed[open + 1..];
    let Some(body) = tail.strip_suffix(')') else {
        return name.to_string();
    };
    let Some(digits) = body.strip_suffix('차') else {
        return name.to_string();
    };
    if !digits.is_empty() && digits.len() <= 2 && digits.bytes().all(|b| b.is_ascii_digit()) {
        trimmed[..open].trim_end().to_string()
    } else {
        name.to_string()
    }
}

/// `GetAstraExtraJobReqString` — 아스트라 보조무기 표에서 직업명을 찾는다.
pub fn get_astra_extra_job_req_string(
    table: &BTreeMap<i32, AstraSubWeaponInfo>,
    id: i32,
) -> Option<String> {
    let value = table.get(&id)?;
    let name = crate::job_name::get_job_name(value.job).unwrap_or("");
    Some(format!("{} 착용 가능", strip_grade_suffix(name)))
}

/// `GetExtraJobReqString(GearType type, hasReqSpecJobs, loadedAstraSubWeapons, id)`.
pub fn extra_job_req_string_for_gear(
    gear_type: &str,
    has_req_spec_jobs: bool,
    astra: Option<&BTreeMap<i32, AstraSubWeaponInfo>>,
    id: i32,
) -> Option<String> {
    let astra_string = if !has_req_spec_jobs {
        astra.and_then(|table| get_astra_extra_job_req_string(table, id))
    } else {
        None
    };
    match gear_type {
        "shield" => astra_string,
        "cygnusGem" => {
            Some(astra_string.unwrap_or_else(|| "시그너스 기사단 착용 가능".to_string()))
        }
        "heroMedal" => Some("히어로 직업군 착용 가능".to_string()),
        "rosario" => Some("팔라딘 직업군 착용 가능".to_string()),
        "chain" => Some("다크나이트 직업군 착용 가능".to_string()),
        "book1" => Some("불,독 계열 마법사 착용 가능".to_string()),
        "book2" => Some("얼음,번개 계열 마법사 착용 가능".to_string()),
        "book3" => Some("비숍 계열 마법사 착용 가능".to_string()),
        "bowMasterFeather" => Some("보우마스터 직업군 착용 가능".to_string()),
        "crossBowThimble" => Some("신궁 직업군 착용 가능".to_string()),
        "shadowerSheath" => Some("섀도어 직업군 착용 가능".to_string()),
        "nightLordPoutch" => Some("나이트로드 직업군 착용 가능".to_string()),
        "katara" => Some("듀얼블레이드 직업군 착용 가능".to_string()),
        "viperWristband" => Some("바이퍼 직업군 착용 가능".to_string()),
        "captainSight" => Some("캡틴 직업군 착용 가능".to_string()),
        "connonGunPowder" | "connonGunPowder2" => Some("캐논 슈터 직업군 착용 가능".to_string()),
        "box" | "boxingClaw" => Some("용의 전인 착용 가능".to_string()),
        "relic" => Some("패스파인더 직업군 착용 가능".to_string()),
        "aranPendulum" => wz_db::extra_job_req_string(21).map(|s| s.to_string()),
        "dragonMask" | "dragonPendant" | "dragonWings" | "dragonTail" | "evanPaper" => {
            wz_db::extra_job_req_string(22).map(|s| s.to_string())
        }
        "magicArrow" => wz_db::extra_job_req_string(23).map(|s| s.to_string()),
        "card" => wz_db::extra_job_req_string(24).map(|s| s.to_string()),
        "foxPearl" => wz_db::extra_job_req_string(25).map(|s| s.to_string()),
        "orb" | "shiningRod" => wz_db::extra_job_req_string(27).map(|s| s.to_string()),
        "demonShield" => wz_db::extra_job_req_string(31).map(|s| s.to_string()),
        "desperado" => Some("데몬 어벤져 착용 가능".to_string()),
        "battlemageBall" => Some("배틀메이지 착용 가능".to_string()),
        "wildHunterArrowHead" => Some("와일드헌터 착용 가능".to_string()),
        "machineEngine" | "machineArms" | "machineLegs" | "machineBody" | "machineTransistors"
        | "mailin" => Some("메카닉 착용 가능".to_string()),
        "controller" | "powerSource" | "energySword" => {
            wz_db::extra_job_req_string(36).map(|s| s.to_string())
        }
        "GauntletBuster" | "ExplosivePill" => {
            wz_db::extra_job_req_string(37).map(|s| s.to_string())
        }
        "katana" | "kodachi" | "kodachi2" => Some("하야토 착용 가능".to_string()),
        "onmyouSen" | "kannaReifu" | "fan" => Some("칸나 착용 가능".to_string()),
        "soulShield" => Some("미하일 착용 가능".to_string()),
        "novaMarrow" => wz_db::extra_job_req_string(61).map(|s| s.to_string()),
        "weaponBelt" | "breathShooter" => wz_db::extra_job_req_string(63).map(|s| s.to_string()),
        "chain2" | "transmitter" => wz_db::extra_job_req_string(64).map(|s| s.to_string()),
        "soulBangle" | "soulShooter" => wz_db::extra_job_req_string(65).map(|s| s.to_string()),
        "swordZB" | "swordZL" | "hourGlass" => {
            wz_db::extra_job_req_string(101).map(|s| s.to_string())
        }
        "whistle" | "whistle2" | "memorialStaff" => {
            wz_db::extra_job_req_string(172).map(|s| s.to_string())
        }
        "magicStick" => wz_db::extra_job_req_string(112).map(|s| s.to_string()),
        "espLimiter" | "chess" => wz_db::extra_job_req_string(142).map(|s| s.to_string()),
        "magicGauntlet" | "magicWing" => wz_db::extra_job_req_string(152).map(|s| s.to_string()),
        "pathOfAbyss" => wz_db::extra_job_req_string(155).map(|s| s.to_string()),
        "handFan" | "fanTassel" => wz_db::extra_job_req_string(164).map(|s| s.to_string()),
        "tuner" | "bracelet" => wz_db::extra_job_req_string(151).map(|s| s.to_string()),
        "boxingCannon" | "boxingSky" => wz_db::extra_job_req_string(175).map(|s| s.to_string()),
        "ornament" => wz_db::extra_job_req_string(162).map(|s| s.to_string()),
        "chakram" | "hexSeeker" => wz_db::extra_job_req_string(154).map(|s| s.to_string()),
        "longSword" | "sacredJewel" => wz_db::extra_job_req_string(161).map(|s| s.to_string()),
        "gram" | "keir" => wz_db::extra_job_req_string(181).map(|s| s.to_string()),
        "celestialLight" | "compass" => wz_db::extra_job_req_string(182).map(|s| s.to_string()),
        "carta" | "magicQuill" => wz_db::extra_job_req_string(192).map(|s| s.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(job: i32) -> BTreeMap<i32, AstraSubWeaponInfo> {
        let mut map = BTreeMap::new();
        map.insert(
            3,
            AstraSubWeaponInfo {
                id: 3,
                index: 0,
                job,
            },
        );
        map
    }

    #[test]
    fn strips_grade_suffix() {
        assert_eq!(strip_grade_suffix("히어로(6차)"), "히어로");
        assert_eq!(strip_grade_suffix("히어로"), "히어로");
        assert_eq!(strip_grade_suffix("제로(10차)"), "제로");
        assert_eq!(strip_grade_suffix("이상한(가)"), "이상한(가)");
    }

    #[test]
    fn astra_and_gear_labels() {
        let map = table(114);
        assert_eq!(
            get_astra_extra_job_req_string(&map, 3),
            Some("히어로 착용 가능".to_string())
        );
        assert_eq!(get_astra_extra_job_req_string(&map, 9), None);
        assert_eq!(
            extra_job_req_string_for_gear("shield", false, Some(&map), 3),
            Some("히어로 착용 가능".to_string())
        );
        assert_eq!(
            extra_job_req_string_for_gear("shield", false, None, 3),
            None
        );
        assert_eq!(
            extra_job_req_string_for_gear("cygnusGem", false, None, 3),
            Some("시그너스 기사단 착용 가능".to_string())
        );
        assert_eq!(
            extra_job_req_string_for_gear("desperado", false, None, 0),
            Some("데몬 어벤져 착용 가능".to_string())
        );
        assert_eq!(extra_job_req_string_for_gear("cap", false, None, 0), None);
    }
}
