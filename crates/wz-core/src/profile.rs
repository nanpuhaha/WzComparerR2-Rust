//! 프로파일 레지스트리 — `(컨테이너 형식, 클라이언트 버전)` → 알고리즘 조합.
//!
//! 원본: `WzComparerR2.WzLib/Compatibility/WzVersionProfile.cs:171-184`.
//!
//! # 왜 레지스트리가 필요한가
//!
//! WZ 포맷은 **한 축으로 버전이 안 결정된다.** 같은 `PKG2` 라도 KMST1196–1201 은
//! 32비트 해시 + 서로 다른 오프셋 산식, 1202–1204 는 64비트 해시, 1205/1206 은
//! 64비트 해시 + `Pkg2Kmst1205Hash` 이다. 여기에 문자열 키 타입과 이름 위치까지
//! 또 갈린다. 그래서 "버전" 하나가 아니라 **프로파일 하나**를 식별해야 한다.
//!
//! # 순서가 비단조다 — 정렬하면 안 된다
//!
//! 원본 `allProfiles` 배열 순서 그대로 시험한다. 특히:
//!
//! - `Pkg2Kmst1198` 포맷: **1200 → 1199 → 1198** (오름차순이 아님!)
//! - `Pkg2Kmst1196` 포맷: **1197 → 1196**
//!
//! 오름차순으로 정렬하면 1200 파일에서 1198 을 먼저 시도해 잘못된 키로
//! 트리를 읽고, "엔트리 개수가 이상하다" 는 이유로 **정답인 1200 을 건너뛴다**.
//!
//! # 판별의 결정적 단서는 오프셋 검증이다
//!
//! `hashVersion` 을 하나 고르면 모든 오프셋이 정해진다. 틀린 키는 오프셋을
//! 무작위에 가깝게 흩뿌리므로 거의 항상 (a) 파일 범위를 벗어나거나,
//! (b) 서로 겹친다. 모듈 [`detect_version`] 의 검증이 이걸 잡는다.

use crate::offset::{
    OffsetCalc, Pkg1OffsetCalc, Pkg2OffsetCalc32, Pkg2OffsetCalc64V1, Pkg2OffsetCalc64V2,
};
use crate::verify;

/// 컨테이너/헤더 형식 (원본 `WzFileFormat`, `WzPreReadProvider.cs:762-772`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WzFileFormat {
    Pkg1,
    Pkg2Kmst1196,
    Pkg2Kmst1198,
    Pkg2Kmst1201,
    Pkg2Kmst1202,
    Pkg2Kmst1204,
    Pkg2Kmst1205,
    Pkg2Kmst1206,
}

impl WzFileFormat {
    /// 헤더 길이로 유추 가능한 형식 (랜덤 헤더). `None` 이면 매직 기반.
    ///
    /// 함정: 150 / 200 / 163 / 353 이 **랜덤 헤더 전체 길이**다. `copyright`
    /// 길이가 아니다 — 랜덤 헤더에는 `copyright` 필드가 아예 없다.
    pub fn from_random_header_len(header_len: u32) -> Option<Self> {
        Some(match header_len {
            68 => Self::Pkg2Kmst1201,
            150 => Self::Pkg2Kmst1202,
            200 => Self::Pkg2Kmst1204,
            163 => Self::Pkg2Kmst1205,
            353 => Self::Pkg2Kmst1206,
            _ => return None,
        })
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pkg1 => "Pkg1",
            Self::Pkg2Kmst1196 => "Pkg2Kmst1196",
            Self::Pkg2Kmst1198 => "Pkg2Kmst1198",
            Self::Pkg2Kmst1201 => "Pkg2Kmst1201",
            Self::Pkg2Kmst1202 => "Pkg2Kmst1202",
            Self::Pkg2Kmst1204 => "Pkg2Kmst1204",
            Self::Pkg2Kmst1205 => "Pkg2Kmst1205",
            Self::Pkg2Kmst1206 => "Pkg2Kmst1206",
        }
    }
}

/// 오프셋 난독화 세대 (원본 `Pkg2OffsetVersion`, `WzOffsetCalc.cs:53-65`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OffsetVersion {
    /// KMST1196–1197
    V1,
    /// KMST1198
    V2,
    /// KMST1199–1201
    V3,
    /// KMST1202–1204
    V4,
    /// KMST1205–1206
    V5,
}

/// PKG2 디렉터리 이름 키 세대 (원본 `Pkg2EntryNameVersion`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryNameVersion {
    V1196,
    V1198,
    V1199,
    V1202,
    V1204,
    V1205,
    V1206,
}

/// PKG2 엔트리 안에서 이름이 놓이는 위치 (원본 `Pkg2EntryNamePosition`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryNamePosition {
    /// `[type][name][size][cs32]`
    BeforeData,
    /// `[type][size][cs32][name]`
    AfterData,
}

/// `hashVersion` 계산기 종류.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashVersionCalc {
    /// PKG1: `encver` 바이트를 되돌려 후보 나열 (별도 경로)
    Pkg1,
    /// KMST1196 — 닫힌 형태
    V1,
    /// KMST1197 — 비트 백트래킹
    V2,
    /// KMST1198 — 비트 백트래킹
    V3,
    /// KMST1199–1201 — 2³² 스윕
    V4,
    /// KMST1200 — 2³² 스윕 (target 만 다름)
    V5,
    /// KMST1202–1204 — 닫힌 형태
    V64V1,
    /// KMST1205 — 하드코딩된 버전 문자열
    V64V2,
    /// KMST1206 — HMAC 기반 하드코딩
    V64V3,
}

/// 하나의 포맷 프로파일.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Profile {
    /// 보고되는 클라이언트 버전.
    ///
    /// 함정: `1200` 은 파일 형식이 `Pkg2Kmst1198` 이다. 버전과 형식이 1:1 이다.
    pub wz_version: u32,
    pub format: WzFileFormat,
    pub offset_version: Option<OffsetVersion>,
    pub entry_name_version: Option<EntryNameVersion>,
    pub hash_calc: HashVersionCalc,
    pub name_position: EntryNamePosition,
}

impl Profile {
    /// 오프셋 계산기를 만든다. `hash_version` 이 틀리면 오프셋이 무작위가 된다.
    pub fn offset_calc(
        &self,
        header_len: u32,
        hash_version: u64,
        hash1: u64,
    ) -> Box<dyn OffsetCalc> {
        match self.format {
            WzFileFormat::Pkg1 => Box::new(Pkg1OffsetCalc::new(header_len, hash_version as u32)),
            WzFileFormat::Pkg2Kmst1196 => Box::new(Pkg2OffsetCalc32::v1(
                header_len,
                hash_version as u32,
                hash1 as u32,
            )),
            WzFileFormat::Pkg2Kmst1198 => match self.offset_version {
                Some(OffsetVersion::V1) => Box::new(Pkg2OffsetCalc32::v1(
                    header_len,
                    hash_version as u32,
                    hash1 as u32,
                )),
                _ => Box::new(Pkg2OffsetCalc32::v2(
                    header_len,
                    hash_version as u32,
                    hash1 as u32,
                )),
            },
            WzFileFormat::Pkg2Kmst1201 => Box::new(Pkg2OffsetCalc32::v3(
                header_len,
                hash_version as u32,
                hash1 as u32,
            )),
            WzFileFormat::Pkg2Kmst1202 | WzFileFormat::Pkg2Kmst1204 => {
                Box::new(Pkg2OffsetCalc64V1::new(header_len, hash1, hash_version))
            }
            WzFileFormat::Pkg2Kmst1205 | WzFileFormat::Pkg2Kmst1206 => {
                Box::new(Pkg2OffsetCalc64V2::new(header_len, hash1, hash_version))
            }
        }
    }

    /// 이 프로파일의 `hashVersion` 후보를 열거한다.
    ///
    /// # 함정: 2³² 스윕은 **매우 느리다**
    ///
    /// KMST1199–1201 은 후보 하나당 43억 번 검증한다. 그래서 여러 후보를
    /// 무작정 시도하면 수십 초 걸린다. 호출자는 순서를 지키고, 가능하면
    /// 밖에서 캐시된 `hash_version` 을 주어야 한다.
    pub fn hash_version_candidates(&self, hash1: u64, hash2: u64) -> Vec<u64> {
        let (h1, h2) = (hash1 as u32, hash2 as u32);
        match self.hash_calc {
            HashVersionCalc::V1 => verify::candidates_1196(h1, h2)
                .into_iter()
                .map(u64::from)
                .collect(),
            HashVersionCalc::V2 => verify::candidates_1197(h1, h2)
                .into_iter()
                .map(u64::from)
                .collect(),
            HashVersionCalc::V3 => verify::candidates_1198(h1, h2)
                .into_iter()
                .map(u64::from)
                .collect(),
            HashVersionCalc::V4 => verify::candidates_1199_1201(h1, h2)
                .into_iter()
                .map(u64::from)
                .collect(),
            HashVersionCalc::V5 => verify::candidates_1200(h1, h2)
                .into_iter()
                .map(u64::from)
                .collect(),
            HashVersionCalc::V64V1 => vec![verify::hash_version_1202_1204(hash1, hash2)],
            HashVersionCalc::V64V2 => verify::candidates_1205(hash1, hash2),
            HashVersionCalc::V64V3 => verify::candidates_1206(hash1, hash2),
            HashVersionCalc::Pkg1 => Vec::new(),
        }
    }
}

/// 원본 `allProfiles` 배열 — **순서를 그대로 보존**한다.
///
/// (원본 `WzVersionProfile.cs:171-184`)
pub const ALL_PROFILES: &[Profile] = &[
    Profile {
        wz_version: 777,
        format: WzFileFormat::Pkg1,
        offset_version: None,
        entry_name_version: None,
        hash_calc: HashVersionCalc::Pkg1,
        name_position: EntryNamePosition::BeforeData,
    },
    Profile {
        wz_version: 1206,
        format: WzFileFormat::Pkg2Kmst1206,
        offset_version: Some(OffsetVersion::V5),
        entry_name_version: Some(EntryNameVersion::V1206),
        hash_calc: HashVersionCalc::V64V3,
        name_position: EntryNamePosition::AfterData,
    },
    Profile {
        wz_version: 1205,
        format: WzFileFormat::Pkg2Kmst1205,
        offset_version: Some(OffsetVersion::V5),
        entry_name_version: Some(EntryNameVersion::V1205),
        hash_calc: HashVersionCalc::V64V2,
        name_position: EntryNamePosition::AfterData,
    },
    Profile {
        wz_version: 1204,
        format: WzFileFormat::Pkg2Kmst1204,
        offset_version: Some(OffsetVersion::V4),
        entry_name_version: Some(EntryNameVersion::V1204),
        hash_calc: HashVersionCalc::V64V1,
        name_position: EntryNamePosition::BeforeData,
    },
    Profile {
        wz_version: 1202,
        format: WzFileFormat::Pkg2Kmst1202,
        offset_version: Some(OffsetVersion::V4),
        entry_name_version: Some(EntryNameVersion::V1202),
        hash_calc: HashVersionCalc::V64V1,
        name_position: EntryNamePosition::BeforeData,
    },
    Profile {
        wz_version: 1201,
        format: WzFileFormat::Pkg2Kmst1201,
        offset_version: Some(OffsetVersion::V3),
        entry_name_version: Some(EntryNameVersion::V1199),
        hash_calc: HashVersionCalc::V4,
        name_position: EntryNamePosition::BeforeData,
    },
    // 함정: 1200 은 파일 형식이 Pkg2Kmst1198 이다.
    Profile {
        wz_version: 1200,
        format: WzFileFormat::Pkg2Kmst1198,
        offset_version: Some(OffsetVersion::V3),
        entry_name_version: Some(EntryNameVersion::V1199),
        hash_calc: HashVersionCalc::V5,
        name_position: EntryNamePosition::BeforeData,
    },
    Profile {
        wz_version: 1199,
        format: WzFileFormat::Pkg2Kmst1198,
        offset_version: Some(OffsetVersion::V3),
        entry_name_version: Some(EntryNameVersion::V1199),
        hash_calc: HashVersionCalc::V4,
        name_position: EntryNamePosition::BeforeData,
    },
    Profile {
        wz_version: 1198,
        format: WzFileFormat::Pkg2Kmst1198,
        offset_version: Some(OffsetVersion::V2),
        entry_name_version: Some(EntryNameVersion::V1198),
        hash_calc: HashVersionCalc::V3,
        name_position: EntryNamePosition::BeforeData,
    },
    // 함정: 1196 포맷에서 1197 이 1196 보다 먼저 시도된다.
    Profile {
        wz_version: 1197,
        format: WzFileFormat::Pkg2Kmst1196,
        offset_version: Some(OffsetVersion::V1),
        entry_name_version: Some(EntryNameVersion::V1196),
        hash_calc: HashVersionCalc::V2,
        name_position: EntryNamePosition::BeforeData,
    },
    Profile {
        wz_version: 1196,
        format: WzFileFormat::Pkg2Kmst1196,
        offset_version: Some(OffsetVersion::V1),
        entry_name_version: Some(EntryNameVersion::V1196),
        hash_calc: HashVersionCalc::V1,
        name_position: EntryNamePosition::BeforeData,
    },
];

/// 감지된 헤더에서 가능한 프로파일 후보를 **레지스트리 순서**로 돌려준다.
///
/// # 함정: 감지가 이미 한 단계 걸러준다
///
/// `Pkg2Kmst1201` 과 `Pkg2Kmst1198` 은 둘 다 `Pkg2RandomHeader` capability 를
/// 갖지 않으므로 헤더만으로 구별되지 않는다. 그래서 둘 다 후보로 남고
/// 뒤의 오프셋 검증이 결정한다.
pub fn candidates_for(format: WzFileFormat) -> impl Iterator<Item = &'static Profile> {
    ALL_PROFILES.iter().filter(move |p| p.format == format)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_matches_csharp_order() {
        // 원본 `WzVersionProfile.cs:171-184` 과 1:1 이어야 한다.
        let got: Vec<u32> = ALL_PROFILES.iter().map(|p| p.wz_version).collect();
        assert_eq!(
            got,
            vec![777, 1206, 1205, 1204, 1202, 1201, 1200, 1199, 1198, 1197, 1196],
            "레지스트리 순서"
        );
    }

    #[test]
    fn pkg2_1198_format_order_is_non_monotonic() {
        // 함정: 오름차순으로 정렬하면 1200 을 건너뛴다.
        let vs: Vec<u32> = candidates_for(WzFileFormat::Pkg2Kmst1198)
            .map(|p| p.wz_version)
            .collect();
        assert_eq!(vs, vec![1200, 1199, 1198], "1200 이 먼저다");
    }

    #[test]
    fn pkg2_1196_format_order_is_non_monotonic() {
        let vs: Vec<u32> = candidates_for(WzFileFormat::Pkg2Kmst1196)
            .map(|p| p.wz_version)
            .collect();
        assert_eq!(vs, vec![1197, 1196], "1197 이 먼저다");
    }

    #[test]
    fn random_header_lengths_map_to_formats() {
        assert_eq!(
            WzFileFormat::from_random_header_len(68),
            Some(WzFileFormat::Pkg2Kmst1201)
        );
        assert_eq!(
            WzFileFormat::from_random_header_len(150),
            Some(WzFileFormat::Pkg2Kmst1202)
        );
        assert_eq!(
            WzFileFormat::from_random_header_len(200),
            Some(WzFileFormat::Pkg2Kmst1204)
        );
        assert_eq!(
            WzFileFormat::from_random_header_len(163),
            Some(WzFileFormat::Pkg2Kmst1205)
        );
        assert_eq!(
            WzFileFormat::from_random_header_len(353),
            Some(WzFileFormat::Pkg2Kmst1206)
        );
        assert_eq!(WzFileFormat::from_random_header_len(60), None);
    }

    #[test]
    fn every_profile_has_candidates_or_is_pkg1() {
        // 함정: 2³² 전수 스윕(1199–1201, 1200)은 **프로필 하나당 ~18초**다
        // (12코어 기준). 기본 테스트 스위트에 넣으면 검증이 느려져서
        // 사람이 스킵하게 되고, 그러면 아예 안 돌게 된다. 그래서 여기서는
        // **닫힌 형태와 백트래킹만** 확인하고 스윕은 `sweep_profiles_are_callable`
        // (#[ignore]) 로 뺀다.
        for p in ALL_PROFILES {
            if p.hash_calc == HashVersionCalc::Pkg1 {
                continue;
            }
            // 2³² 스윕 두 개는 여기서 건너뛴다.
            if matches!(p.hash_calc, HashVersionCalc::V4 | HashVersionCalc::V5) {
                continue;
            }
            let c = p.hash_version_candidates(0x1122_3344, 0x5566_7788);
            match p.hash_calc {
                // 닫힌 형태 — 탐색이 없으므로 항상 정확히 1개
                HashVersionCalc::V1 | HashVersionCalc::V64V1 => {
                    assert_eq!(c.len(), 1, "{} 는 후보가 정확히 1개", p.wz_version)
                }
                // 함정: 1205/1206 은 **0 개일 수도 있다.**
                //
                // 버전 문자열이 하드코딩돼 있어 `hash2` 가 그 값으로 계산한
                // 결과와 다르면 후보를 아예 못 낸다. "항상 1개" 라고 단정하면
                // 정상적인 검증 실패를 오류로 취급하게 된다.
                HashVersionCalc::V64V2 | HashVersionCalc::V64V3 => {
                    assert!(c.len() <= 1, "{} 후보 과다: {}", p.wz_version, c.len());
                    for hv in &c {
                        let ok = if p.hash_calc == HashVersionCalc::V64V2 {
                            verify::verify_1205(0x1122_3344, 0x5566_7788, *hv)
                        } else {
                            verify::verify_1206(0x1122_3344, 0x5566_7788, *hv)
                        };
                        assert!(
                            ok,
                            "{} 가 낸 후보는 자기 검증식을 통과해야 한다",
                            p.wz_version
                        );
                    }
                }
                // 백트래킹 — 0 개일 수 있다 (해당 키가 아니면)
                _ => assert!(c.len() <= 4, "{} 후보 과다: {}", p.wz_version, c.len()),
            }
        }
    }

    /// 2³² 스윕이 **실제로 동작하고** 얼마나 걸리는지 확인한다.
    ///
    /// `cargo test -p wz-core -- --ignored` 로 명시적으로 실행한다.
    /// 기본 스위트에 넣지 않는 이유는 위 테스트에 적었다.
    #[test]
    #[ignore = "프로필당 ~18초 (2^32 전수 스윕) — 성능 확인용"]
    fn sweep_profiles_are_callable() {
        use std::time::Instant;
        for v in [1201u32, 1200] {
            let p = ALL_PROFILES
                .iter()
                .find(|p| p.wz_version == v)
                .expect("프로파일");
            let t = Instant::now();
            let c = p.hash_version_candidates(0x1122_3344, 0x5566_7788);
            eprintln!("KMST{v}: {:?} -> {} 후보", t.elapsed(), c.len());
        }
    }

    #[test]
    fn hardcoded_versions_reject_mismatched_hash2() {
        // 함정: 1205/1206 은 임의의 hash2 로는 후보가 0 개다. 이게 **정상**이다.
        let p = ALL_PROFILES
            .iter()
            .find(|p| p.wz_version == 1205)
            .expect("1205 프로파일");
        assert!(p
            .hash_version_candidates(0x1122_3344, 0x5566_7788)
            .is_empty());
    }

    #[test]
    fn name_position_only_for_1205_1206() {
        for p in ALL_PROFILES {
            match p.format {
                WzFileFormat::Pkg2Kmst1205 | WzFileFormat::Pkg2Kmst1206 => {
                    assert_eq!(p.name_position, EntryNamePosition::AfterData)
                }
                _ => assert_eq!(p.name_position, EntryNamePosition::BeforeData),
            }
        }
    }
}
