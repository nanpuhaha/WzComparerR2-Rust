//! WZ 컨테이너 포맴 감지 (리포트 §6.1 `detect/`).
//!
//! # 진행 상태
//!
//! | 계층 | 상태 |
//! |---|---|
//! | 매직 바이트 판별 (PKG1 / PKG2) | ✅ 이 모듈 |
//! | 헤더 레이아웃 파싱 (headerSize, hash1/2) | ✅ 이 모듈 |
//! | random-header 5종 (KMST1201/1202/1204/1205/1206) | ⏳ Phase 1.9 |
//! | `hashVersion` 역산 (32b V1~V5, 64b V1~V3) | ⏳ Phase 1.9 |
//! | 오프셋 검증 (비겹침) 후 프로파일 확정 | ⏳ Phase 1.10 |
//! | `Ms_File` / `Ms_FileV2` trial 판별 | ⏳ Phase 1.11 |
//!
//! 매직만으로 판별되지 않는 경우에도 **진단 정보**를 최대한 내보낸다.
//! 미지원 포맷이어도 사용자가 무엇이 문제인지 알 수 있어야 한다
//! (리포트 §12 R3: 새 클라이언트가 새 난독화를 도입하는 경우가 남는다).

use wz_error::{Result, WzFormatError};
use wz_pool::ByteSource;

/// 컨테이너 매직 바이트.
pub mod magic {
    pub const PKG1: [u8; 4] = *b"PKG1";
    pub const PKG2: [u8; 4] = *b"PKG2";
    /// MS 컨테이너는 매직이 없다 (`Ms_File` trial 판별) — 리포트 §2.3
    pub const MS_NONE: Option<[u8; 4]> = None;
}

/// 감지 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detected {
    /// 감지된 프로파일 이름 (예: `PKG1`, `PKG2_32B_HEADER`).
    pub profile: String,
    /// PKG2 헤더의 `hash1` (PKG1 이면 `None`).
    pub hash1: Option<u64>,
    /// PKG2 헤더의 `hash2`.
    pub hash2: Option<u64>,
    /// 헤더 길이 (`dataSize` 가 시작되는 오프셋).
    pub header_len: u32,
    /// 감지된 능력치 비트 (원본 `Wz_Capabilities`).
    pub capabilities: Capabilities,
    /// PKG1 `encver` 값 (헤더 바로 다음 u16).
    ///
    /// # 왜 보관해야 하는가
    ///
    /// `hashVersion` 은 `encver` 의 반전이라 후보를 만들려면 **필수**다
    /// (`WzVersionProfile.cs` 의 `Pkg1HashVersionCalc`). 감지 단계에서 이미
    /// 읽는데 보관하지 않으면 버전 감지에서 후보를 만들 수 없어
    /// **PKG1 파일 전체가 미분류**가 된다.
    ///
    /// `encver_missing` 면 `None` (값이 없고 777 고정).
    pub encver: Option<u32>,
}
/// 원본 `Wz_Capabilities` 대응 (`Wz_Capabilities.cs:9-22`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Capabilities {
    /// `encver` 필드 없음 (KMST1132 이후, wzver 는 777 고정).
    pub encver_missing: bool,
    /// 랜덤 헤더 PKG2.
    pub pkg2_random_header: bool,
    /// 랜덤 헤더 PKG2 (64비트 변형).
    pub pkg2_random_header64: bool,
    /// 어떤 알려진 프로파일에도 안 맞아 폴백 경로로 진입.
    pub unknown_pkg2: bool,
}

impl Capabilities {
    /// 비트 값으로 (원본과 동일한 배정).
    pub fn bits(&self) -> u32 {
        let mut v = 0;
        if self.encver_missing {
            v |= 1;
        }
        if self.pkg2_random_header {
            v |= 2;
        }
        if self.pkg2_random_header64 {
            v |= 4;
        }
        v
    }
}

/// 감지 시도 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DetectError {
    /// 파일이 너무 작음.
    TooShort { len: u64, need: u64 },
    /// 매직이 어떤 컨테이너와도 불일치.
    UnknownMagic { magic: [u8; 4], offset: u64 },
    /// 컨테이너는 알겠는데 지원하지 않는 세대.
    Unsupported { profile: String, detail: String },
}

impl std::fmt::Display for DetectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DetectError::TooShort { len, need } => {
                write!(f, "파일이 너무 짧음: {len} 바이트 (최소 {need})")
            }
            DetectError::UnknownMagic { magic, offset } => write!(
                f,
                "알 수 없는 매직 {:02X?} (오프셋 {offset}) — PKG1/PKG2 가 아니며 MS 계열 trial 판별 필요",
                magic
            ),
            DetectError::Unsupported { profile, detail } => {
                write!(f, "지원하지 않는 프로파일 `{profile}`: {detail}")
            }
        }
    }
}

impl std::error::Error for DetectError {}

impl From<DetectError> for WzFormatError {
    fn from(e: DetectError) -> Self {
        match e {
            DetectError::UnknownMagic { magic, offset } => {
                WzFormatError::UnknownMagic { magic, offset }
            }
            other => WzFormatError::UnsupportedProfile {
                profile: format!("{other}"),
                path_hint: String::new(),
            },
        }
    }
}

/// 최소 헤더 크기. PKG 헤더는 `4 + 8 + 4 = 16` 바이트 + copyright.
const MIN_HEADER: u64 = 16;

/// 파일 앞부분을 읽는다.
fn read_magic(src: &dyn ByteSource) -> Result<[u8; 4]> {
    let mut m = [0u8; 4];
    src.try_read_exact(0, &mut m)?;
    Ok(m)
}

/// 컨테이너를 감지한다.
///
/// # Errors
/// - 파일이 너무 짧으면 [`DetectError::TooShort`]
/// - 매직이 불일치하면 [`DetectError::UnknownMagic`]
///   (MS 계열은 trial 판별이 필요하므로 Phase 1.11 전까지 여기서 걸린다)
pub fn detect(src: &dyn ByteSource) -> std::result::Result<Detected, DetectError> {
    let len = src.len().map_err(|_| DetectError::TooShort {
        len: 0,
        need: MIN_HEADER,
    })?;
    if len < MIN_HEADER {
        return Err(DetectError::TooShort {
            len,
            need: MIN_HEADER,
        });
    }

    let magic = read_magic(src).map_err(|_| DetectError::TooShort { len, need: 4 })?;

    if magic == magic::PKG1 {
        return detect_pkg1(src);
    }
    if magic == magic::PKG2 {
        return detect_pkg2(src);
    }

    // 매직이 둘 다 아니면 **random-header PKG2** 일 수 있다.
    //
    // 실측: `C:/Nexon/M` 788개 중 192개(24%)가 이 형태다. 첫 4바이트가
    // 무작위라 위 매직 비교로는 절대 못 잡는다.
    if let Some(d) = detect_pkg2_random_header_all(src, len)? {
        return Ok(d);
    }

    Err(DetectError::UnknownMagic { magic, offset: 0 })
}

/// random-header PKG2 감지 (`Wz_File.cs:227-246`).
///
/// # 왜 매직으로 안 잡히나
///
/// KMST1201+ PKG2 는 오프셋 0 에 `PKG2` 문자열이 **없다**. 대신 353바이트
/// 랜덤 블록 뒤에 마커 바이트가 온다. 그래서 "첫 4바이트 == PKG2" 판별은
/// 이 파일들에서 **전부 실패**한다.
///
/// # 판별 규칙
///
/// 오프셋 353 의 바이트가 `0x80` 이면 random-header PKG2 다.
/// 원본은 353 부터 스캔해 첫 `0x80` 을 찾지만, 곧바로 이어지는
/// `if (dataStartPos >= 354) break` 때문에 **오프셋 353 만** 인정된다.
/// 그보다 뒤의 `0x80` 은 실패로 나간다.
///
/// ```text
/// +353  0x80          ← 마커. dataStartPos = 353 = headerSize
/// ```
///
/// # 함정: 오프셋은 353 이지 354 가 아니다
///
/// 직관적으로 "354바이트 헤더" 처럼 읽히지만 실제 값은 **353** 이다.
/// 원본이 `dataStartPos = position - 1` 로 마커 바이트 **뒤** 위치를
/// 쓰기 때문이다. 354 를 넣으면 디렉터리 오프셋이 전부 1씩 밀린다.
///
/// # 거짓 양성
///
/// `0x80` 은 흔한 바이트라 PKG1 파일에도 우연히 나올 수 있다. 다만
/// [`detect`] 가 매직을 먼저 확인하므로 PKG1 은 여기까지 오지 않는다.
/// 오프셋 목록으로 `u32` 모은다 (`MathHelper.GatherAsUInt32`).
fn gather_u32(buf: &[u8], offsets: &[usize; 4]) -> u32 {
    u32::from(buf[offsets[0]])
        | (u32::from(buf[offsets[1]]) << 8)
        | (u32::from(buf[offsets[2]]) << 16)
        | (u32::from(buf[offsets[3]]) << 24)
}

/// 오프셋 목록으로 `u64` 모은다 (`MathHelper.GatherAsUInt64`).
fn gather_u64(buf: &[u8], offsets: &[usize; 8]) -> u64 {
    let mut v = 0u64;
    for (i, &o) in offsets.iter().enumerate() {
        v |= u64::from(buf[o]) << (i * 8);
    }
    v
}

/// 위치 마스크 (`Wz_File.GatherMask`).
///
/// # 함정: `0x5A - 0x65 * offset` 는 **u8 로 잘린다**
///
/// C# 이 `(byte)` 캐스트를 하므로 음수가 될 수 있다. Rust 에서 `wrapping_sub`
/// 없이 계산하면 오버플로 패닉이 난다.
fn gather_mask(offsets: &[usize]) -> Vec<u8> {
    offsets
        .iter()
        .map(|&o| 0x5Au8.wrapping_sub(0x65u8.wrapping_mul(o as u8)))
        .collect()
}

fn gather_mask_u32(offsets: &[usize; 4]) -> u32 {
    let m = gather_mask(offsets);
    u32::from_le_bytes([m[0], m[1], m[2], m[3]])
}

fn gather_mask_u64(offsets: &[usize; 8]) -> u64 {
    let m = gather_mask(offsets);
    u64::from_le_bytes([m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7]])
}

/// 64비트 PKG2 random-header 규칙 하나.
struct RandomHeader64 {
    wz_version: u32,
    header_len: u32,
    hash1_offsets: [usize; 8],
    hash2_offsets: [usize; 8],
    data_size_offsets: [usize; 4],
    /// KMST1206 만 true. 필드가 위치 마스크로 XOR 되어 있다.
    masked: bool,
}

/// 규칙 목록 — **C# 호출 순서 그대로** (`Wz_File.cs:132-162`).
///
/// 순서가 중요하다. 1206 은 353 바이트, 1205 는 163 바이트인데 둘 다
/// `dataSize` 검사를 통과할 수 있으므로 먼저 온 것이 이긴다.
const RANDOM_HEADER_64_RULES: &[RandomHeader64] = &[
    RandomHeader64 {
        wz_version: 1206,
        header_len: 353,
        hash1_offsets: [0x4F, 0x4C, 0x15E, 0xE6, 0x155, 0x34, 0x51, 0x13F],
        hash2_offsets: [0xFF, 0x111, 0x98, 0x23, 0x1C, 0x39, 0xB0, 0x13D],
        data_size_offsets: [0x27, 0x32, 0x1E, 0x12F],
        masked: true,
    },
    RandomHeader64 {
        wz_version: 1205,
        header_len: 163,
        hash1_offsets: [0x84, 0x32, 0x43, 0x7F, 0x62, 0x01, 0x83, 0x27],
        hash2_offsets: [0xA2, 0x1B, 0x05, 0x0D, 0x9A, 0x85, 0x79, 0x4D],
        data_size_offsets: [0x08, 0x8F, 0x3F, 0x63],
        masked: false,
    },
    RandomHeader64 {
        wz_version: 1204,
        header_len: 150,
        hash1_offsets: [0x48, 0x24, 0x0F, 0x31, 0x46, 0x47, 0x63, 0x67],
        hash2_offsets: [0x8E, 0x8C, 0x93, 0x0E, 0x64, 0x7B, 0x2E, 0x4D],
        data_size_offsets: [0x12, 0x09, 0x02, 0x95],
        masked: false,
    },
    RandomHeader64 {
        wz_version: 1202,
        header_len: 200,
        hash1_offsets: [0x1E, 0x1A, 0x10, 0x01, 0x0F, 0x48, 0xC5, 0x99],
        hash2_offsets: [0x64, 0x6C, 0x25, 0x16, 0x0A, 0x03, 0xA2, 0xAA],
        data_size_offsets: [0x14, 0xB0, 0xB6, 0xB7],
        masked: false,
    },
];

/// 64비트 PKG2 random-header 파서 (`Wz_File.TryReadPkg2RandomHeader64`).
///
/// # 판별 원리
///
/// `dataSize` 를 흩어진 오프셋에서 모아 **마스크를 풀고** `fileSize - headerLen`
/// 과 비교한다. 일치하면 그 파일은 확실히 이 규칙의 헤더다.
///
/// # 함정: `dataSize` 비교가 진짜 판별 근거다
///
/// 해시는 검증 없이 그냥 읽힌다. 오직 `dataSize == fileSize - headerLen`
/// 이 통과를 결정한다. 그래서 `dataSize` 오프셋이 틀리면 아무 파일도 못 잡는다.
fn try_read_random_header64(
    src: &dyn ByteSource,
    len: u64,
    rule: &RandomHeader64,
) -> Option<(u32, u32, u64, u64, u32)> {
    if len < u64::from(rule.header_len) {
        return None;
    }
    let expected_data_size = len - u64::from(rule.header_len);
    // C# 은 `uint.MaxValue` 초과면 실패 (`Wz_File.cs:330`)
    if expected_data_size > u64::from(u32::MAX) {
        return None;
    }

    let mut buf = vec![0u8; rule.header_len as usize];
    src.try_read_exact(0, &mut buf).ok()?;

    let mut data_size = gather_u32(&buf, &rule.data_size_offsets);
    if rule.masked {
        data_size ^= gather_mask_u32(&rule.data_size_offsets);
    }
    if data_size != expected_data_size as u32 {
        return None;
    }

    let mut hash1 = gather_u64(&buf, &rule.hash1_offsets);
    let mut hash2 = gather_u64(&buf, &rule.hash2_offsets);
    if rule.masked {
        hash1 ^= gather_mask_u64(&rule.hash1_offsets);
        hash2 ^= gather_mask_u64(&rule.hash2_offsets);
    }

    Some((rule.wz_version, rule.header_len, hash1, hash2, data_size))
}

/// random-header PKG2 전체 감지 (규칙들 + 폴백).
///
/// `hash1/hash2` 가 0 이면 **폴백** 이라 해시 기반 오프셋 계산이 불가능하다.
fn detect_pkg2_random_header_all(
    src: &dyn ByteSource,
    len: u64,
) -> std::result::Result<Option<Detected>, DetectError> {
    for rule in RANDOM_HEADER_64_RULES {
        if let Some((wz, header_len, hash1, hash2, _)) = try_read_random_header64(src, len, rule) {
            return Ok(Some(Detected {
                profile: format!("PKG2_RANDOM_HEADER_KMST{wz}"),
                hash1: Some(hash1),
                hash2: Some(hash2),
                header_len,
                encver: None,
                capabilities: Capabilities {
                    pkg2_random_header64: true,
                    ..Default::default()
                },
            }));
        }
    }
    detect_pkg2_random_header(src, len)
}
fn detect_pkg2_random_header(
    src: &dyn ByteSource,
    len: u64,
) -> std::result::Result<Option<Detected>, DetectError> {
    const MARKER_OFFSET: u64 = 353;

    if len < MARKER_OFFSET + 1 {
        return Ok(None);
    }
    let mut b = [0u8; 1];
    src.try_read_exact(MARKER_OFFSET, &mut b)
        .map_err(|_| DetectError::TooShort {
            len,
            need: MARKER_OFFSET + 1,
        })?;
    if b[0] != 0x80 {
        return Ok(None);
    }

    Ok(Some(Detected {
        profile: "PKG2_RANDOM_HEADER".into(),
        // C# 은 해시를 0 으로 넘긴다 (`Wz_File.cs:237`).
        // 0 은 "없음" 이 아니라 "알 수 없음" 이므로 Some(0) 을 유지한다.
        hash1: Some(0),
        hash2: Some(0),
        // 함정: 353 이지 354 가 아니다.
        header_len: MARKER_OFFSET as u32,
        encver: None,
        capabilities: Capabilities {
            pkg2_random_header64: true,
            unknown_pkg2: true,
            ..Default::default()
        },
    }))
}

/// PKG1 헤더 (`Wz_File.cs:123-247`).
///
/// ```text
/// +0x00 char[4] "PKG1"
/// +0x04 i64     dataSize
/// +0x0C i32     headerSize      ← copyright 길이이자 데이터 시작 오프셋
/// +0x10 char[]  copyright       (headerSize - 16 바이트)
/// ```
///
/// 그 뒤(오프셋 `headerSize`)에 `u16 encver` 가 오지만, KMST1132 부터 제거되고
/// wzver 777 로 고정되었다 (`Wz_Crypto.cs`, `Wz_Header.cs:42-45`).
fn detect_pkg1(src: &dyn ByteSource) -> std::result::Result<Detected, DetectError> {
    let mut buf = [0u8; 16];
    src.try_read_exact(0, &mut buf)
        .map_err(|_| DetectError::TooShort { len: 16, need: 16 })?;

    let data_size = i64::from_le_bytes(buf[4..12].try_into().expect("8바이트"));
    let header_len = i32::from_le_bytes(buf[12..16].try_into().expect("4바이트"));

    if header_len < 16 {
        return Err(DetectError::Unsupported {
            profile: "PKG1".into(),
            detail: format!("headerSize {header_len} < 16"),
        });
    }

    // `encver` 확인: `headerSize` 위치의 u16.
    // 0x80 은 압축 int 개수의 첫 바이트일 수 있어 재해석이 필요하다
    // (`Wz_File.cs:187-202`).
    let encver_pos = header_len as u64;
    let mut two = [0u8; 2];
    let mut encver_missing = true;
    let mut encver_value: Option<u32> = None;
    if data_size >= 2 && src.try_read_exact(encver_pos, &mut two).is_ok() {
        let ev = u16::from_le_bytes(two);
        if ev > 0xFF {
            // KMST1132 이후: 필드 제거 → 값 없음
            encver_missing = true;
        } else if ev == 0x80 {
            // 압축 int 재해석 (`Wz_File.cs:187-202`)
            // 0 < 개수 <= 0xFFFF 이고 하위 바이트가 0 이면 실제 값이다.
            encver_missing = matches!(
                try_read_compressed_count(src, encver_pos),
                Some(n) if n > 0 && n <= 0xFFFF && (n & 0xFF) == 0
            );
            if !encver_missing {
                // 0x80 은 실제 encver 일 수도 있다. 그대로 보관한다.
                encver_value = Some(u32::from(ev));
            }
        } else {
            encver_missing = false;
            encver_value = Some(u32::from(ev));
        }
    }

    Ok(Detected {
        profile: if encver_missing {
            "PKG1_ENCVER_MISSING".into()
        } else {
            "PKG1".into()
        },
        hash1: None,
        hash2: None,
        header_len: header_len as u32,
        encver: encver_value,
        capabilities: Capabilities {
            encver_missing,
            ..Default::default()
        },
    })
}

/// `encver == 0x80` 일 때 압축 int 로 개수를 재해석 (`Wz_File.cs:187-202`).
fn try_read_compressed_count(src: &dyn ByteSource, pos: u64) -> Option<i64> {
    let mut b = [0u8; 1];
    src.try_read_exact(pos, &mut b).ok()?;
    let s = b[0] as i8;
    if s != -128 {
        return Some(s as i64);
    }
    let mut four = [0u8; 4];
    src.try_read_exact(pos + 1, &mut four).ok()?;
    Some(i32::from_le_bytes(four) as i64)
}

/// PKG2 헤더 (`Wz_File.cs:213-219`).
///
/// ```text
/// +0x00 char[4] "PKG2"
/// +0x04 i64     dataSize
/// +0x0C i32     headerSize
/// +0x10 char[]  copyright
/// +??  u32      hash1     ← copyright 직후
/// +??  u32      hash2
/// ```
///
/// **주의**: 랜덤 헤더 변형(KMST1201/1202/1204/1205/1206)은 copyright 길이가
/// 고정값(68/150/200/163/353)이 아니고, hash1/hash2/dataSize 를 **흩어진 바이트
/// 오프셋**에서 수집한다 (`Wz_File.cs:249-318`). 여기서는 표준 레이아웃만
/// 파싱하고, 변형 감지는 Phase 1.9 에서 추가한다.
fn detect_pkg2(src: &dyn ByteSource) -> std::result::Result<Detected, DetectError> {
    let mut head = [0u8; 16];
    src.try_read_exact(0, &mut head)
        .map_err(|_| DetectError::TooShort { len: 16, need: 16 })?;

    let header_len = i32::from_le_bytes(head[12..16].try_into().expect("4바이트"));
    if header_len < 16 {
        return Err(DetectError::Unsupported {
            profile: "PKG2".into(),
            detail: format!("headerSize {header_len} < 16"),
        });
    }

    // copyright 는 `headerSize - 16` 바이트.
    // 표준 레이아웃에서는 copyright 직후에 hash1/hash2 가 온다.
    // (랜덤 헤서 변형에서는 이 오프셋이 맞지 않으므로 Phase 1.9 에서
    //  고유한 시그니처로 구분한다.)
    let hashes_at = header_len as u64;
    if hashes_at + 8 > src.len().unwrap_or(0) {
        return Err(DetectError::Unsupported {
            profile: "PKG2".into(),
            detail: format!("hash 위치 {hashes_at} 가 파일 끝을 넘음"),
        });
    }

    let mut hb = [0u8; 8];
    src.try_read_exact(hashes_at, &mut hb)
        .map_err(|_| DetectError::TooShort { len: 16, need: 24 })?;

    Ok(Detected {
        profile: "PKG2_STANDARD_HEADER".into(),
        hash1: Some(u32::from_le_bytes(hb[0..4].try_into().expect("4")) as u64),
        hash2: Some(u32::from_le_bytes(hb[4..8].try_into().expect("4")) as u64),
        header_len: header_len as u32,
        encver: None,
        capabilities: Capabilities::default(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wz_pool::MemorySource;

    fn src(bytes: Vec<u8>) -> MemorySource {
        MemorySource::named(bytes, "test")
    }

    #[test]
    fn rejects_too_short() {
        assert!(matches!(
            detect(&src(vec![1, 2, 3])),
            Err(DetectError::TooShort { .. })
        ));
    }

    #[test]
    fn rejects_unknown_magic() {
        // 최소 길이(16) 는 넘어야 매직 판별에 도달한다
        let mut b = b"NOPE".to_vec();
        b.extend_from_slice(&[0u8; 20]);
        let e = detect(&src(b)).unwrap_err();
        match e {
            DetectError::UnknownMagic { magic, offset } => {
                assert_eq!(&magic, b"NOPE");
                assert_eq!(offset, 0);
            }
            other => panic!("예상치 못한 에러: {other:?}"),
        }
    }

    #[test]
    fn detects_pkg1_standard() {
        // headerSize = 16 → copyright 0 바이트
        let mut b = Vec::new();
        b.extend_from_slice(b"PKG1");
        b.extend_from_slice(&1000i64.to_le_bytes());
        b.extend_from_slice(&16i32.to_le_bytes());
        b.push(0x01); // encver
        b.push(0x00);
        b.extend_from_slice(&[0u8; 16]);

        let d = detect(&src(b)).unwrap();
        assert_eq!(d.profile, "PKG1");
        assert_eq!(d.header_len, 16);
        assert!(!d.capabilities.encver_missing);
        assert_eq!(d.hash1, None);
    }

    #[test]
    fn detects_pkg1_encver_missing() {
        // `encver` 가 **없다** 는 것은 headerSize 위치가 곧 파일 끝(데이터 시작)인
        // 경우다. 파일을 정확히 headerSize 에서 잘라서 만든다.
        let mut b = Vec::new();
        b.extend_from_slice(b"PKG1");
        b.extend_from_slice(&100i64.to_le_bytes());
        b.extend_from_slice(&16i32.to_le_bytes());
        // 여기서 끝 — offset 16 에서 2바이트를 읽을 수 없다

        assert_eq!(b.len(), 16);
        let d = detect(&src(b)).unwrap();
        assert_eq!(d.profile, "PKG1_ENCVER_MISSING");
        assert!(d.capabilities.encver_missing);
    }

    #[test]
    fn detects_pkg1_encver_missing_via_0x80_escape() {
        // 함정: encver 값이 0x80 (= -128) 이면 압축 int 로 재해석해야 한다.
        // 개수가 0x100 이면 실제 파일이고 → encver 없음 판정.
        let mut b = Vec::new();
        b.extend_from_slice(b"PKG1");
        b.extend_from_slice(&1000i64.to_le_bytes());
        b.extend_from_slice(&16i32.to_le_bytes());
        // encver = 0x80, 그 다음 i32 = 0x00000100 (256)
        b.push(0x80);
        b.extend_from_slice(&0x0000_0100i32.to_le_bytes());

        let d = detect(&src(b)).unwrap();
        assert!(
            d.capabilities.encver_missing,
            "0x80 + 개수 0x100 이면 encver 없는 것"
        );
    }

    #[test]
    fn pkg1_rejects_tiny_header() {
        let mut b = Vec::new();
        b.extend_from_slice(b"PKG1");
        b.extend_from_slice(&1000i64.to_le_bytes());
        b.extend_from_slice(&8i32.to_le_bytes());
        assert!(matches!(
            detect(&src(b)),
            Err(DetectError::Unsupported { .. })
        ));
    }

    #[test]
    fn detects_pkg2_standard() {
        let mut b = Vec::new();
        b.extend_from_slice(b"PKG2");
        b.extend_from_slice(&1000i64.to_le_bytes());
        b.extend_from_slice(&16i32.to_le_bytes()); // copyright 0 바이트
        b.extend_from_slice(&0x1122_3344u32.to_le_bytes()); // hash1
        b.extend_from_slice(&0x5566_7788u32.to_le_bytes()); // hash2
        b.extend_from_slice(&[0u8; 32]);

        let d = detect(&src(b)).unwrap();
        assert_eq!(d.profile, "PKG2_STANDARD_HEADER");
        assert_eq!(d.hash1, Some(0x1122_3344));
        assert_eq!(d.hash2, Some(0x5566_7788));
        assert_eq!(d.header_len, 16);
    }

    #[test]
    fn pkg2_rejects_hash_beyond_eof() {
        let mut b = Vec::new();
        b.extend_from_slice(b"PKG2");
        b.extend_from_slice(&1000i64.to_le_bytes());
        b.extend_from_slice(&1024i32.to_le_bytes()); // 파일보다 큰 headerSize
        assert!(matches!(
            detect(&src(b)),
            Err(DetectError::Unsupported { .. })
        ));
    }

    #[test]
    fn capability_bits_match_csharp() {
        // 원본 `Wz_Capabilities`: EncverMissing=1, Pkg2RandomHeader=2, Pkg2RandomHeader64=4
        let c = Capabilities {
            encver_missing: true,
            pkg2_random_header: false,
            pkg2_random_header64: false,
            ..Default::default()
        };
        assert_eq!(c.bits(), 1);

        let c = Capabilities {
            pkg2_random_header: true,
            ..Default::default()
        };
        assert_eq!(c.bits(), 2);

        let c = Capabilities {
            pkg2_random_header64: true,
            ..Default::default()
        };
        assert_eq!(c.bits(), 4);
    }

    #[test]
    fn errors_display_informatively() {
        // 미지원 포맷이어도 사용자가 원인을 알 수 있어야 한다 (리포트 §12 R3)
        let e = DetectError::Unsupported {
            profile: "PKG2_RANDOM_HEADER_1206".into(),
            detail: "offset 검증 실패: 구간 겹침".into(),
        };
        let s = e.to_string();
        assert!(s.contains("PKG2_RANDOM_HEADER_1206"), "{s}");
        assert!(s.contains("구간 겹침"), "{s}");

        let e2 = DetectError::UnknownMagic {
            magic: *b"XXXX",
            offset: 0,
        };
        assert!(
            e2.to_string().contains("PKG1"),
            "이미지 파일일 수 있음을 안내"
        );
    }

    #[test]
    fn detect_converts_to_wz_error() {
        let e: WzFormatError = DetectError::TooShort { len: 2, need: 16 }.into();
        assert!(e.to_string().contains("너무 짧음"));

        let e2: WzFormatError = DetectError::UnknownMagic {
            magic: *b"ZZZZ",
            offset: 0,
        }
        .into();
        assert!(matches!(e2, WzFormatError::UnknownMagic { .. }));
    }
}
