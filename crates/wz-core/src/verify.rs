//! `hashVersion` 역산 — 헤더 해시에서 클라이언트 버전을 되찾는다.
//!
//! 원본: `WzComparerR2.WzLib/Compatibility/WzVersionVerifier.cs`,
//! `WzVersionHasher.cs`.
//!
//! # 왜 이게 필요한가
//!
//! WZ 헤더의 `hash1`/`hash2` 는 버전 **문자열**에서 파생된 값이다. 파일에는
//! 버전이 직접 적혀 있지 않고, `hashVersion` 만 있으면 오프셋 복호화 키가
//! 정해진다 (원본 `WzOffsetCalc` 들이 `hashVersion` 를 첫 인자로 받는다).
//! 즉 `hashVersion` 를 못 구하면 **이미지 하나도 못 연다**.
//!
//! # 알고리즘은 버전마다 다르다
//!
//! | 버전 | 방식 | 비용 |
//! |---|---|---|
//! | PKG1 | `encver` 바이트를 비트 역순 XOR 로 되돌려 후보 나열 | O(32767) |
//! | KMST1196 | 닫힌 형태 `ROL(hash1,7) ^ hash2` | O(1) |
//! | KMST1197 | 비트 벡터 백트래킹 | O(2^5) |
//! | KMST1198 | 비트 벡터 백트래킹 | O(2^4) |
//! | KMST1199–1201 | 2³² 전수 스윕 | O(2³²) |
//! | KMST1202–1204 | 닫힌 형태 `hash1 ^ hash2 ^ Magic` | O(1) |
//! | KMST1205 | 버전 문자열이 하드코딩됨 | O(1) |
//! | KMST1206 | HMAC-SHA256 기반, 버전 문자열 하드코딩 | O(1) |
//!
//! # 반드시 wrapping 연산
//!
//! 원본 C# 은 전부 `unchecked` 다. 넘침을 panic 으로 두면 어떤 파일을 열었는지에
//! 따라 프로그램이 죽는다 (리포트 §14.4).

use crate::crypto::pkg2_key::mix32;
use crate::offset::rol32;

/// `Pkg2BacktrackSolver.Magic` (`WzVersionVerifier.cs:501`).
pub const BACKTRACK_MAGIC: u32 = 0x1A2B_3C4D;

/// KMST1205/1206 이 하드코딩한 버전 문자열 (`WzVersionVerifier.cs:457,478`).
///
/// 두 계산기 모두 이 문자열을 쓴다. 1205 는 1202 해시를, 1206 은 HMAC 해시를
/// 씁니다 — **같은 문자열, 다른 해시 함수**.
pub const KMST1205_1206_VERSION_STRING: &str = "v410_260106_1_A1F3C9E2";

/// KMST1202–1204 닫힌 형태 상수 (`WzVersionVerifier.cs:437`).
const MAGIC_64_V1: u64 = 0xABCD_EF01_2345_6789;

// ─────────────────────────────────────────────────────────────────────────────
// PKG1
// ─────────────────────────────────────────────────────────────────────────────

/// PKG1 `hashVersion` 계산 (`WzVersionHasher.ComputePkg1HashVersion`).
///
/// 버전을 십진 문자열로 바꿔 `sum = sum * 32 + (c + 1)` 을 반복한다.
///
/// ```text
/// 777 → '7'=0x37:  sum = 0<<5 + 0x38      = 0x38
///                 sum = 0x38<<5 + 0x38    = 0x738
///                 sum = 0x738<<5 + 0x38   = 0xE738
/// ```
/// ⇒ `compute_pkg1_hash_version(777) == 0x0000_E738`.
pub fn compute_pkg1_hash_version(wz_version: u32) -> u32 {
    let mut sum: u32 = 0;
    for b in wz_version.to_string().bytes() {
        sum = sum.wrapping_shl(5);
        sum = sum.wrapping_add(u32::from(b) + 1);
    }
    sum
}

/// `hashVersion` → 헤더에 저장되는 `encver` 바이트
/// (`WzVersionVerifier.CalcEncryptedVersion`, `:67-73`).
///
/// 4바이트를 **모두 반전(bits NOT)** 한 뒤 1바이트로 접는다.
pub fn calc_encrypted_version(hash_version: u32) -> u32 {
    0xFF ^ ((hash_version >> 24) & 0xFF)
        ^ ((hash_version >> 16) & 0xFF)
        ^ ((hash_version >> 8) & 0xFF)
        ^ (hash_version & 0xFF)
}

/// PKG1 `encver` 로부터 `hashVersion` 후보를 전부 복원한다.
///
/// # 함정: 항상 유일하지 않다
///
/// `calc_encrypted_version` 은 4바이트를 반전시켜 접기 때문에 서로 다른
/// 버전이 **같은** `encver` 를 만들 수 있다. 그래서 복수 후보가 나오는 게
/// 정상이며, 어느 것이 맞는지 결정하는 것은 오프셋 검증
/// (`ValidateOffsets`) 의 몫이다.
///
/// # 함정: 탐색 순서가 의미있다
///
/// 원본은 `1` 부터 시작해 **오름차순**으로 훑는다 (`:90-103`). 반환 순서를
/// 그대로 보존해야 캐시 히트율이 재현된다.
pub fn pkg1_hash_version_candidates(encver: u32) -> Vec<(u32, u32)> {
    let target = if encver <= 0xFF { encver } else { 0 };
    let mut out = Vec::new();
    for v in 1..i16::MAX as u32 {
        let hv = compute_pkg1_hash_version(v);
        if calc_encrypted_version(hv) == target {
            out.push((v, hv));
        }
    }
    out
}

/// `encver` 가 사라진(KMST1132+) PKG1 의 유일한 고정 후보.
pub const PKG1_ENCVER_MISSING_VERSION: u32 = 777;

/// `encverMissing` 고정 모드의 후보. 항상 1개.
pub fn pkg1_fixed_candidate() -> Vec<(u32, u32)> {
    vec![(
        PKG1_ENCVER_MISSING_VERSION,
        compute_pkg1_hash_version(PKG1_ENCVER_MISSING_VERSION),
    )]
}

// ─────────────────────────────────────────────────────────────────────────────
// KMST1196 (닫힌 형태)
// ─────────────────────────────────────────────────────────────────────────────

/// KMST1196 검증식 (`Pkg2HashVersionCalcV1`, `WzVersionVerifier.cs:149-158`).
///
/// ```text
/// hashVersion = ROL(hash1, 7) ^ hash2
/// ```
///
/// 후보가 **정확히 하나**다. 탐색이 없다.
pub fn verify_1196(hash1: u32, hash2: u32, hash_version: u32) -> bool {
    hash_version == rol32(hash1, 7) ^ hash2
}

/// KMST1196 `hashVersion` 후보. 항상 길이 1.
pub fn candidates_1196(hash1: u32, hash2: u32) -> Vec<u32> {
    vec![rol32(hash1, 7) ^ hash2]
}

// ─────────────────────────────────────────────────────────────────────────────
// 비트 벡터 백트래킹 (KMST1197 / KMST1198)
// ─────────────────────────────────────────────────────────────────────────────

/// 백트래킹 파라미터 (`WzVersionVerifier.BacktrackParameters`).
struct Backtrack<'a> {
    hash1: u32,
    /// 회전 거리. `lowBitLen` 비트는 `low_bits` 로 고정된다.
    s: u32,
    low_bit_len: u32,
    low_bits: u32,
    target: u32,
    /// 덧셈 올림. 33칸 (0..=32).
    carries: [u32; 33],
    /// 각 비트의 LHS 값. 32칸.
    lhs_bits: [u32; 32],
    validator: &'a dyn Fn(u32) -> bool,
    results: Vec<u32>,
}

/// 비트 단위 백트래킹 솔버 (`WzVersionVerifier.Pkg2BacktrackSolver.Solve`).
///
/// # 이게 왜 필요하다
///
/// `Verify` 가 `ROL(hash1 ^ (hashVersion + Magic), hashVersion & 0x1F)` 처럼
/// 비트 위치에 의존하는 식이라 `hashVersion` 를 `2^32` 전수 탐색하면 굼장히
/// 느리다. 그런데 이 식은 **각 비트가 독립적**이라 하위 5비트(또는 4비트)만
/// 가정하면 나머지는 비트별로 결정된다. 하위 비트 후보만 열거하고 나머지를
/// 비트 단위로 채우면 `2^5` / `2^4` 에 끝난다.
///
/// 원본은 Google AI 가 생성한 코드를 사람이 리팩터링한 것이라 변수명이
/// `s` / `vBit` / `lhs` 같은 형태 그대로다. 의미가 불명확한 줄은 주석을
/// 보존했다.
fn solve(p: &mut Backtrack<'_>, bit_idx: u32, v_hash: u32) {
    if bit_idx == 32 {
        if (p.validator)(v_hash) {
            p.results.push(v_hash);
        }
        return;
    }

    // 하위 비트는 `low_bits` 가 정해준 값으로 고정, 그 위는 0/1 둘 다 시도.
    let (start, end) = if bit_idx < p.low_bit_len {
        let v = (p.low_bits >> bit_idx) & 1;
        (v, v)
    } else {
        (0, 1)
    };

    for v_bit in start..=end {
        // ── 뒤쪽 검사: 이미 결정된 LHS 와 모순되면 자른다 ──
        // `hashVersion + Magic` 의 올림이 왼쪽(LSB 방향)으로 전파되므로
        // 이전 비트들의 LHS 가 지금 비트에 영향을 준다.
        let prev_lhs_idx = (bit_idx as i32 - p.s as i32 + 32) & 0x1F;
        if prev_lhs_idx < bit_idx as i32 {
            let v_xor_h2 = v_bit ^ ((p.target >> bit_idx) & 1);
            if v_xor_h2 != p.lhs_bits[prev_lhs_idx as usize] {
                continue;
            }
        }

        let sum = v_bit + ((BACKTRACK_MAGIC >> bit_idx) & 1) + p.carries[bit_idx as usize];
        let current_lhs_bit = (sum ^ (p.hash1 >> bit_idx)) & 1;

        // ── 앞쪽 검사: 앞으로 확정될 비트와 모순되면 자른다 ──
        let future_v_idx = (bit_idx as i32 + p.s as i32) & 0x1F;
        if future_v_idx <= bit_idx as i32 {
            // 순환했으므로 이미 결정된 `vHash` 의 그 비트를 쓴다.
            let known_v_bit = (v_hash >> future_v_idx) & 1;
            let target_v_xor_h2 = known_v_bit ^ ((p.target >> future_v_idx) & 1);
            if current_lhs_bit != target_v_xor_h2 {
                continue;
            }
        } else if future_v_idx < p.low_bit_len as i32 {
            let known_v_bit = (p.low_bits >> future_v_idx) & 1;
            let target_v_xor_h2 = known_v_bit ^ ((p.target >> future_v_idx) & 1);
            if current_lhs_bit != target_v_xor_h2 {
                continue;
            }
        }

        p.lhs_bits[bit_idx as usize] = current_lhs_bit;
        p.carries[(bit_idx + 1) as usize] = sum >> 1;
        solve(p, bit_idx + 1, v_hash | (v_bit << bit_idx));
    }
}

/// 공통 백트래킹 드라이버.
fn backtrack(
    hash1: u32,
    low_bit_len: u32,
    s_for_candidate: impl Fn(u32) -> u32,
    target: u32,
    validator: &dyn Fn(u32) -> bool,
) -> Vec<u32> {
    let mut results = Vec::new();
    let count = 1u32 << low_bit_len;
    for s_candidate in 0..count {
        let mut p = Backtrack {
            hash1,
            s: s_for_candidate(s_candidate),
            low_bit_len,
            low_bits: s_candidate,
            target,
            carries: [0; 33],
            lhs_bits: [0; 32],
            validator,
            results: Vec::new(),
        };
        solve(&mut p, 0, 0);
        results.append(&mut p.results);
    }
    results
}

/// KMST1197 검증식 (`Pkg2HashVersionCalcV2.Verify`, `WzVersionVerifier.cs:191-195`).
///
/// ```text
/// lt = ROL(hash1 ^ (hashVersion + 0x1A2B3C4D), hashVersion & 0x1F)
/// (lt ^ hashVersion) == hash2
/// ```
pub fn verify_1197(hash1: u32, hash2: u32, hash_version: u32) -> bool {
    let lt = rol32(
        hash1 ^ hash_version.wrapping_add(BACKTRACK_MAGIC),
        hash_version & 0x1F,
    );
    (lt ^ hash_version) == hash2
}

/// KMST1197 `hashVersion` 후보 (`WzVersionVerifier.cs:165-189`).
pub fn candidates_1197(hash1: u32, hash2: u32) -> Vec<u32> {
    backtrack(hash1, 5, |s| s, hash2, &|v| verify_1197(hash1, hash2, v))
}

/// KMST1198 검증식 (`Pkg2HashVersionCalcV3.Verify`, `WzVersionVerifier.cs:228-232`).
///
/// # 함정: 회전량에 `hash1 & 0xF` 가 더해진다
///
/// KMST1197 과 달리 `hashVersion & 0xF` 만이 아니라 `(hashVersion & 0xF) +
/// (hash1 & 0xF)` 이 회전량이다. `hash1` 가 바뀌면 회전량도 바뀐다.
pub fn verify_1198(hash1: u32, hash2: u32, hash_version: u32) -> bool {
    let lt = rol32(
        hash1 ^ hash_version.wrapping_add(BACKTRACK_MAGIC),
        (hash_version & 0xF).wrapping_add(hash1 & 0xF),
    );
    !(lt ^ hash_version ^ hash1) == hash2
}

/// KMST1198 `hashVersion` 후보 (`WzVersionVerifier.cs:202-226`).
pub fn candidates_1198(hash1: u32, hash2: u32) -> Vec<u32> {
    backtrack(
        hash1,
        4,
        |s| s.wrapping_add(hash1 & 0xF),
        !hash2 ^ hash1,
        &|v| verify_1198(hash1, hash2, v),
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// 2³² 전수 스윕 (KMST1199 / KMST1200 / KMST1201)
// ─────────────────────────────────────────────────────────────────────────────

/// KMST1199–1201 검증식 (`Pkg2HashVersionCalcV4.Verify`, `WzVersionVerifier.cs:401-407`).
///
/// # 함정: `mixedHash` 의 상위 16비트를 버린다
///
/// `(ushort)mixedHash` 캐스트가 있어 상위 16비트가 잘린다. 그러고 나서
/// `hashVersion` 을 더하므로, 상위 16비트를 유지하면 다른 답이 나온다.
pub fn verify_1199_1201(hash1: u32, hash2: u32, hash_version: u32) -> bool {
    let pre_hash = hash1 ^ hash_version;
    let mixed = mix32(pre_hash ^ 0x6D4C_3B2A) ^ 0x91E1_0DA5;
    let lt = rol32(
        hash1
            ^ ((mixed as u16 as u32)
                .wrapping_add(hash_version)
                .wrapping_add(BACKTRACK_MAGIC)),
        ((mixed ^ hash_version) & 0xF).wrapping_add(hash1 & 0xF),
    );
    (lt ^ pre_hash.wrapping_add(mixed)) == !hash2
}

/// 2³² 전수 스윕 하나. KMST1199–1201 과 KMST1200 가 **이 함수만 다르다**.
///
/// # 왜 2³² 를 다 돌는지
///
/// `Verify` 는 결정적이다. "찾으면 중단" 하면 답이 여러 개일 때 그 뒤를 못 본다.
/// 원본도 전수 확인한다 — 256 청크로 나눠 병렬 처리한다
/// (`WzVersionVerifier.cs:256-259`).
///
/// # 비결정성이 없다는 전제
///
/// 청크는 서로 겹치지 않는 연속 구간이라 결과는 실행 순서와 무관하다. 그래서
/// 마지막에 정렬한다 (`par_iter` 수집 순서가 rayon 스케줄에 따라 바뀐다).
fn sweep_2p32(hash1: u32, target: u32) -> Vec<u32> {
    use rayon::prelude::*;

    const CHUNK_SIZE: u32 = 1 << 24;
    let hits: Vec<Vec<u32>> = (0..256u32)
        .into_par_iter()
        .map(|chunk| {
            let start = chunk << 24;
            let mut local = Vec::new();
            for i in 0..CHUNK_SIZE {
                let hv = start.wrapping_add(i);
                if verify_1199_1201_inner(hash1, target, hv) {
                    local.push(hv);
                }
            }
            local
        })
        .collect();

    let mut out: Vec<u32> = hits.into_iter().flatten().collect();
    out.sort_unstable();
    out
}

/// KMST1199–1201 `hashVersion` 후보 — 2³² 전수 탐색.
pub fn candidates_1199_1201(hash1: u32, hash2: u32) -> Vec<u32> {
    sweep_2p32(hash1, !hash2)
}

/// `target` 이 이미 반전된 `~hash2` 를 받는다 (루프 안에서 재계산 안 하려고).
#[inline]
fn verify_1199_1201_inner(hash1: u32, target: u32, hash_version: u32) -> bool {
    let pre_hash = hash1 ^ hash_version;
    let mixed = mix32(pre_hash ^ 0x6D4C_3B2A) ^ 0x91E1_0DA5;
    let lt = rol32(
        hash1
            ^ ((mixed as u16 as u32)
                .wrapping_add(hash_version)
                .wrapping_add(BACKTRACK_MAGIC)),
        ((mixed ^ hash_version) & 0xF).wrapping_add(hash1 & 0xF),
    );
    (lt ^ pre_hash.wrapping_add(mixed)) == target
}

/// KMST1200 검증식 (`Pkg2HashVersionCalcV5.Verify`, `WzVersionVerifier.cs:422-429`).
///
/// KMST1199 와 **전부 같고 `target` 만 `hash2 ^ magicV5` 로 다르다.**
pub const MAGIC_V5: u32 = 0x2A2C_818B;

pub fn verify_1200(hash1: u32, hash2: u32, hash_version: u32) -> bool {
    let pre_hash = hash1 ^ hash_version;
    let mixed = mix32(pre_hash ^ 0x6D4C_3B2A) ^ 0x91E1_0DA5;
    let lt = rol32(
        hash1
            ^ ((mixed as u16 as u32)
                .wrapping_add(hash_version)
                .wrapping_add(BACKTRACK_MAGIC)),
        ((mixed ^ hash_version) & 0xF).wrapping_add(hash1 & 0xF),
    );
    (lt ^ pre_hash.wrapping_add(mixed) ^ MAGIC_V5) == hash2
}

/// KMST1200 `hashVersion` 후보. KMST1199 와 동일한 2³² 스윕, `target` 만 다름.
pub fn candidates_1200(hash1: u32, hash2: u32) -> Vec<u32> {
    sweep_2p32(hash1, hash2 ^ MAGIC_V5)
}

// ─────────────────────────────────────────────────────────────────────────────
// 64비트 닫힌 형태 (KMST1202–1204)
// ─────────────────────────────────────────────────────────────────────────────

/// KMST1202–1204 `hashVersion` — 닫힌 형태, 탐색 없음
/// (`Pkg2HashVersionCalc64V1`, `WzVersionVerifier.cs:443-448`).
pub fn hash_version_1202_1204(hash1: u64, hash2: u64) -> u64 {
    hash1 ^ hash2 ^ MAGIC_64_V1
}

/// KMST1202–1204 검증식. 닫힌 형태이므로 항상 참이다 (값을 직접 만들어 냈다).
pub fn verify_1202_1204(hash1: u64, hash2: u64, hash_version: u64) -> bool {
    hash_version == hash1 ^ hash2 ^ MAGIC_64_V1
}

// ─────────────────────────────────────────────────────────────────────────────
// KMST1202 / KMST1206 버전 문자열 해시
// ─────────────────────────────────────────────────────────────────────────────

/// KMST1202/1205 버전 문자열 해시 (`WzVersionHasher.ComputePkg2HashVersion1202`).
///
/// # 함정
///
/// - 선두 4자리는 `<<` 로 분배하고, 이후는 홀수면 `ROL` / 짝수면 `ROR`.
/// - `c & 0x3F` 이므로 회전량이 0~63. 문자마다 달라진다.
pub fn compute_pkg2_hash_version_1202(version: &str) -> u64 {
    let mut hash: u64 = 0x6D4C_3B2A_91E1_0DA5;
    let units: Vec<u16> = version.encode_utf16().collect();
    for (i, &c) in units.iter().enumerate() {
        let cu = c as u64;
        if i < 4 {
            hash ^= cu << (i * 2);
        } else {
            let n = (c & 0x3F) as u32;
            hash = if i % 2 != 0 {
                hash.rotate_left(n)
            } else {
                hash.rotate_right(n)
            };
        }
    }
    hash
}

// ─────────────────────────────────────────────────────────────────────────────
// KMST1205 / KMST1206 — 버전 문자열이 하드코딩됨
// ─────────────────────────────────────────────────────────────────────────────

/// KMST1205 검증식 (`Pkg2HashVersionCalc64V2.Verify`, `WzVersionVerifier.cs:465-468`).
///
/// `hash2` 가 **정답인지 아닌지**를 알려주는 함수다. `hashVersion` 는
/// `KMST1205_1206_VERSION_STRING` 에서 미리 계산해 둔다.
pub fn verify_1205(hash1: u64, hash2: u64, hash_version: u64) -> bool {
    crate::offset::compute_hash2_1205(hash1, hash_version) == hash2
}

/// KMST1205 `hashVersion` 후보.
///
/// 검증에 실패하면 **빈 목록**을 준다 (C# 의 `Array.Empty<ulong>()`).
pub fn candidates_1205(hash1: u64, hash2: u64) -> Vec<u64> {
    let hv = compute_pkg2_hash_version_1202(KMST1205_1206_VERSION_STRING);
    if verify_1205(hash1, hash2, hv) {
        vec![hv]
    } else {
        Vec::new()
    }
}

/// KMST1206 `hashVersion` — HMAC-SHA256 기반
/// (`WzVersionHasher.ComputePkg2HashVersion1206`, `WzVersionHasher.cs:61-89`).
///
/// # 함정
///
/// 1. 메시지는 **UTF-16LE** (`Encoding.Unicode`).
/// 2. HMAC 을 **두 번** 겹친다: `HMAC(key2, HMAC(key1, msg))`.
/// 3. 결과의 **앞 8바이트만** 쓰고 **리틀 엔디언**으로 읽는다
///    (`BitConverter.ToUInt64`). 빅 엔디언으로 읽으면 다른 버전으로 판정된다.
pub fn compute_pkg2_hash_version_1206(version: &str) -> u64 {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    const KEY1: [u8; 32] = [
        0xDE, 0x17, 0x00, 0xB9, 0x5C, 0x7C, 0xF2, 0xFE, 0x10, 0x11, 0x5B, 0x6E, 0xE3, 0xE0, 0xE9,
        0x84, 0xA8, 0x35, 0x3C, 0xF4, 0x21, 0x87, 0xD8, 0x52, 0x52, 0xCA, 0x5E, 0x0E, 0x71, 0x3C,
        0x25, 0x87,
    ];
    /// 함정: `key2` 는 **17바이트**다 (16 이 아니라).
    const KEY2: [u8; 17] = [
        0xEC, 0xEB, 0x71, 0xCE, 0x04, 0xCD, 0x3B, 0xB3, 0xEE, 0xAD, 0x10, 0x78, 0x0E, 0x2F, 0x58,
        0x0A, 0x01,
    ];

    let msg: Vec<u8> = version
        .encode_utf16()
        .flat_map(|u| u.to_le_bytes())
        .collect();

    let mut mac = HmacSha256::new_from_slice(&KEY1).expect("HMAC 은 모든 키 길이를 받는다");
    mac.update(&msg);
    let inner = mac.finalize().into_bytes();

    let mut mac2 = HmacSha256::new_from_slice(&inner).expect("HMAC 키");
    mac2.update(&KEY2);
    let outer = mac2.finalize().into_bytes();

    u64::from_le_bytes(outer[0..8].try_into().expect("8바이트"))
}

/// KMST1206 검증식 (`Pkg2HashVersionCalc64V3`, `WzVersionVerifier.cs:485-491`).
///
/// `hashVersion` 가 하드코딩된 값과 **같아야** 하고, 그 값으로 계산한
/// `hash2` 도 일치해야 한다. 두 조건을 모두 본다.
pub fn verify_1206(hash1: u64, hash2: u64, hash_version: u64) -> bool {
    let expected = compute_pkg2_hash_version_1206(KMST1205_1206_VERSION_STRING);
    hash_version == expected && crate::offset::compute_hash2_1205(hash1, expected) == hash2
}

/// KMST1206 `hashVersion` 후보.
pub fn candidates_1206(hash1: u64, hash2: u64) -> Vec<u64> {
    let hv = compute_pkg2_hash_version_1206(KMST1205_1206_VERSION_STRING);
    if verify_1206(hash1, hash2, hv) {
        vec![hv]
    } else {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pkg1_hash_version_known_vector() {
        // '7' → 0x37, +1 = 0x38
        assert_eq!(compute_pkg1_hash_version(777), 0x0000_E738);
    }

    #[test]
    fn pkg1_encrypted_version_of_777() {
        // 0xFF ^ 0x00 ^ 0x00 ^ 0xE7 ^ 0x38
        assert_eq!(calc_encrypted_version(0x0000_E738), 0x20);
    }

    #[test]
    fn pkg1_roundtrip_finds_777() {
        let cands =
            pkg1_hash_version_candidates(calc_encrypted_version(compute_pkg1_hash_version(777)));
        assert!(
            cands.iter().any(|(v, _)| *v == 777),
            "777 이 후보에 있어야 한다: {:?}",
            &cands[..cands.len().min(5)]
        );
    }

    #[test]
    fn pkg1_candidates_are_verified() {
        // 반환된 후보는 전부 자기 검증식을 통과해야 한다.
        let cands = pkg1_hash_version_candidates(0x20);
        assert!(!cands.is_empty());
        for (v, hv) in &cands {
            assert_eq!(calc_encrypted_version(*hv), 0x20, "버전 {v}");
            assert_eq!(*hv, compute_pkg1_hash_version(*v), "버전 {v}");
        }
    }

    #[test]
    fn pkg1_fixed_candidate_is_777() {
        let c = pkg1_fixed_candidate();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].0, 777);
        assert_eq!(c[0].1, 0x0000_E738);
    }

    // ── 64비트 닫힌 형태는 검증이 쉬우므로 임의 값으로 왕복 확인 ──

    #[test]
    fn hash_version_1202_1204_is_closed_form() {
        for (h1, h2) in [(0u64, 0u64), (1, 2), (0xDEAD_BEEF_CAFE_F00D, 0x1234)] {
            let hv = hash_version_1202_1204(h1, h2);
            assert!(verify_1202_1204(h1, h2, hv));
            assert_eq!(hv, h1 ^ h2 ^ MAGIC_64_V1);
        }
    }

    #[test]
    fn compute_1202_is_deterministic() {
        let a = compute_pkg2_hash_version_1202(KMST1205_1206_VERSION_STRING);
        let b = compute_pkg2_hash_version_1202(KMST1205_1206_VERSION_STRING);
        assert_eq!(a, b);
    }

    #[test]
    fn compute_1202_mixes_every_character() {
        // 함정: 같은 문자를 연속으로 두면 ROL/ROR 가 상쇄되어 값이 같아진다.
        //   예: "ZZZZ" → ROR26, ROL26, ROR26, ROL26 = 항등
        // 그래서 **위치마다 다른 문자**로 바꿔야 실제로 섞이는지 볼 수 있다.
        let a = compute_pkg2_hash_version_1202("abcdPQRS");
        let b = compute_pkg2_hash_version_1202("abcdPQRT");
        assert_ne!(a, b, "8번째 문자가 값을 바꿔야 한다");

        let c = compute_pkg2_hash_version_1202("abcxPQRS");
        let d = compute_pkg2_hash_version_1202("abcyPQRS");
        assert_ne!(c, d, "4번째 문자도 값을 바꿔야 한다");
    }

    #[test]
    fn compute_1202_leading_four_are_xor_shifted() {
        // 선두 4자리는 `hash ^= c << (i*2)` — 시프트가 겹치지 않으므로
        // 순서를 바꿔도 결과가 달라야 한다 (XOR 는 교환법칙이지만 시프트가 다르다).
        let a = compute_pkg2_hash_version_1202("abcdPQRS");
        let b = compute_pkg2_hash_version_1202("dcbaPQRS");
        assert_ne!(a, b, "선두 4자리는 위치별로 다른 시프트 비트에 들어간다");
    }
}
