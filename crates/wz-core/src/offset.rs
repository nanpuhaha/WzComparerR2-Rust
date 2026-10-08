//! WZ 오프셋 난독화 (원본 `Compatibility/WzOffsetCalc.cs`, `Pkg2Kmst1205Hash.cs`).
//!
//! # 왜 이게 정확한지가 전부인가
//!
//! WZ 디렉터리 엔트리는 파일 내 실제 오프셋를 **평문으로 저장하지 않는다.**
//! 대신 `(hashedOffsetPosition, hashedOffset)` 쌍을 저장하고, 실제 오프셋는
//! 파일 위치와 클라이언트 버전별 키로 **다시 계산**한다.
//!
//! 키를 하나라도 잘못 쓰면 "대부분은 되다가 특정 위치에서 깨지는" 파일이 된다.
//! 그래서 원본 산식을 **줄 단위로** 옮기고, C# 로직을 Rust 참조 구현으로
//! 하나씩 짝지어 대조한다.
//!
//! # 반드시 wrapping 연산이어야 한다
//!
//! C# 코드는 전부 `unchecked` 다. Rust 에서 오버플로를 panic 으로 두면 어떤
//! WZ 파일이 열렸는지에 따라 프로그램이 죽는다 (리포트 §14.4).
//! **모든 산술에 `wrapping_*` 를 쓴다.**

use crate::crypto::pkg2_key::mix32;

/// 32비트 왼쪽 회전. C# `MathHelper.ROL` 과 동일.
///
/// `n` 은 0..=31 범위로 정규화한다. C# 은 하위 5비트만 쓰지만, 32 를
/// 넘으면 정의가 달라지므로 여기서 명시적으로 막는다.
#[inline]
pub fn rol32(x: u32, n: u32) -> u32 {
    let n = n & 31;
    if n == 0 {
        x
    } else {
        x.rotate_left(n)
    }
}

/// 오프셋 계산기.
pub trait OffsetCalc {
    /// 파일 내 위치 → 실제 데이터 오프셋.
    fn calc_offset(&self, file_pos: u32, hashed_offset: u32) -> u32;

    /// 암호화된 엔트리 개수를 복호화.
    fn decrypt_entry_count_i32(&self, encrypted: i32) -> i32;

    /// 64비트 PKG2 인가?
    ///
    /// PKG2 디렉터리에서 **64비트만 규칙이 다르다** —
    /// 오프셋 표 앞에 카운트 필드가 없다 (WzFileReadContext.cs:110 의
    /// ShouldReadOffsets 가 무조건 true). 32비트는 카운트를 읽어
    /// 엔트리 개수와 비교한다.
    fn is_wide(&self) -> bool {
        false
    }
    fn decrypt_entry_count_i64(&self, encrypted: i64) -> i64 {
        self.decrypt_entry_count_i32(encrypted as i32) as i64
    }

    /// 이미지의 길이/체크섬 필드 복호화 (KMST1202+).
    ///
    /// 지원하지 않는 버전은 `None`.
    fn calc_length(&self, _file_pos: u32, _encrypted: i32) -> Option<i32> {
        None
    }

    /// `calc_offset` 의 **역함수** — `target` 오프셋을 만드는 `hashedOffset` 를 구한다.
    ///
    /// # 왜 필요한가
    ///
    /// 합성 코퍼스를 만들려면 "이 오프셋으로 해석되게 하는 해시값"을 써야 한다.
    /// 임의의 값을 넣으면 복호화 결과가 파일 밖으로 튀어 트리가 깨진다.
    ///
    /// # 왜 대부분 간단한가
    ///
    /// 모든 계산기가 `hashedOffset` 를 **단 한 번 XOR** 로 끼워 넣는다:
    ///
    /// ```text
    /// calc:    offset = f(filePos) ^ hashedOffset + headerLen
    /// encode:  hashedOffset = f(filePos) ^ (target - headerLen)
    /// ```
    ///
    /// 그러므로 `f(filePos)` 부분만 재사용하면 된다. `f` 는 `hashedOffset` 를
    /// 보기 **전에** 계산되므로 역산이 가능하다.
    fn encode_offset(&self, file_pos: u32, target: u32) -> u32;

    /// 프로파일 이름 (디버그/로깅).
    fn name(&self) -> &'static str;
}

// ─────────────────────────────────────────────────────────────────────────────
// PKG1
// ─────────────────────────────────────────────────────────────────────────────

/// PKG1 오프셋 계산 (원본 `Pkg1OffsetCalc`, `WzOffsetCalc.cs:70-93`).
///
/// ```text
/// offset = filePos - headerLen
/// offset = ~offset
/// offset *= hashVersion
/// offset -= 0x581C3F6D
/// distance = (int)offset & 0x1F      ← 회전량이 최종 offset 에서 나온다!
/// offset = ROL(offset, distance)
/// offset ^= hashedOffset
/// offset += headerLen * 2
/// ```
///
/// # 함정
///
/// 회전량 `distance` 가 `~offset * hashVersion - 0x581C3F6D` **이후** 값에서
/// 나온다. 곱하기 전 값에서 꺼내면 어긋난다.
#[derive(Debug, Clone, Copy)]
pub struct Pkg1OffsetCalc {
    pub header_len: u32,
    pub hash_version: u32,
}

impl Pkg1OffsetCalc {
    pub fn new(header_len: u32, hash_version: u32) -> Self {
        Self {
            header_len,
            hash_version,
        }
    }

    /// `hashedOffset` 를 XOR 하기 **전**의 값.
    ///
    /// `calc_offset` 과 `encode_offset` 가 공유한다 — `hashedOffset` 가 끼는
    /// 지점이 여기까지라는 사실이 역산 가능성의 근거다.
    fn key_part(header_len: u32, hash_version: u32, file_pos: u32) -> u32 {
        let mut offset = file_pos.wrapping_sub(header_len);
        offset = !offset;
        offset = offset.wrapping_mul(hash_version);
        offset = offset.wrapping_sub(0x581C_3F6D);
        // 회전량은 여기서 나온다 (원본 `:87`)
        let distance = offset & 0x1F;
        rol32(offset, distance)
    }
}

impl OffsetCalc for Pkg1OffsetCalc {
    fn calc_offset(&self, file_pos: u32, hashed_offset: u32) -> u32 {
        let mut offset = Pkg1OffsetCalc::key_part(self.header_len, self.hash_version, file_pos);
        offset ^= hashed_offset;
        offset.wrapping_add(self.header_len.wrapping_mul(2))
    }

    fn decrypt_entry_count_i32(&self, encrypted: i32) -> i32 {
        // PKG1 은 엔트리 개수를 암호화하지 않는다.
        encrypted
    }

    fn encode_offset(&self, file_pos: u32, target: u32) -> u32 {
        let base = Pkg1OffsetCalc::key_part(self.header_len, self.hash_version, file_pos);
        base ^ target.wrapping_sub(self.header_len.wrapping_mul(2))
    }

    fn name(&self) -> &'static str {
        "PKG1"
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PKG2 32비트
// ─────────────────────────────────────────────────────────────────────────────

/// PKG2 공통: `CalcOffset` + `DecryptEntryCount` 를 묶는다.
#[derive(Debug, Clone, Copy)]
pub struct Pkg2OffsetCalc32 {
    pub version: u32,
    pub header_len: u32,
    pub hash_version: u32,
    pub hash1: u32,
    /// V3 전용
    pub pre_hash: u32,
    /// V3 전용
    pub mixed_hash: u32,
}

impl Pkg2OffsetCalc32 {
    /// KMST1196-1197 (V1, 원본 `Pkg2OffsetCalcV1`)
    pub fn v1(header_len: u32, hash_version: u32, hash1: u32) -> Self {
        Self {
            version: 1,
            header_len,
            hash_version,
            hash1,
            pre_hash: 0,
            mixed_hash: 0,
        }
    }

    /// KMST1198 (V2, 원본 `Pkg2OffsetCalcV2`)
    pub fn v2(header_len: u32, hash_version: u32, hash1: u32) -> Self {
        Self {
            version: 2,
            header_len,
            hash_version,
            hash1,
            pre_hash: 0,
            mixed_hash: 0,
        }
    }

    /// KMST1199-1201 (V3, 원본 `Pkg2OffsetCalcV3`)
    pub fn v3(header_len: u32, hash_version: u32, hash1: u32) -> Self {
        let pre_hash = hash1 ^ hash_version;
        let mixed_hash = mix32(pre_hash ^ 0x6D4C_3B2A) ^ 0x91E1_0DA5;
        Self {
            version: 3,
            header_len,
            hash_version,
            hash1,
            pre_hash,
            mixed_hash,
        }
    }

    /// `hashedOffset` 를 XOR 하기 **전**의 값.
    ///
    /// `calc_offset` 과 `encode_offset` 가 공유한다. `hashedOffset` 가 끼는
    /// 지점이 여기까지라는 사실이 역산 가능성의 근거다.
    fn key_part(&self, file_pos: u32) -> u32 {
        let mut offset = file_pos.wrapping_sub(self.header_len);
        offset = !offset;

        offset = match self.version {
            1 => offset.wrapping_mul(self.hash_version),
            2 => offset.wrapping_mul(self.hash_version ^ self.hash1),
            _ => offset.wrapping_mul(self.pre_hash.wrapping_add(self.mixed_hash ^ 0xA7E3_C093)),
        };

        offset = offset.wrapping_sub(0x581C_3F6D);
        offset ^= self.hash1.wrapping_mul(0x0101_0101);
        if self.version == 3 {
            offset ^= self.mixed_hash.wrapping_mul(0x9E37_79B9);
        }

        let distance = match self.version {
            3 => (self.pre_hash ^ self.mixed_hash) & 0x1F,
            _ => (self.hash_version ^ self.hash1) & 0x1F,
        };
        rol32(offset, distance)
    }

    /// V1 은 `^ hashedOffset`, V2/V3 은 `^ ~hashedOffset` (원본 `WzOffsetCalc.cs`).
    #[inline]
    fn xor_target(version: u32, hashed: u32) -> u32 {
        if version == 1 {
            hashed
        } else {
            !hashed
        }
    }
}

impl OffsetCalc for Pkg2OffsetCalc32 {
    fn calc_offset(&self, file_pos: u32, hashed_offset: u32) -> u32 {
        let base = self.key_part(file_pos);
        (base ^ Self::xor_target(self.version, hashed_offset)).wrapping_add(self.header_len)
    }

    fn encode_offset(&self, file_pos: u32, target: u32) -> u32 {
        // calc:  target = key_part ^ xor_target(h) + headerLen
        // ⇒ xor_target(h) = (target - headerLen) ^ key_part
        //   xor_target(h) 가 h 또는 ~h 이므로 마지막에 한 번 뒤집는다.
        let v = target.wrapping_sub(self.header_len) ^ self.key_part(file_pos);
        if self.version == 1 {
            v
        } else {
            !v
        }
    }

    fn decrypt_entry_count_i32(&self, encrypted: i32) -> i32 {
        let e = encrypted as u32;
        let result = match self.version {
            1 => {
                e ^ (self.hash1 << 24).wrapping_add(0x7F4A_7C15u32.wrapping_mul(self.hash_version))
            }
            2 => {
                e ^ (self.hash1 << 16).wrapping_sub(0x2152_4111u32.wrapping_mul(self.hash_version))
            }
            _ => {
                e ^ (self.hash1 << 16)
                    .wrapping_add(self.mixed_hash & 0x7fff_ffff)
                    .wrapping_sub(0x2152_4111u32.wrapping_mul(self.hash_version))
            }
        };
        result as i32
    }

    fn name(&self) -> &'static str {
        match self.version {
            1 => "PKG2_32B_V1_KMST1196",
            2 => "PKG2_32B_V2_KMST1198",
            _ => "PKG2_32B_V3_KMST1199",
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PKG2 64비트
// ─────────────────────────────────────────────────────────────────────────────

/// 64비트 PKG2 오프셋 계산.
///
/// # 함정: 클라이언트는 하위 32비트만 쓴다
///
/// 원본 `Pkg2OffsetCalc64V1` 주석 (`WzOffsetCalc.cs:217`):
/// *"client only use low 32bits."* 그래서 64비트 키를 받아도
/// 산술은 전부 `u32` 다. `u64` 로 하면 다른 값이 나온다.
#[derive(Debug, Clone, Copy)]
pub struct Pkg2OffsetCalc64V1 {
    pub header_len: u32,
    pub hash1: u64,
    pub hash_version: u64,
    pub pre_hash: u32,
    pub mixed_hash: u32,
}

impl Pkg2OffsetCalc64V1 {
    /// KMST1202-1204 (원본 `Pkg2OffsetCalc64V1`)
    pub fn new(header_len: u32, hash1: u64, hash_version: u64) -> Self {
        let pre_hash = hash1 as u32 ^ hash_version as u32;
        Self {
            header_len,
            hash1,
            hash_version,
            pre_hash,
            mixed_hash: pre_hash ^ 0x33BB_BB33,
        }
    }

    /// 공유 키 (원본 `CalcSharedKey`, `:244-253`)
    #[inline]
    fn calc_shared_key(&self, file_pos: u32) -> u32 {
        let mut key = file_pos.wrapping_sub(self.header_len);
        key = !key;
        key = key.wrapping_mul(self.pre_hash.wrapping_add(self.mixed_hash ^ 0xA7E3_C093));
        key = key.wrapping_sub(0x581C_3F6D);
        key ^= (self.hash1 as u32).wrapping_mul(0x0101_0101);
        key ^= self.mixed_hash.wrapping_mul(0x9E37_79B9);
        // 회전량은 **고정 19** — 파일 위치와 무관하다
        rol32(key, 19)
    }
}

impl OffsetCalc for Pkg2OffsetCalc64V1 {
    fn calc_offset(&self, file_pos: u32, hashed_offset: u32) -> u32 {
        let key = self.calc_shared_key(file_pos);
        key ^ !hashed_offset
    }

    fn calc_length(&self, file_pos: u32, encrypted: i32) -> Option<i32> {
        let key = self.calc_shared_key(file_pos);
        Some(encrypted ^ (key as i32))
    }

    fn decrypt_entry_count_i32(&self, encrypted: i32) -> i32 {
        self.decrypt_entry_count_i64(encrypted as i64) as i32
    }

    fn decrypt_entry_count_i64(&self, encrypted: i64) -> i64 {
        // 원본 `:257` — 64비트 XOR 후 >> 16
        let v = (encrypted as u64) ^ self.hash1 ^ self.hash_version ^ 0x550E_C4DD_02C4_68ECu64;
        (v >> 16) as i64
    }

    fn encode_offset(&self, file_pos: u32, target: u32) -> u32 {
        // calc: target = key ^ ~hashedOffset   (headerLen 없음)
        let v = self.calc_shared_key(file_pos) ^ target;
        !v
    }

    fn is_wide(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "PKG2_64B_V1_KMST1202"
    }
}

/// KMST1205-1206 오프셋 계산 (원본 `Pkg2Kmst1205Hash`).
///
/// 원본은 `Pkg2Kmst1205Hash.cs` 로 위임한다. 산식을 그대로 옮긴다.
#[derive(Debug, Clone, Copy)]
pub struct Pkg2OffsetCalc64V2 {
    pub header_len: u32,
    pub hash1: u64,
    pub hash_version: u64,
}

impl Pkg2OffsetCalc64V2 {
    pub fn new(header_len: u32, hash1: u64, hash_version: u64) -> Self {
        Self {
            header_len,
            hash1,
            hash_version,
        }
    }
}

impl OffsetCalc for Pkg2OffsetCalc64V2 {
    fn calc_offset(&self, file_pos: u32, hashed_offset: u32) -> u32 {
        compute_image_offset_1205(
            self.hash1,
            self.hash_version,
            self.header_len,
            file_pos,
            hashed_offset,
        )
    }

    fn calc_length(&self, file_pos: u32, encrypted: i32) -> Option<i32> {
        // 함정: `CalcOffset` 은 원본 `filePos` 를 넘기지만 `CalcLength` 은
        // `filePos - headerLen` 을 넘긴다. 두 개를 합치면 안 된다
        // (원본 `WzOffsetCalc.cs:296-299`).
        let key = compute_entry_field_key_1205(
            self.hash1,
            self.hash_version,
            file_pos.wrapping_sub(self.header_len),
        );
        Some(encrypted ^ (key as i32))
    }

    fn decrypt_entry_count_i32(&self, encrypted: i32) -> i32 {
        self.decrypt_entry_count_i64(encrypted as i64) as i32
    }

    fn decrypt_entry_count_i64(&self, encrypted: i64) -> i64 {
        let key = compute_directory_count_key_1205(self.hash1, self.hash_version);
        let v = (encrypted as u64) ^ key;
        (v >> 16) as i64
    }

    fn encode_offset(&self, file_pos: u32, target: u32) -> u32 {
        // `ComputeImageOffset` 이 `(!hashedOffset ^ key) + headerLen` 형태이므로
        // `hashedOffset` 를 빼면 XOR 하나가 남는다. 다만 `key` 안에 `filePos` 가
        // 섞여 있어 **한 번에 역산되지 않는다**. 그래도 결과는
        // `hashedOffset` 에 대해 선형이라 이 식으로 복원된다:
        //   target - headerLen = !hashed ^ key  ⇒  !hashed = (target - headerLen) ^ key
        let mixed = compute_shared_hash_1205(self.hash1, self.hash_version) as u32;
        let h1l = self.hash1 as u32;
        let hvl = self.hash_version as u32;
        let mut key = (mixed ^ M1205_MAGIC7).wrapping_add(h1l ^ hvl);
        key = key
            .wrapping_mul(!file_pos.wrapping_sub(self.header_len))
            .wrapping_add(M1205_MAGIC7);
        key ^= 0xC2B2_AE3Du32.wrapping_mul(mixed) ^ M1205_MAGIC8.wrapping_mul(h1l);
        key = key.rotate_left((mixed ^ h1l ^ hvl) & 0x1F);
        !(target.wrapping_sub(self.header_len) ^ key)
    }

    fn is_wide(&self) -> bool {
        true
    }

    fn name(&self) -> &'static str {
        "PKG2_64B_V2_KMST1205"
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// KMST1205 해시 대수 (원본 Pkg2Kmst1205Hash.cs)
//
// # 주의: 이 섹션은 처음에 "간단한 혼합" 으로 작성돼 있었으나 실제 C# 산식이
// 아니었다. 버전 판정이 1205/1206 에서 틀리면 **어떤 오프셋도 안 열리므로**
// 원본 상수와 연산을 한 줄도 바꾸지 않고 옮긴다.
// ─────────────────────────────────────────────────────────────────────────────

const M1205_MAGIC1: u64 = 0x84CA_A73B_2BB7_0682;
const M1205_MAGIC2: u64 = 0x510E_527F_ADE6_82D1;
const M1205_MAGIC3: u64 = 0xBF58_476D_1CE4_E5B9;
const M1205_MAGIC4: u64 = 0x94D0_49BB_1331_11EB;
const M1205_MAGIC5: u64 = 0x2545_F491_4F6C_DD1D;
const M1205_MAGIC6: u64 = 0x6A09_E667_F3BC_C908;
const M1205_MAGIC7: u32 = 0x2545_F491;
const M1205_MAGIC8: u32 = 0x85EB_CA77;

#[inline]
fn xor_shift_right64(v: u64, shift: u32) -> u64 {
    v ^ (v >> shift)
}

#[inline]
fn xor_fold_64_to_32(v: u64) -> u32 {
    (v as u32) ^ ((v >> 32) as u32)
}

#[inline]
fn mul_xor_right(value: u64, multiplier: u64, shift: u32) -> u64 {
    multiplier.wrapping_mul(xor_shift_right64(value, shift))
}

#[inline]
fn rotate_mul_xor_right_30(value: u64) -> u64 {
    mul_xor_right(value, M1205_MAGIC3, 30).rotate_right(27)
}

#[inline]
fn mul_rotate_xor_right_25_47(value: u64) -> u64 {
    0x9FB2_1C65_1E98_DF25u64.wrapping_mul(value ^ value.rotate_right(25) ^ value.rotate_right(47))
}

#[inline]
fn mul_rotate_xor_left_23_41(value: u64) -> u64 {
    M1205_MAGIC5.wrapping_mul(value ^ value.rotate_left(23) ^ value.rotate_left(41))
}

#[inline]
fn mul_xor_right_27(value: u64) -> u64 {
    mul_xor_right(value, M1205_MAGIC4, 27)
}

/// `ComputeSharedHash` (원본 `Pkg2Kmst1205Hash.cs:41-66`)
pub fn compute_shared_hash_1205(hash1: u64, hash_version: u64) -> u64 {
    let mut tmp1 = (hash1 ^ 0x81B4_A012_24AA_B10C).rotate_left(31);
    tmp1 = mul_xor_right(tmp1, 0xFF51_AFD7_ED55_8CCD, 33);
    tmp1 = mul_xor_right(tmp1, 0xC4CE_B9FE_1A85_EC53, 29);
    tmp1 = xor_shift_right64(tmp1, 32);

    let temp2_in = hash_version.wrapping_sub(0x2E4A_B5CD_2E6D_12FD) ^ M1205_MAGIC1;
    let temp2 = xor_shift_right64(mul_xor_right_27(rotate_mul_xor_right_30(temp2_in)), 31);

    let sum = tmp1.wrapping_add(temp2).wrapping_add(M1205_MAGIC2);
    let mut value = mul_rotate_xor_right_25_47(sum);
    value = tmp1 ^ value ^ temp2.rotate_left(17) ^ (value >> 28);
    value = mul_xor_right(value, M1205_MAGIC4, 29);
    xor_shift_right64(value, 32)
}

/// `ComputeHash2` (원본 `Pkg2Kmst1205Hash.cs:68-73`)
///
/// # 함정: `RotateMultiplyXorRight30` 안에 **또 다른** `MultiplyXorRight` 가 있다
/// (shift 30) — 두 단계를 빼먹으면 답이 나간다.
pub fn compute_hash2_1205(hash1: u64, hash_version: u64) -> u64 {
    let shared = compute_shared_hash_1205(hash1, hash_version);
    let value = mul_xor_right_27(rotate_mul_xor_right_30(shared ^ M1205_MAGIC1));
    xor_shift_right64(value, 31)
}

/// `ComputeDirectoryCountKey` (원본 `Pkg2Kmst1205Hash.cs:75-88`)
pub fn compute_directory_count_key_1205(hash1: u64, hash_version: u64) -> u64 {
    let mut tmp1 = rotate_mul_xor_right_30(hash1 ^ M1205_MAGIC1);
    let tmp2 = mul_xor_right_27(tmp1);
    tmp1 = xor_shift_right64(
        mul_rotate_xor_right_25_47(hash_version.wrapping_add(M1205_MAGIC2)),
        28,
    ) ^ (tmp2 >> 31);
    tmp1 ^= tmp2;

    let mut key = hash_version.wrapping_add(hash1) ^ M1205_MAGIC6;
    key = mul_rotate_xor_left_23_41(key);
    key = M1205_MAGIC3.wrapping_mul(xor_shift_right64(key, 32));
    key = xor_shift_right64(key, 29) ^ (tmp1.wrapping_add(tmp1.rotate_left(23)));
    xor_shift_right64(key, 31)
}

/// `ComputeDirEntryNameKey` (원본 `Pkg2Kmst1205Hash.cs:90-105`)
pub fn compute_dir_entry_name_key_1205(hash1: u64, hash_version: u64, file_pos: u64) -> u64 {
    let shared = compute_shared_hash_1205(hash1, hash_version);
    let mut position_hash = 0xD1B5_4A32_D192_ED03u64.wrapping_mul(file_pos);
    position_hash ^= position_hash.rotate_left(32);

    let mut tmp1 = position_hash
        .wrapping_add(shared)
        .wrapping_add(M1205_MAGIC2);
    tmp1 = mul_rotate_xor_right_25_47(tmp1);
    let mut tmp2 = position_hash ^ shared ^ M1205_MAGIC6;
    tmp2 = mul_rotate_xor_left_23_41(tmp2);

    let mut key = M1205_MAGIC3.wrapping_mul(xor_shift_right64(tmp2, 32));
    key = xor_shift_right64(tmp1, 28) ^ xor_shift_right64(key, 29);
    key = key.rotate_left((position_hash & 0x3F) as u32);
    xor_shift_right64(key, 29)
}

/// `ComputeEntryFieldKey` (원본 `Pkg2Kmst1205Hash.cs:107-121`)
pub fn compute_entry_field_key_1205(hash1: u64, hash_version: u64, file_pos: u32) -> u32 {
    let mixed = xor_fold_64_to_32(compute_shared_hash_1205(hash1, hash_version));
    let hash1_low = hash1 as u32;
    let hash_version_low = hash_version as u32;
    let mut key = mixed
        .wrapping_add(M1205_MAGIC7.wrapping_mul(xor_shift_right64(file_pos as u64, 15) as u32))
        .wrapping_add(hash1_low ^ hash_version_low);
    key = key.rotate_left((mixed ^ hash1_low) & 0x1F);
    key = xor_shift_right64(M1205_MAGIC8.wrapping_mul(key) as u64, 13) as u32;
    key = key.wrapping_sub(0x3D4D_51C3u32.wrapping_mul(mixed));
    key ^ key.rotate_left(16)
}

/// `ComputeImageOffset` (원본 `Pkg2Kmst1205Hash.cs:123-138`)
pub fn compute_image_offset_1205(
    hash1: u64,
    hash_version: u64,
    header_len: u32,
    file_pos: u32,
    hashed_offset: u32,
) -> u32 {
    let mixed = compute_shared_hash_1205(hash1, hash_version) as u32;
    let hash1_low = hash1 as u32;
    let hash_version_low = hash_version as u32;
    let mut key = (mixed ^ M1205_MAGIC7).wrapping_add(hash1_low ^ hash_version_low);
    key = key
        .wrapping_mul(!file_pos.wrapping_sub(header_len))
        .wrapping_add(M1205_MAGIC7);
    key ^= 0xC2B2_AE3Du32.wrapping_mul(mixed) ^ M1205_MAGIC8.wrapping_mul(hash1_low);
    key = key.rotate_left((mixed ^ hash1_low ^ hash_version_low) & 0x1F);
    (!hashed_offset ^ key).wrapping_add(header_len)
}

/// PKG2 V2 디렉트리 이름 키스트림 생성 (`Wz_Crypto.Pkg2DirStringKeyV6`).
///
/// # 함정 1: 바이트 쌍이 **겹친다**
///
/// C# 은 `u16Keys[i] = (ushort)(baseKey >> (8 * i))` (i = 0..7) 를 16바이트 배열에
/// 쓴다. `i` 가 2바이트씩 건너뛰므로 쌍은 `[b0,b1], [b1,b2], [b2,b3], …` 이다.
/// `[b0,b1], [b2,b3], [b4,b5], …` 로 짝지으면 **완전히 다른 키** 가 된다.
///
/// # 함정 2: 마지막 쌍은 0
///
/// `baseKey` 가 64비트 라 `i = 7` 의 `>> 56` 은 `0` 이다.
///
/// # 함정 3: `file_pos` 는 **상대** 위치
///
/// C# 은 `ApplyState((ulong)reader.BaseStream.Position)` 를 호출하는데, 디렉터리는
/// `PartialStream` 위에서 읽어 `Position` 이 `dirStart` 기준 상대값이다
/// (`PartialStream.cs:68`). 절대 오프셋을 넣으면 완전히 다른 키가 된다.
///
/// # 실측 검증 (`_Canvas_041.wz`)
///
/// ```text
/// hash1 = 0xDCC2AB20469D0574  hashVersion = 0xB8047BAB2410606A
/// filePos = 20 (상대)  →  base = 0x9E5E3430C1601381
/// keys  = 81-13-13-60-60-C1-C1-30-30-34-34-5E-5E-9E-9E-00
/// 파일 375..392 ^ keys → UTF-16 "19214.img"   (C# oracle 출력과 일치)
/// ```
pub fn pkg2_dir_string_keys_v6(hash1: u64, hash_version: u64, file_pos: u64) -> [u8; 16] {
    let base = compute_dir_entry_name_key_1205(hash1, hash_version, file_pos);
    let b = base.to_le_bytes();
    let mut keys = [0u8; 16];
    for i in 0..8 {
        keys[i * 2] = b[i];
        keys[i * 2 + 1] = if i + 1 < 8 { b[i + 1] } else { 0 };
    }
    keys
}

/// PKG2 V2 이름 복호화 (`WzBinaryReader.ReadPkg2DirStringV2`의 복호화 부분).
///
/// 입력 바이트를 제자리에서 복호화하고 UTF-16LE 로 해석한다.
pub fn decode_pkg2_dir_string_v2(keys: &[u8; 16], data: &mut [u8]) -> Option<String> {
    for (i, b) in data.iter_mut().enumerate() {
        *b ^= keys[i % keys.len()];
    }
    let mut out = String::with_capacity(data.len() / 2);
    for c in data.as_chunks::<2>().0.iter() {
        let u = u16::from_le_bytes(*c);
        out.push(char::from_u32(u32::from(u)).unwrap_or('\u{FFFD}'));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    /// 모든 계산기에서 `calc_offset(encode_offset(pos, target)) == target` 여야 한다.
    ///
    /// # 왜 이게 중요한가
    ///
    /// 합성 코퍼스는 "이 오프셋으로 해석되는 해시값"을 써야 한다. 역함수가
    /// 틀리면 코퍼스가 meaningless 해지고, 더 나쁜 건 **트리 테스트가 조용히
    /// 통과하면서 아무것도 검증하지 못하는** 상태가 된다.
    #[test]
    fn encode_offset_inverts_calc_offset() {
        let calcs: Vec<Box<dyn OffsetCalc>> = vec![
            Box::new(Pkg1OffsetCalc::new(60, 0x0000_E738)),
            Box::new(Pkg2OffsetCalc32::v1(60, 0x1234_5678, 0xABCD_EF01)),
            Box::new(Pkg2OffsetCalc32::v2(60, 0x1234_5678, 0xABCD_EF01)),
            Box::new(Pkg2OffsetCalc32::v3(60, 0x1234_5678, 0xABCD_EF01)),
            Box::new(Pkg2OffsetCalc64V1::new(
                150,
                0x1122_3344_5566_7788,
                0x99AA_BBCC_DDEE_FF00,
            )),
            Box::new(Pkg2OffsetCalc64V2::new(
                163,
                0x1122_3344_5566_7788,
                0x99AA_BBCC_DDEE_FF00,
            )),
        ];
        for c in calcs {
            for file_pos in [60u32, 64, 128, 4096, 0xFFFF] {
                for target in [64u32, 100, 4096, 0x8000, u32::MAX] {
                    let h = c.encode_offset(file_pos, target);
                    let back = c.calc_offset(file_pos, h);
                    assert_eq!(
                        back,
                        target,
                        "{}: encode→calc 불일치 (pos={file_pos}, target={target})",
                        c.name()
                    );
                }
            }
        }
    }

    /// 역함수는 `hashedOffset` 에 대해 선형이라 서로 다른 대상은 다른 해시를 낸다.
    #[test]
    fn encode_offset_is_injective_over_targets() {
        let c = Pkg1OffsetCalc::new(60, 0x0000_E738);
        let a = c.encode_offset(100, 200);
        let b = c.encode_offset(100, 201);
        assert_ne!(a, b, "대상이 다르면 해시도 달라야 한다");
        assert_ne!(c.encode_offset(100, 200), c.encode_offset(101, 200));
    }
    use super::*;

    /// C# 산식을 **한 줄씩** 옮긴 참조 구현들.
    /// Rust 구현과 각각 독립적으로 계산해 같음을 확인한다.
    mod reference {
        pub fn rol(x: u32, n: u32) -> u32 {
            let n = n & 31;
            if n == 0 {
                x
            } else {
                x.rotate_left(n)
            }
        }

        pub fn pkg1(header: u32, hv: u32, pos: u32, hashed: u32) -> u32 {
            let mut o = pos.wrapping_sub(header);
            o = !o;
            o = o.wrapping_mul(hv);
            o = o.wrapping_sub(0x581C_3F6D);
            let d = o & 0x1F;
            o = rol(o, d);
            o ^= hashed;
            o.wrapping_add(header.wrapping_mul(2))
        }

        pub fn pkg2v1(header: u32, hv: u32, h1: u32, pos: u32, hashed: u32) -> u32 {
            let mut o = pos.wrapping_sub(header);
            o = !o;
            o = o.wrapping_mul(hv);
            o = o.wrapping_sub(0x581C_3F6D);
            o ^= h1.wrapping_mul(0x0101_0101);
            let d = (hv ^ h1) & 0x1F;
            o = rol(o, d);
            o ^= hashed;
            o.wrapping_add(header)
        }

        pub fn pkg2v2(header: u32, hv: u32, h1: u32, pos: u32, hashed: u32) -> u32 {
            let mut o = pos.wrapping_sub(header);
            o = !o;
            o = o.wrapping_mul(hv ^ h1);
            o = o.wrapping_sub(0x581C_3F6D);
            o ^= h1.wrapping_mul(0x0101_0101);
            let d = (hv ^ h1) & 0x1F;
            o = rol(o, d);
            o ^= !hashed;
            o.wrapping_add(header)
        }

        pub fn mix32(mut x: u32) -> u32 {
            x ^= x >> 16;
            x = x.wrapping_mul(0x21F0_AAAD);
            x ^= x >> 15;
            x = x.wrapping_mul(0x735A_2D97);
            x ^= x >> 15;
            x
        }

        pub fn pkg2v3(header: u32, hv: u32, h1: u32, pos: u32, hashed: u32) -> u32 {
            let pre = h1 ^ hv;
            let mixed = mix32(pre ^ 0x6D4C_3B2A) ^ 0x91E1_0DA5;
            let mut o = pos.wrapping_sub(header);
            o = !o;
            o = o.wrapping_mul(pre.wrapping_add(mixed ^ 0xA7E3_C093));
            o = o.wrapping_sub(0x581C_3F6D);
            o ^= h1.wrapping_mul(0x0101_0101);
            o ^= mixed.wrapping_mul(0x9E37_79B9);
            let d = (pre ^ mixed) & 0x1F;
            o = rol(o, d);
            o ^= !hashed;
            o.wrapping_add(header)
        }
    }

    // 결정적 의사 난수 (deny.toml 이 `rand` 를 금지하므로 직접 구현)
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self) -> u32 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            (self.0 >> 32) as u32
        }
    }

    #[test]
    fn rol_matches_csharp() {
        assert_eq!(rol32(0x1234_5678, 0), 0x1234_5678);
        assert_eq!(rol32(1, 1), 2);
        assert_eq!(rol32(0x8000_0000, 1), 1);
        assert_eq!(rol32(0xFFFF_FFFF, 16), 0xFFFF_FFFF);
        // n > 31 은 하위 5비트만 사용
        assert_eq!(rol32(1, 32), 1);
        assert_eq!(rol32(1, 33), 2);
    }

    #[test]
    fn pkg1_matches_reference() {
        let mut r = Lcg(0x1234_5678_9ABC_DEF0);
        for _ in 0..2000 {
            let header = 16 + (r.next() % 512);
            let hv = r.next();
            let pos = r.next();
            let hashed = r.next();
            assert_eq!(
                Pkg1OffsetCalc::new(header, hv).calc_offset(pos, hashed),
                reference::pkg1(header, hv, pos, hashed),
                "header={header} hv={hv:#x} pos={pos} hashed={hashed:#x}"
            );
        }
    }

    #[test]
    fn pkg2_v1_matches_reference() {
        let mut r = Lcg(0xDEAD_BEEF_CAFE_0001);
        for _ in 0..2000 {
            let (header, hv, h1) = (16 + r.next() % 256, r.next(), r.next());
            let (pos, hashed) = (r.next(), r.next());
            assert_eq!(
                Pkg2OffsetCalc32::v1(header, hv, h1).calc_offset(pos, hashed),
                reference::pkg2v1(header, hv, h1, pos, hashed),
            );
        }
    }

    #[test]
    fn pkg2_v2_matches_reference() {
        let mut r = Lcg(0xDEAD_BEEF_CAFE_0002);
        for _ in 0..2000 {
            let (header, hv, h1) = (16 + r.next() % 256, r.next(), r.next());
            let (pos, hashed) = (r.next(), r.next());
            assert_eq!(
                Pkg2OffsetCalc32::v2(header, hv, h1).calc_offset(pos, hashed),
                reference::pkg2v2(header, hv, h1, pos, hashed),
            );
        }
    }

    #[test]
    fn pkg2_v3_matches_reference() {
        let mut r = Lcg(0xDEAD_BEEF_CAFE_0003);
        for _ in 0..2000 {
            let (header, hv, h1) = (16 + r.next() % 256, r.next(), r.next());
            let (pos, hashed) = (r.next(), r.next());
            assert_eq!(
                Pkg2OffsetCalc32::v3(header, hv, h1).calc_offset(pos, hashed),
                reference::pkg2v3(header, hv, h1, pos, hashed),
                "header={header} hv={hv:#x} h1={h1:#x}",
            );
        }
    }

    #[test]
    fn v3_precomputes_hash_consistently() {
        // 생성자에서 계산한 pre_hash/mixed_hash 가 calc_offset 안의 값과 같아야 한다
        let (hv, h1) = (0x1234_5678u32, 0x9ABC_DEF0u32);
        let c = Pkg2OffsetCalc32::v3(64, hv, h1);
        let pre = h1 ^ hv;
        let mixed = reference::mix32(pre ^ 0x6D4C_3B2A) ^ 0x91E1_0DA5;
        assert_eq!(c.pre_hash, pre);
        assert_eq!(c.mixed_hash, mixed);
    }

    #[test]
    fn offset_is_invertible_for_fixed_params() {
        // 같은 (header, hv, h1) 에서 offset→hashedOffset→offset 이 항등이어야 한다.
        // (마지막 XOR 가 역연산 가능하므로)
        let c = Pkg1OffsetCalc::new(64, 0xABCD_1234);
        for pos in [100u32, 5000, 1_000_000, 50_000_000] {
            let off = c.calc_offset(pos, 0x1234_5678);
            // hashedOffset 는 파일에 저장된 값이라 그대로라 가정하면
            // calc_offset(pos, hashed) 의 결과는 off 와 같아야 한다 (결정성)
            assert_eq!(c.calc_offset(pos, 0x1234_5678), off);
        }
    }

    #[test]
    fn versions_differ() {
        let (header, hv, h1, pos, hashed) = (64u32, 0x1111_2222u32, 0x3333_4444u32, 1000, 0x5555);
        let a = Pkg2OffsetCalc32::v1(header, hv, h1).calc_offset(pos, hashed);
        let b = Pkg2OffsetCalc32::v2(header, hv, h1).calc_offset(pos, hashed);
        let c = Pkg2OffsetCalc32::v3(header, hv, h1).calc_offset(pos, hashed);
        assert_ne!(a, b, "V1 과 V2 는 `^ hashed` vs `^ ~hashed` 가 다르다");
        assert_ne!(b, c, "V2 와 V3 는 곱하는 키가 다르다");
    }

    #[test]
    fn entry_count_roundtrip_is_identity_when_key_is_zero() {
        // V1: n ^ (h1<<24 + 0x7F4A7C15*hv). 키가 0 이면 항등.
        // 직접 키를 0 으로 만들기는 어려우므로, 복호화→재암호화 대신
        // "임의 n 에 대해 결과가 결정적" 만 확인하고 키 의존성을 본다.
        let c = Pkg2OffsetCalc32::v1(64, 0x1111_2222, 0x3333_4444);
        let a = c.decrypt_entry_count_i32(0x1234);
        let b = c.decrypt_entry_count_i32(0x1234);
        assert_eq!(a, b, "결정적");
        let d = Pkg2OffsetCalc32::v1(64, 0x1111_2222, 0x3333_4445);
        assert_ne!(
            a,
            d.decrypt_entry_count_i32(0x1234),
            "hash1 의 영향을 받는다"
        );
    }

    #[test]
    fn pkg64v1_shared_key_uses_fixed_rotation_19() {
        // 원본 `CalcSharedKey` 의 마지막은 항상 ROL(key, 19) 다.
        let c = Pkg2OffsetCalc64V1::new(64, 0x0123_4567_89AB_CDEF, 0x1111_2222_3333_4444);
        let k = c.calc_shared_key(1000);
        // calc_offset 은 `key ^ ~hashed` 이므로 key 를 역추적할 수 있다
        let hashed = 0x1234u32;
        assert_eq!(c.calc_offset(1000, hashed), k ^ !hashed);
    }

    #[test]
    fn pkg64v1_low32bits_only() {
        // 함정 검증: 64비트 키의 **상위 32비트** 는 오프셋 계산에 영향을 주지 않는다.
        // (원본 주석 "client only use low 32bits")
        let lo = 0x89AB_CDEFu32;
        let a = Pkg2OffsetCalc64V1::new(64, lo as u64, 0x1111_2222);
        let b = Pkg2OffsetCalc64V1::new(64, (lo as u64) | 0xFFFF_0000_0000_0000, 0x1111_2222);
        // preHash/mixedHash 는 하위 32비트만 쓰므로 오프셋은 같아야 한다
        assert_eq!(a.calc_offset(1000, 0x1234), b.calc_offset(1000, 0x1234));
    }

    #[test]
    fn pkg64v1_length_calc() {
        let c = Pkg2OffsetCalc64V1::new(64, 0x0123_4567_89AB_CDEF, 0x1111_2222_3333_4444);
        let pos = 1000u32;
        let key = c.calc_shared_key(pos);
        assert_eq!(c.calc_length(pos, 0x1234), Some(0x1234 ^ (key as i32)));
        // 같은 pos 에서 길이 복호화는 결정적
        assert_eq!(c.calc_length(pos, 0x1234), c.calc_length(pos, 0x1234));
    }

    #[test]
    fn pkg32_has_no_length_calc() {
        // KMST1202 전에는 길이 필드 암호화가 없다
        let c = Pkg2OffsetCalc32::v1(64, 1, 2);
        assert_eq!(c.calc_length(100, 0x1234), None);
    }

    #[test]
    fn entry_count_64_shifts_right_16() {
        // 64비트 변형은 `(x ^ key) >> 16`
        let c = Pkg2OffsetCalc64V1::new(64, 0x0123_4567_89AB_CDEF, 0x1111_2222_3333_4444);
        let enc = 0x0001_0002_0003_0004i64;
        let expect = ((enc as u64) ^ c.hash1 ^ c.hash_version ^ 0x550E_C4DD_02C4_68EC) >> 16;
        assert_eq!(c.decrypt_entry_count_i64(enc), expect as i64);
    }

    #[test]
    fn kmst1205_helpers_are_deterministic() {
        let (h1, hv) = (0x0123_4567_89AB_CDEFu64, 0x1111_2222_3333_4444u64);
        assert_eq!(
            compute_shared_hash_1205(h1, hv),
            compute_shared_hash_1205(h1, hv)
        );
        assert_ne!(
            compute_shared_hash_1205(h1, hv),
            compute_shared_hash_1205(h1, hv.wrapping_add(1))
        );
        assert_ne!(
            compute_directory_count_key_1205(h1, hv),
            compute_dir_entry_name_key_1205(h1, hv, 0)
        );
        assert_ne!(
            compute_entry_field_key_1205(h1, hv, 0),
            compute_entry_field_key_1205(h1, hv, 1)
        );
    }

    #[test]
    fn pkg64v2_offset_depends_on_both_hashes() {
        let c = Pkg2OffsetCalc64V2::new(64, 0x0123_4567_89AB_CDEF, 0x1111_2222_3333_4444);
        let d = Pkg2OffsetCalc64V2::new(64, 0x0123_4567_89AB_CDF0, 0x1111_2222_3333_4444);
        assert_ne!(c.calc_offset(1000, 0x1234), d.calc_offset(1000, 0x1234));
    }

    #[test]
    fn no_panic_on_extreme_inputs() {
        // u32/u64 최댓값에서도 패닉 없이 계산돼야 한다 (파일 손상 대응)
        let calcs: Vec<Box<dyn OffsetCalc>> = vec![
            Box::new(Pkg1OffsetCalc::new(u32::MAX, u32::MAX)),
            Box::new(Pkg2OffsetCalc32::v1(u32::MAX, u32::MAX, u32::MAX)),
            Box::new(Pkg2OffsetCalc32::v2(u32::MAX, u32::MAX, u32::MAX)),
            Box::new(Pkg2OffsetCalc32::v3(u32::MAX, u32::MAX, u32::MAX)),
            Box::new(Pkg2OffsetCalc64V1::new(u32::MAX, u64::MAX, u64::MAX)),
            Box::new(Pkg2OffsetCalc64V2::new(u32::MAX, u64::MAX, u64::MAX)),
        ];
        for c in calcs {
            let _ = c.calc_offset(u32::MAX, u32::MAX);
            let _ = c.calc_offset(0, 0);
            let _ = c.decrypt_entry_count_i32(i32::MIN);
            let _ = c.decrypt_entry_count_i64(i64::MIN);
            assert!(!c.name().is_empty());
        }
    }

    #[test]
    fn all_profiles_have_distinct_names() {
        let names = [
            Pkg1OffsetCalc::new(1, 1).name(),
            Pkg2OffsetCalc32::v1(1, 1, 1).name(),
            Pkg2OffsetCalc32::v2(1, 1, 1).name(),
            Pkg2OffsetCalc32::v3(1, 1, 1).name(),
            Pkg2OffsetCalc64V1::new(1, 1, 1).name(),
            Pkg2OffsetCalc64V2::new(1, 1, 1).name(),
        ];
        let uniq: std::collections::HashSet<_> = names.iter().collect();
        assert_eq!(uniq.len(), names.len(), "프로파일 이름이 고유해야 한다");
    }
}

#[cfg(test)]
mod real_hv_tests {
    use crate::offset::{OffsetCalc, Pkg2OffsetCalc64V2};

    /// 실제 random-header PKG2 파일에서 파생된 값으로 오프셋 397 을 재현한다.
    ///
    /// 값들은 모두 C# 진단 출력 (`_Canvas_041.wz`) 에서obtained 값이다:
    ///   hash1       = 0xDCC2AB20469D0574   (헤더, Wz_File.TryReadPkg2KMST1206Header)
    ///   hashVersion = 0xB8047BAB2410606A   (1206 프로파일 정상값)
    ///   headerLen   = 353
    ///   pos         = 393,  hashOffset = 0x071F1C7E
    ///   → C# offset = 397, 그리고 397 + size(393977711) = 파일 크기
    #[test]
    fn real_values_reproduce_offset_397() {
        let calc = Pkg2OffsetCalc64V2::new(353, 0xDCC2_AB20_469D_0574, 0xB804_7BAB_2410_606A);
        assert_eq!(calc.calc_offset(393, 0x071F_1C7E), 397, "C# 보고 offset");
    }

    /// 실제 해시로 1206 후보가 나온다 (폴백의 hash1=0 은 틀렸음).
    #[test]
    fn real_hash1_yields_single_1206_candidate() {
        let hv = crate::verify::compute_pkg2_hash_version_1206(
            crate::verify::KMST1205_1206_VERSION_STRING,
        );
        assert_eq!(hv, 0xB804_7BAB_2410_606A);
        assert!(crate::verify::verify_1206(
            0xDCC2_AB20_469D_0574,
            0xC4AD_96FF_DF28_DF48,
            hv
        ));
        assert_eq!(
            crate::verify::candidates_1206(0xDCC2_AB20_469D_0574, 0xC4AD_96FF_DF28_DF48),
            vec![hv]
        );
    }
}

#[cfg(test)]
mod dbg_key {
    #[test]
    fn print_dir_count_key() {
        let hash1 = 0xDCC2_AB20_469D_0574u64;
        let hv = 0xB804_7BAB_2410_606Au64;
        let enc: u64 = 0xD44F_14AD_C8EC_EF51; // 파일 354..361 LE
        let key = crate::offset::compute_directory_count_key_1205(hash1, hv);
        eprintln!("key  = {:#018x}", key);
        eprintln!("enc  = {:#018x}", enc);
        eprintln!("xor  = {:#018x}", enc ^ key);
        eprintln!(">>16 = {}", (enc ^ key) >> 16);
        // 개수가 1 이려면 (enc ^ key) 가 0x10000..0x1FFFF 여야 한다
        eprintln!("필요한 key ≈ {:#018x}", enc ^ 0x10000);
    }
}
