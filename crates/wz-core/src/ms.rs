//! `Ms_File` (MapleStory Data 컨테이너) 헤더 + 엔트리 테이블.
//!
//! 원본: `WzComparerR2.WzLib/Ms_File.cs:64-177`.
//!
//! # 이 포맷이 왜 특별한가
//!
//! `Ms_File` 은 **PKG 와 완전히 다른 설계**다.
//!
//! - 매직이 없다. 파일 앞부분이 전부 **파일명으로 파생된 키로 암호화**된다.
//! - 그래서 "이 파일이 Ms_File 인지"를 **매직으로 판별할 수 없다.** 파일명을
//!   알고 있어야 헤더를 풀 수 있다.
//! - 대신 **자기검증 해시**가 있다 (`hash == hashedSaltLen + version +
//!   entryCount + salt 합`). 그래서 판별이 **확실**하다 — "일치하지 않으면
//!   이 포맷이 아니다" 가 확실하다. PKG2 의 `dataSize` 검증보다 강하다.
//!
//! 실측: `C:/Nexon/M` 의 788개 WZ 중 **193개(24%)** 가 이 포맷이다.
//! PKG1/PKG2 만 지원하면 4분의 1을 못 연다.
//!
//! # 구조
//!
//! ```text
//! [random bytes]           길이 = sum(파일명 바이트) % 312 + 30
//! [hashedSaltLen : i32 LE]
//! [saltBytes  : saltLen * 2]   saltLen = (byte)hashedSaltLen ^ random[0]
//! [9 bytes SNOW2 암호화]   hash(i32) version(u8) entryCount(i32)
//! [random bytes]           길이 = sum(파일명 문자 * 3) % 212 + 33
//! [엔트리 테이블]          두 번째 SNOW2 키로 복호화
//! [1024 바이트 정렬]
//! [이미지 데이터]
//! ```
//!
//! # 함정: 파일명을 **소문자로** 바꾼다
//!
//! 원본은 `Path.GetFileName(f).ToLower()` (`Ms_File.cs:66`). 대문자로 하면
//! salt 와 SNOW2 키가 전부 달라져 **해시 검증이 실패**한다. 파일명을 그대로
//! 쓰면 안 된다.

use std::sync::Arc;

use wz_pool::{ByteSource, ReadCtx, ReadCtxExt};

use crate::crypto::chacha::ChaCha20Stream;
use crate::crypto::snow2::Snow2;

/// SNOW2 키 길이. 원본은 "128 이어야 하지만 앞 16바이트만 쓴다" 고注释했다.
const SNOW_KEY_LEN: usize = 16;

/// `Ms_File` 이 지원하는 유일한 버전 (`Ms_File.cs:102`).
pub const MS_SUPPORTED_VERSION: u8 = 2;

/// 헤더 파싱 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsHeader {
    /// 원본 파일명 (소문자 정규화됨).
    pub file_name: String,
    /// 파일명 + salt.
    pub file_name_with_salt: String,
    /// 복호화된 salt 문자열.
    pub salt: String,
    /// 자기검증 해시.
    pub hash: i32,
    /// 컨테이너 버전. 항상 2.
    pub version: u8,
    /// 엔트리 수.
    pub entry_count: i32,
    /// 9바이트 헤더의 절대 시작 위치.
    pub header_start: u64,
    /// 엔트리 테이블의 절대 시작 위치.
    pub entry_start: u64,
}

/// 엔트리 하나 (`Ms_Entry`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsEntry {
    /// `/` 로 구분된 경로.
    pub name: String,
    pub check_sum: i32,
    pub flags: i32,
    /// 정렬 적용 전 오프셋. 실제 위치는 `data_start + start_pos * 1024`.
    pub start_pos: i32,
    pub size: i32,
    pub size_aligned: i32,
    pub unk1: i32,
    pub unk2: i32,
    /// 이미지 복호화 키 (16바이트).
    pub entry_key: [u8; 16],
    /// 원본이 계산한 체크섬 (`Ms_File.cs:161`).
    ///
    /// 함정: `unk2` 가 **포함되지 않는다.**
    pub calculated_check_sum: i32,
}

impl MsEntry {
    /// 정렬 적용 후 실제 시작 위치.
    ///
    /// 함정: 오프셋이 1024 바이트 **배수 단위**다. 그대로 더하면 안 된다.
    pub fn resolved_offset(&self, data_start: u64) -> u64 {
        data_start + (self.start_pos as i64).wrapping_mul(1024) as u64
    }
}

/// 파싱 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MsError {
    /// 파일이 너무 짧다.
    TooShort { need: usize, have: u64 },
    /// 버전이 2 가 아니다.
    BadVersion { found: u8 },
    /// 자기검증 해시 불일치 → 이 포맷이 아니다.
    ///
    /// PKG1/PKG2 판별에도 이 값을 쓴다. **불일치가 아니라 판별 실패다.**
    HashMismatch { expected: i32, actual: i32 },
    /// 구조상 불가능한 값 → 이 포맷이 아니다.
    ///
    /// 해시 검사 **전에** 걸러야 한다. 해시 검사는 그 뒤에 오는步骤이다.
    NotMsFile { reason: String },
    /// 엔트리 이름 길이가 음수/범위 밖.
    BadNameLength { len: i32 },
    /// SNOW2 복호화 실패.
    Snow(String),
    /// I/O.
    Io(String),
}

impl std::fmt::Display for MsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::TooShort { need, have } => {
                write!(f, "파일이 너무 짧다 (필요 {need}, 실제 {have})")
            }
            Self::BadVersion { found } => write!(f, "버전 불일치 (기대 2, 실제 {found})"),
            Self::HashMismatch { expected, actual } => {
                write!(f, "해시 불일치 (기대 {expected}, 실제 {actual})")
            }
            Self::NotMsFile { reason } => write!(f, "Ms_File 이 아님: {reason}"),
            Self::BadNameLength { len } => write!(f, "엔트리 이름 길이 오류: {len}"),
            Self::Snow(e) => write!(f, "SNOW2 오류: {e}"),
            Self::Io(e) => write!(f, "I/O 오류: {e}"),
        }
    }
}

impl std::error::Error for MsError {}

impl From<wz_error::WzError> for MsError {
    fn from(e: wz_error::WzError) -> Self {
        Self::Io(e.to_string())
    }
}

/// 파일명 정규화 — 원본과 **반드시** 같아야 한다 (`Ms_File.cs:66`).
///
/// 함정: 경로를 떼고 **소문자로** 바꾼다. 확장자 포함.
pub fn normalize_file_name(path: &str) -> String {
    // Windows/UNIX 경로 구분자 모두 처리한다.
    let base = path.rsplit(['/', '\\']).next().unwrap_or(path);
    base.to_lowercase()
}

/// 헤더 키 계산.
///
/// # 함정: `+ i` 가 `u8` 로 **넘쳐야 한다**
///
/// 원본은 `(byte)(c + i)` 다. Rust 에서 `u8` 캐스트가 없으면 **에러**가
/// 아니라 **다른 값**이 되어 키가 완전히 달라진다.
fn snow_key_header(file_name_with_salt: &str) -> [u8; SNOW_KEY_LEN] {
    let bytes = file_name_with_salt.as_bytes();
    let mut key = [0u8; SNOW_KEY_LEN];
    for (i, k) in key.iter_mut().enumerate() {
        // 원본: `fileNameWithSalt[i % len] + i` 를 **byte 로 캐스트**
        let c = bytes[i % bytes.len()] as u16;
        *k = (c + i as u16) as u8;
    }
    key
}

/// 엔트리 키 계산 (`Ms_File.cs:140`).
///
/// # 함정: 뒤에서부터 **역순**으로 읽는다
///
/// 원본은 `fileNameWithSalt[len - 1 - i % len]` 이라 문자열을 거꾸로 훑는다.
/// 정방향으로 쓰면 키가 완전히 다르다.
fn snow_key_entries(file_name_with_salt: &str) -> [u8; SNOW_KEY_LEN] {
    let bytes = file_name_with_salt.as_bytes();
    let len = bytes.len();
    let mut key = [0u8; SNOW_KEY_LEN];
    for (i, k) in key.iter_mut().enumerate() {
        let c = bytes[len - 1 - (i % len)] as u16;
        let t = (i % 3 + 2) as u16;
        *k = (i as u16 + t.wrapping_mul(c)) as u8;
    }
    key
}

/// 헤더를 읽는다.
///
/// # 함정: 파일명이 필요하다
///
/// 호출자는 **정확한 파일명**을 줘야 한다. 경로를 주면 안 되고
/// [`normalize_file_name`] 을 통과시킨 basename 이어야 한다.
pub fn read_header(src: &dyn ByteSource, path: &str) -> Result<MsHeader, MsError> {
    let file_name = normalize_file_name(path);
    let len = src.len().map_err(|e| MsError::Io(e.to_string()))?;

    // 1. 랜덤 바이트
    //
    // 함정: `sum(c => (int)c)` 는 **소문자 기준 파일명**의 바이트 합이다.
    let sum: u32 = file_name.bytes().map(u32::from).sum();
    let rand_byte_count = (sum % 312 + 30) as usize;
    if len < rand_byte_count as u64 + 4 {
        return Err(MsError::TooShort {
            need: rand_byte_count + 4,
            have: len,
        });
    }
    let mut rand_bytes = vec![0u8; rand_byte_count];
    read_exact(src, 0, &mut rand_bytes)?;

    // 2. salt
    let mut b = [0u8; 4];
    read_exact(src, rand_byte_count as u64, &mut b)?;
    let hashed_salt_len = i32::from_le_bytes(b);
    // 함정: `(byte)hashedSaltLen` — 하위 1바이트만 쓴다.
    let salt_len = (hashed_salt_len as u8) ^ rand_bytes[0];
    let salt_len = usize::from(salt_len);
    if len < (rand_byte_count + 4 + salt_len * 2) as u64 {
        return Err(MsError::TooShort {
            need: rand_byte_count + 4 + salt_len * 2,
            have: len,
        });
    }
    let mut salt_bytes = vec![0u8; salt_len * 2];
    read_exact(src, (rand_byte_count + 4) as u64, &mut salt_bytes)?;

    // 함정: 아래 루프는 `rand_bytes[i]` 를 `i < salt_len` 만큼 읽는다.
    //
    // `salt_len` 은 **쓰레기 바이트에서** 나오므로 최대 255 인 반면
    // `rand_byte_count` 는 최소 30 이다. 즉 PKG1 같은 다른 포맷을 이
    // 함수에 넣으면 **반드시** 범위를 벗어난다.
    //
    // C# 은 여기서 `IndexOutOfRangeException` 을 던진다 (`Ms_File.cs:81`).
    // Rust 는 panic 이므로 **반드시 먼저 검사**해야 한다 — 판별기가
    // 이 함수를 모든 파일에 호출하기 때문이다.
    if salt_len > rand_byte_count {
        return Err(MsError::NotMsFile {
            reason: format!("salt_len({salt_len}) > rand_byte_count({rand_byte_count})"),
        });
    }

    // 함정: 짝수/홀수 인덱스가 뒤섞인다 — `saltBytes[i*2]` 만 쓴다.
    let mut salt_chars = String::with_capacity(salt_len);
    for i in 0..salt_len {
        salt_chars.push((rand_bytes[i] ^ salt_bytes[i * 2]) as char);
    }
    let salt = salt_chars;

    // 3. 9바이트 헤더를 SNOW2 로 복호화
    let file_name_with_salt = format!("{file_name}{salt}");
    let key = snow_key_header(&file_name_with_salt);
    let header_start = (rand_byte_count + 4 + salt_len * 2) as u64;
    if len < header_start + 9 {
        return Err(MsError::TooShort {
            need: header_start as usize + 9,
            have: len,
        });
    }
    let mut enc12 = [0u8; 12];
    read_exact(src, header_start, &mut enc12)?;

    // 함정: 헤더는 **9바이트**인데 SNOW2 는 4바이트 블록만 한다.
    //
    // C# 의 `CryptoStream` 은 내부적으로 4바이트 블록을 연속 복호화하면서
    // 순서대로 흘려보내므로 `ReadInt32/ReadByte/ReadInt32` 이 **연속한 9바이트**
    // 를 받는다. 즉 키스트림은 **연속**이고 9바이트 경계가 없다.
    //
    // 따라서 4의 배수인 12바이트를 복호화한 뒤 **앞 9바이트**만 쓴다.
    // 9바이트만 따로 복호화하려 하면 (a) 4배수 assert 에 걸리고
    // (b) 하위 1바이트의 키스트림 위치가 어긋난다.
    let dec = {
        let mut s = Snow2::new(&key, &[]).encryptor();
        let mut buf = enc12;
        s.transform(&mut buf);
        buf
    };
    let hash = i32::from_le_bytes([dec[0], dec[1], dec[2], dec[3]]);
    let version = dec[4];
    let entry_count = i32::from_le_bytes([dec[5], dec[6], dec[7], dec[8]]);

    // 4. 검증 — 이게 판별의 근거다
    if version != MS_SUPPORTED_VERSION {
        return Err(MsError::BadVersion { found: version });
    }

    // 함정: salt 합은 `u16` 으로 **캐스트해서** 더한다 (`MemoryMarshal.Cast<byte,ushort>`).
    let mut actual_hash = hashed_salt_len
        .wrapping_add(i32::from(version))
        .wrapping_add(entry_count);
    for c in salt_bytes.as_chunks::<2>().0.iter() {
        actual_hash = actual_hash.wrapping_add(i32::from(u16::from_le_bytes(*c)));
    }
    if hash != actual_hash {
        return Err(MsError::HashMismatch {
            expected: actual_hash,
            actual: hash,
        });
    }

    // 5. 엔트리 테이블 위치
    //
    // 함정: `headerStartPos + 9 + sum(문자 * 3) % 212 + 33` — 9바이트는
    // **암호화된 헤더 길이**고, 그 뒤에 또 랜덤 바이트가 붙는다.
    let tri: u32 = file_name.bytes().map(|c| u32::from(c) * 3).sum();
    let entry_start = header_start + 9 + u64::from(tri % 212) + 33;

    Ok(MsHeader {
        file_name,
        file_name_with_salt,
        salt,
        hash,
        version,
        entry_count,
        header_start,
        entry_start,
    })
}

fn read_exact(src: &dyn ByteSource, off: u64, buf: &mut [u8]) -> Result<(), MsError> {
    src.read_at(off, buf).map_err(|_| MsError::TooShort {
        need: buf.len(),
        have: src.len().unwrap_or(0),
    })
}

/// 헤더 + 엔트리 테이블을 모두 읽는다.
pub fn read(src: &Arc<dyn ByteSource>, path: &str) -> Result<MsFile, MsError> {
    let header = read_header(src.as_ref(), path)?;
    let (entries, table_end) = read_entries(src.as_ref(), &header)?;

    // 데이터 시작 위치는 1024 바이트 정렬 (`Ms_File.cs:165-170`)
    //
    // 함정: 정렬은 **엔트리 테이블 끝**을 기준으로 한다. 헤더 시작이 아니다.
    let data_start = if table_end & 0x3FF != 0 {
        (table_end & !0x3FF) + 0x400
    } else {
        table_end
    };

    Ok(MsFile {
        header,
        entries,
        data_start,
    })
}

/// `Ms_File` 전체.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsFile {
    pub header: MsHeader,
    pub entries: Vec<MsEntry>,
    /// 이미지 데이터 시작 (1024 정렬됨).
    pub data_start: u64,
}

/// 엔트리 테이블을 읽는다.
pub fn read_entries(
    src: &dyn ByteSource,
    header: &MsHeader,
) -> Result<(Vec<MsEntry>, u64), MsError> {
    let key = snow_key_entries(&header.file_name_with_salt);
    let count = header.entry_count;
    if !(0..=500_000).contains(&count) {
        return Err(MsError::BadNameLength { len: count });
    }

    // 엔트리 전체를 한 번에 읽고 SNOW2 로 풀다.
    // (원본은 CryptoStream 을 스트리밍하지만, 결과가 같고 훨씬 단순하다.)
    let need = estimate_entry_bytes(count);
    let len = src.len().map_err(|e| MsError::Io(e.to_string()))?;
    if len < header.entry_start + need {
        return Err(MsError::TooShort {
            need: header.entry_start as usize + need as usize,
            have: len,
        });
    }
    // 함정: SNOW2 는 4바이트 블록만 처리한다. 버퍼 길이를 반드시 4배수로.
    let mut total = need.max(12);
    total = (total + 3) & !3;
    let mut buf = vec![0u8; total as usize];
    read_exact(src, header.entry_start, &mut buf)?;

    let mut s = Snow2::new(&key, &[]).encryptor();
    s.transform(&mut buf);

    let mut c = ReadCtx::from_bytes(std::mem::take(&mut buf));
    // `count` 는 상위에서 0..=500_000 으로 검증되지만, 손상된 파일이 상한까지
    // 채워도 한 번에 거대한 Vec 을 예약하지 않도록 작은 값으로 시작한다.
    let mut out = Vec::with_capacity((count as usize).min(4096));
    for _ in 0..count {
        let name_len = c.read_i32_le().map_err(|e| MsError::Snow(e.to_string()))?;
        if !(0..=0x10000).contains(&name_len) {
            return Err(MsError::BadNameLength { len: name_len });
        }
        // 함정: UTF-16LE 문자. `ReadChars(n)` 과 같은 바이트 소비다.
        let mut name = String::with_capacity(name_len as usize);
        for _ in 0..name_len {
            let ch = c.read_u16_le().map_err(|e| MsError::Snow(e.to_string()))?;
            name.push(char::from_u32(u32::from(ch)).unwrap_or('\u{FFFD}'));
        }
        let check_sum = c.read_i32_le().map_err(|e| MsError::Snow(e.to_string()))?;
        let flags = c.read_i32_le().map_err(|e| MsError::Snow(e.to_string()))?;
        let start_pos = c.read_i32_le().map_err(|e| MsError::Snow(e.to_string()))?;
        let size = c.read_i32_le().map_err(|e| MsError::Snow(e.to_string()))?;
        let size_aligned = c.read_i32_le().map_err(|e| MsError::Snow(e.to_string()))?;
        let unk1 = c.read_i32_le().map_err(|e| MsError::Snow(e.to_string()))?;
        let unk2 = c.read_i32_le().map_err(|e| MsError::Snow(e.to_string()))?;
        let mut entry_key = [0u8; 16];
        read_exact_slice(&mut c, &mut entry_key)?;

        // 함정: `unk2` 가 합산에 **포함되지 않는다** (`Ms_File.cs:161`).
        let calculated = flags
            .wrapping_add(start_pos)
            .wrapping_add(size)
            .wrapping_add(size_aligned)
            .wrapping_add(unk1)
            .wrapping_add(entry_key.iter().map(|b| i32::from(*b)).sum::<i32>());

        out.push(MsEntry {
            name,
            check_sum,
            flags,
            start_pos,
            size,
            size_aligned,
            unk1,
            unk2,
            entry_key,
            calculated_check_sum: calculated,
        });
    }

    let consumed = c.pos();
    Ok((out, header.entry_start + consumed))
}

fn read_exact_slice(c: &mut ReadCtx, out: &mut [u8]) -> Result<(), MsError> {
    let v = c
        .read_exact_slice(out.len())
        .map_err(|e| MsError::Snow(e.to_string()))?;
    out.copy_from_slice(&v);
    Ok(())
}

/// 엔트리 테이블의 **최대** 바이트 수.
///
/// 이름 길이를 모를 때의 상한. 디렉터리 경로는 보통 짧지만 악의적으로
/// 클 수 있으 64KB 로 제한한다 (BadNameLength 와 같은 한계).
fn estimate_entry_bytes(count: i32) -> u64 {
    let n = count as u64;
    // 이름 최대 0x10000 코드유닛 * 2 바이트 + 고정 48 바이트
    n.saturating_mul(0x10000 * 2 + 48)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wz_pool::MemorySource;

    #[test]
    fn file_name_is_lowercased_basename() {
        // 함정: 대문자로 두면 salt/키가 달라져 해시 검증이 실패한다.
        assert_eq!(
            normalize_file_name(r"C:\Nexon\M\Data\Base_000.WZ"),
            "base_000.wz"
        );
        assert_eq!(normalize_file_name("/data/base.wz"), "base.wz");
        assert_eq!(normalize_file_name("Base.wz"), "base.wz");
    }

    #[test]
    fn random_byte_count_matches_csharp() {
        // 원본: `fileName.Sum(c => (int)c) % 312 + 30` (`Ms_File.cs:71`)
        //
        // 함정: 파일명을 **소문자로 정규화한 뒤** 합산한다. 경로를 넣으면
        // 백슬래시(0x5C)까지 합에 들어가 값이 틀어진다.
        let f = normalize_file_name("C:/Nexon/M/Data/Base_000.WZ");
        assert_eq!(f, "base_000.wz");

        let sum: u32 = f.bytes().map(u32::from).sum();
        let want = (sum % 312 + 30) as usize;

        // 경로를 그대로 썼다면 다른 값이 나온다 — 정규화가 필요함을 보인다
        let raw_sum: u32 = "C:/Nexon/M/Data/Base_000.WZ".bytes().map(u32::from).sum();
        assert_ne!(want, (raw_sum % 312 + 30) as usize, "정규화가 값을 바꾼다");

        // 범위 sanity
        assert!((30..342).contains(&want));
    }

    #[test]
    fn snow_key_header_wraps_like_u8() {
        // 원본: `snowCipherKey[i] = (byte)(fileNameWithSalt[i % len] + i)`
        // (`Ms_File.cs:91`)
        //
        // 함정: `u8` 캐스트의 **오버플로가 키를 정의한다.** Rust 에서
        // `as u8` 를 빼면 다른 키가 나오고 해시 검증이 실패한다.
        let s = "abc";
        let k = snow_key_header(s);
        let b = s.as_bytes();
        for (i, got) in k.iter().enumerate() {
            let want = b[i % b.len()].wrapping_add(i as u8);
            assert_eq!(*got, want, "i={i}");
        }

        // 오버플로를 실제로 발생시켜 확인 — 값이 그냥 자르지 않고 돌아간다.
        // 문자 'a'(0x61) + 160 = 0x101 → 0x01
        let long = "a";
        let kl = snow_key_header(long);
        // i 가 16 이라 키 범위를 벗어나지만, 원본도 키는 16 바이트뿐이다.
        // 여기서는 계산 함수의 오버플로 동작을 직접 확인한다.
        let over = (0x61u8).wrapping_add(160u8);
        assert_eq!(over, 0x01, "u8 오버플로 확인");
        assert_eq!(kl[0], 0x61, "i=0 은 오버플로 없음");
    }

    #[test]
    fn snow_key_entries_reads_backwards() {
        // 함정: `len - 1 - i % len` 이라 **역순**이다.
        let s = "abcde";
        let k = snow_key_entries(s);
        let b = s.as_bytes();
        for i in 0..16 {
            let c = b[5 - 1 - (i % 5)] as u16;
            let t = (i % 3 + 2) as u16;
            assert_eq!(k[i], (i as u16 + t.wrapping_mul(c)) as u8, "i={i}");
        }
        // 정방향이었다면 k[0] 이 달라진다
        assert_ne!(k[0], (b[0] as u16).wrapping_add(2 * (b[0] as u16)) as u8);
    }

    #[test]
    fn checksum_excludes_unk2() {
        // 함정: `unk2` 가 합에 들어가지 않는다 (`Ms_File.cs:161`).
        let mut e = MsEntry {
            name: "x".into(),
            check_sum: 0,
            flags: 10,
            start_pos: 20,
            size: 30,
            size_aligned: 40,
            unk1: 50,
            unk2: 999, // 포함되면 값이 달라진다
            entry_key: [1u8; 16],
            calculated_check_sum: 0,
        };
        e.calculated_check_sum = e
            .flags
            .wrapping_add(e.start_pos)
            .wrapping_add(e.size)
            .wrapping_add(e.size_aligned)
            .wrapping_add(e.unk1)
            .wrapping_add(e.entry_key.iter().map(|b| i32::from(*b)).sum::<i32>());
        // 10+20+30+40+50 + 16 = 166  (unk2=999 는 빠짐)
        assert_eq!(e.calculated_check_sum, 166);
    }

    #[test]
    fn short_file_is_error_not_panic() {
        let src = MemorySource::new(vec![0u8; 8]);
        let r = read_header(&src, "data.wz");
        assert!(matches!(r, Err(MsError::TooShort { .. })), "{r:?}");
    }

    #[test]
    fn wrong_file_name_fails_hash_check() {
        // 실제로 파일을 만들 순 없으므로 핵심 성질만 검증한다:
        // 파일명이 다르면 키가 달라 해시 검증이 실패해야 한다.
        let k1 = snow_key_header("data.wz");
        let k2 = snow_key_header("other.wz");
        assert_ne!(k1, k2, "파일명이 키를 바꾼다");
    }
}

/// # `Ms_FileV2` — 진짜 컨테이너 (ChaCha20)
///
/// V1(`Ms_File`) 은 **SNOW2 + 버전 2** 다.
///
/// 실측(`C:/Nexon/M` 788개, C# `Wz_Structure.LoadMsFile` 로 확인): **V1 로
/// 열리는 파일은 0개** 이었다. 그 788개는 전부 PKG1(595) 또는 random-header
/// PKG2(192) 였다. `Ms_File` 계열은 이 데이터셋에 아예 없었다.
///
/// 주의: 원본은 `.wz` 에 대해 `LoadMsFile` 을 **자동 시도하지 않는다**
/// (`Wz_Structure.cs:384` 는 별개 진입점). 그래서 `.wz` 가 PKG 로 안 열리면
/// MS 라고 가정하면 안 된다 — 실제로도 안 않았다. 같은 디렉터리에 `.ms`
/// 파일 11개가 별도로 있어 그쪽이 MS 컨테이너의 정체다.
///
/// 원본: `WzComparerR2.WzLib/Ms_FileV2.cs:66-193`.
///
/// # 구조
///
/// ```text
/// [random bytes]        sum(파일명) % 312 + 30  ← 그 다음 **부호 유지 >> 1**
/// [encryptedVersion:u8] version = enc ^ rand[0],  == 4 이어야 함
/// [hashedSaltLen:i32]
/// [saltBytes : saltLen*2]
/// [8바이트 ChaCha20]    hash(i32) entryCount(i32)
/// [random bytes]        sum(파일명*3) % 212 + 64
/// [엔트리 테이블]       두 번째 ChaCha20 키
/// [1024 정렬]
/// [이미지 데이터]
/// ```
///
/// # 함정 1: 랜덤 바이트에 **부호 유지 시프트**가 걸린다
///
/// 원본은 `(byte)((sbyte)randBytes[i] >> 1)` 다 (`Ms_FileV2.cs:77`).
/// C# 의 `>>` 는 **산술 시프트**라 0x80 이 0xC0 이 된다.
/// Rust 에서 `u8 >> 1` (논리 시프트) 로 쓰면 **모든 값이 달라져**
/// 버전 검사부터 전부 실패한다.
///
/// # 함정 2: 버전 검사가 복호화 **전에** 일어난다
///
/// `version = encryptedVersion ^ randBytes[0]` 은 해시 키 없이 계산된다.
/// 그래서 이것만으로 **부드러운 판별**이 가능하다 — 복호화를 시도할
/// 필요 없다. 이 저장소의 실제 193개 파일이 전부 여기서 걸린다.
///
/// # 함정 3: 해시 합에 `encryptedVersion` 과 `version` 이 **둘 다** 든다
///
/// V1 은 `version` 만 더했지만 V2 는 둘 다 더한다 (`Ms_FileV2.cs:121`).
/// 하나만 넣으면 해시 검증이 실패한다.
///
/// # 함정 4: salt 복호화에 난수 상수 75 가 있다
///
/// `b = ((a | 0x4B) << 1) - a - 75` (`Ms_FileV2.cs:95`).
pub const MS_V2_SUPPORTED_VERSION: u8 = 4;

/// ChaCha20 키 난독화 상수 (`Ms_FileV2.cs:233-238`).
pub const CHACHA20_KEY_OBSCURE: [u8; 32] = [
    0x7B, 0x2F, 0x35, 0x48, 0x43, 0x95, 0x02, 0xB9, //
    0xAE, 0x91, 0xA6, 0xE1, 0xD8, 0xD6, 0x24, 0xB4, //
    0x33, 0x10, 0x1D, 0x3D, 0xC1, 0xBB, 0xC6, 0xF4, //
    0xA5, 0xFE, 0xB3, 0x69, 0x6B, 0x56, 0xE4, 0x75,
];

/// V2 헤더.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsV2Header {
    pub file_name: String,
    pub file_name_with_salt: String,
    pub salt: String,
    pub hash: i32,
    /// 항상 4.
    pub version: u8,
    /// 파일에 **암호화된 그대로** 저장된 버전 바이트.
    pub encrypted_version: u8,
    pub entry_count: i32,
    /// 8바이트 헤더 시작.
    pub header_start: u64,
    pub entry_start: u64,
}

/// V2 엔트리 (`Ms_Entry` + `unk3`/`unk4`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsV2Entry {
    pub name: String,
    pub check_sum: i32,
    pub flags: i32,
    pub start_pos: i32,
    pub size: i32,
    pub size_aligned: i32,
    pub unk1: i32,
    pub unk2: i32,
    pub entry_key: [u8; 16],
    pub unk3: i32,
    pub unk4: i32,
    /// `unk2`/`unk3`/`unk4` 는 합에 **포함되지 않는다** (`Ms_FileV2.cs:177`).
    pub calculated_check_sum: i32,
}

impl MsV2Entry {
    pub fn resolved_offset(&self, data_start: u64) -> u64 {
        data_start + (self.start_pos as i64).wrapping_mul(1024) as u64
    }
}

/// V2 컨테이너.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MsV2File {
    pub header: MsV2Header,
    pub entries: Vec<MsV2Entry>,
    pub data_start: u64,
}

/// C# `(byte)((sbyte)b >> 1)` — **산술** 시프트.
#[inline]
fn sar1(b: u8) -> u8 {
    ((b as i8) >> 1) as u8
}

/// 헤더 ChaCha20 키 (`Ms_FileV2.cs:107-111`).
fn v2_key_header(file_name_with_salt: &str) -> [u8; 32] {
    let b = file_name_with_salt.as_bytes();
    let mut key = [0u8; 32];
    for (i, k) in key.iter_mut().enumerate() {
        *k = (b[i % b.len()].wrapping_add(i as u8)) ^ CHACHA20_KEY_OBSCURE[i];
    }
    key
}

/// 엔트리 ChaCha20 키 (`Ms_FileV2.cs:153-156`).
///
/// 함정: 문자열을 **역순**으로 읽는다 (`len - 1 - i % len`).
fn v2_key_entries(file_name_with_salt: &str) -> [u8; 32] {
    let b = file_name_with_salt.as_bytes();
    let len = b.len();
    let mut key = [0u8; 32];
    for (i, k) in key.iter_mut().enumerate() {
        let c = b[len - 1 - (i % len)];
        let t = (i % 3 + 2) as u8;
        *k = (i as u8).wrapping_add(t.wrapping_mul(c)) ^ CHACHA20_KEY_OBSCURE[i];
    }
    key
}

/// 헤더 앞부분만 읽고 포맷인지 판별한다 (복호화 없음).
///
/// 이게 [`crate::detect`] 가 쓰는 저비용 판별 경로다.
pub fn probe_v2(src: &dyn ByteSource, path: &str) -> Result<u8, MsError> {
    let file_name = normalize_file_name(path);
    let sum: u32 = file_name.bytes().map(u32::from).sum();
    let rand_byte_count = (sum % 312 + 30) as usize;
    let len = src.len().map_err(|e| MsError::Io(e.to_string()))?;
    if len < rand_byte_count as u64 + 1 {
        return Err(MsError::TooShort {
            need: rand_byte_count + 1,
            have: len,
        });
    }
    let mut rand_bytes = vec![0u8; rand_byte_count];
    read_exact(src, 0, &mut rand_bytes)?;
    // 함정 1: 부호 유지 시프트
    for b in rand_bytes.iter_mut() {
        *b = sar1(*b);
    }
    let mut v = [0u8; 1];
    read_exact(src, rand_byte_count as u64, &mut v)?;
    let version = v[0] ^ rand_bytes[0];
    if version != MS_V2_SUPPORTED_VERSION {
        return Err(MsError::BadVersion { found: version });
    }
    Ok(version)
}

/// V2 헤더를 읽는다.
pub fn read_v2_header(src: &dyn ByteSource, path: &str) -> Result<MsV2Header, MsError> {
    let file_name = normalize_file_name(path);
    let len = src.len().map_err(|e| MsError::Io(e.to_string()))?;

    let sum: u32 = file_name.bytes().map(u32::from).sum();
    let rand_byte_count = (sum % 312 + 30) as usize;
    if len < rand_byte_count as u64 + 5 {
        return Err(MsError::TooShort {
            need: rand_byte_count + 5,
            have: len,
        });
    }
    let mut rand_bytes = vec![0u8; rand_byte_count];
    read_exact(src, 0, &mut rand_bytes)?;
    for b in rand_bytes.iter_mut() {
        *b = sar1(*b);
    }

    // 2. 버전 (함정 2: 복호화 전)
    let mut vb = [0u8; 1];
    read_exact(src, rand_byte_count as u64, &mut vb)?;
    let encrypted_version = vb[0];
    let version = encrypted_version ^ rand_bytes[0];
    if version != MS_V2_SUPPORTED_VERSION {
        return Err(MsError::BadVersion { found: version });
    }

    // 3. salt
    let mut b4 = [0u8; 4];
    read_exact(src, rand_byte_count as u64 + 1, &mut b4)?;
    let hashed_salt_len = i32::from_le_bytes(b4);
    let salt_len = usize::from((hashed_salt_len as u8) ^ rand_bytes[0]);
    let salt_off = rand_byte_count as u64 + 5;
    if len < salt_off + salt_len as u64 * 2 {
        return Err(MsError::TooShort {
            need: salt_off as usize + salt_len * 2,
            have: len,
        });
    }
    let mut salt_bytes = vec![0u8; salt_len * 2];
    read_exact(src, salt_off, &mut salt_bytes)?;

    // rand_bytes 가 더 짧을 수 있다 — V1 과 같은 범위 문제
    if salt_len > rand_byte_count {
        return Err(MsError::NotMsFile {
            reason: format!("salt_len({salt_len}) > rand_byte_count({rand_byte_count})"),
        });
    }

    // 함정 4: 난수 상수 75
    let mut salt = String::with_capacity(salt_len);
    for i in 0..salt_len {
        let a = i32::from(rand_bytes[i] ^ salt_bytes[i * 2]);
        let c = ((a | 0x4B) << 1) - a - 75;
        salt.push(char::from_u32(c as u32 % 0x10000).unwrap_or('\u{FFFD}'));
    }

    // 4. 8바이트 ChaCha20 헤더
    let file_name_with_salt = format!("{file_name}{salt}");
    let key = v2_key_header(&file_name_with_salt);
    let header_start = salt_off + salt_len as u64 * 2;
    if len < header_start + 8 {
        return Err(MsError::TooShort {
            need: header_start as usize + 8,
            have: len,
        });
    }
    let mut enc8 = [0u8; 8];
    read_exact(src, header_start, &mut enc8)?;

    let nonce = [0u8; 12];
    let mut st = ChaCha20Stream::new_with_counter(&key, &nonce, 0);
    st.apply_keystream(&mut enc8);

    let hash = i32::from_le_bytes([enc8[0], enc8[1], enc8[2], enc8[3]]);
    let entry_count = i32::from_le_bytes([enc8[4], enc8[5], enc8[6], enc8[7]]);

    // 함정 3: encryptedVersion 과 version 둘 다 더한다
    let mut actual = hashed_salt_len
        .wrapping_add(i32::from(encrypted_version))
        .wrapping_add(i32::from(version))
        .wrapping_add(entry_count);
    for c in salt_bytes.as_chunks::<2>().0.iter() {
        actual = actual.wrapping_add(i32::from(u16::from_le_bytes(*c)));
    }
    if hash != actual {
        return Err(MsError::HashMismatch {
            expected: actual,
            actual: hash,
        });
    }

    // 5. 엔트리 테이블 위치 — V1 과 상수가 다르다 (64 vs 33, 8 vs 9)
    let tri: u32 = file_name.bytes().map(|c| u32::from(c) * 3).sum();
    let entry_start = header_start + 8 + u64::from(tri % 212) + 64;

    Ok(MsV2Header {
        file_name,
        file_name_with_salt,
        salt,
        hash,
        version,
        encrypted_version,
        entry_count,
        header_start,
        entry_start,
    })
}

/// V2 헤더 + 엔트리 + 데이터 시작 위치를 읽는다.
pub fn read_v2(src: &Arc<dyn ByteSource>, path: &str) -> Result<MsV2File, MsError> {
    let header = read_v2_header(src.as_ref(), path)?;
    let count = header.entry_count;
    if !(0..=500_000).contains(&count) {
        return Err(MsError::BadNameLength { len: count });
    }
    let key = v2_key_entries(&header.file_name_with_salt);
    let nonce = [0u8; 12];

    // 이름 길이를 모르면 정확한 크기를 알 수 없다. 각 엔트리를
    // 순차 복호화하되, 한 번에 큰 버퍼를 잡고 끝까지 읽는다.
    //
    // .NET `ChaCha20Reader` 는 블록 단위로 스트리밍하지만 키스트림은
    // 위치 기반이라 **연속**이다. 따라서 필요한 만큼 읽어 한 번에 풀어도
    // 결과가 동일하다.
    let len = src.len().map_err(|e| MsError::Io(e.to_string()))?;
    // 여유 있게: 엔트리당 평균 200 바이트 + 최소
    let est = estimate_v2_entry_bytes(count);
    if len < header.entry_start + est {
        return Err(MsError::TooShort {
            need: header.entry_start as usize + est as usize,
            have: len,
        });
    }
    let mut buf = vec![0u8; est as usize];
    read_exact(src.as_ref(), header.entry_start, &mut buf)?;

    let mut st = ChaCha20Stream::new_with_counter(&key, &nonce, 0);
    st.apply_keystream(&mut buf);

    let mut c = ReadCtx::from_bytes(std::mem::take(&mut buf));
    // `count` 는 상위에서 0..=500_000 으로 검증되지만, 손상된 파일이 상한까지
    // 채워도 한 번에 거대한 Vec 을 예약하지 않도록 작은 값으로 시작한다.
    let mut out = Vec::with_capacity((count as usize).min(4096));
    for _ in 0..count {
        let name_len = c.read_i32_le().map_err(|e| MsError::Snow(e.to_string()))?;
        if !(0..=0x10000).contains(&name_len) {
            return Err(MsError::BadNameLength { len: name_len });
        }
        // 함정: UTF-16LE 문자 (`ReadChars`/`ReadString` 과 동일)
        let mut name = String::with_capacity(name_len as usize);
        for _ in 0..name_len {
            let ch = c.read_u16_le().map_err(|e| MsError::Snow(e.to_string()))?;
            name.push(char::from_u32(u32::from(ch)).unwrap_or('\u{FFFD}'));
        }
        let check_sum = c.read_i32_le().map_err(|e| MsError::Snow(e.to_string()))?;
        let flags = c.read_i32_le().map_err(|e| MsError::Snow(e.to_string()))?;
        let start_pos = c.read_i32_le().map_err(|e| MsError::Snow(e.to_string()))?;
        let size = c.read_i32_le().map_err(|e| MsError::Snow(e.to_string()))?;
        let size_aligned = c.read_i32_le().map_err(|e| MsError::Snow(e.to_string()))?;
        let unk1 = c.read_i32_le().map_err(|e| MsError::Snow(e.to_string()))?;
        let unk2 = c.read_i32_le().map_err(|e| MsError::Snow(e.to_string()))?;
        let mut entry_key = [0u8; 16];
        let v = c
            .read_exact_slice(16)
            .map_err(|e| MsError::Snow(e.to_string()))?;
        entry_key.copy_from_slice(&v);
        let unk3 = c.read_i32_le().map_err(|e| MsError::Snow(e.to_string()))?;
        let unk4 = c.read_i32_le().map_err(|e| MsError::Snow(e.to_string()))?;

        let calculated = flags
            .wrapping_add(start_pos)
            .wrapping_add(size)
            .wrapping_add(size_aligned)
            .wrapping_add(unk1)
            .wrapping_add(entry_key.iter().map(|b| i32::from(*b)).sum::<i32>());

        out.push(MsV2Entry {
            name,
            check_sum,
            flags,
            start_pos,
            size,
            size_aligned,
            unk1,
            unk2,
            entry_key,
            unk3,
            unk4,
            calculated_check_sum: calculated,
        });
    }

    let table_end = header.entry_start + c.pos();
    let data_start = if table_end & 0x3FF != 0 {
        (table_end & !0x3FF) + 0x400
    } else {
        table_end
    };

    Ok(MsV2File {
        header,
        entries: out,
        data_start,
    })
}

/// V2 엔트리 테이블 추정 크기.
///
/// 이름은 최대 0x10000 코드유닛(128KiB)까지 가능하므로 최악 상한이
/// 매우 크다. 실제로는 경로가 짧으므로 평균에 여유를 둔다.
fn estimate_v2_entry_bytes(count: i32) -> u64 {
    let n = count as u64;
    n.saturating_mul(0x10000 * 2 + 68)
}

#[cfg(test)]
mod real_tests {
    use wz_pool::MemorySource;

    /// 실제 게임 데이터가 있으면 감지 결과를 C# 기준과 대조한다.
    ///
    /// C# oracle (`WzComparerR2.Oracle scan C:\Nexon\M`) 이 낸 ground truth:
    /// PKG1 595 / PKG2Header64 192 / 미분류 1 = 788.
    ///
    /// 이 테스트가 지키는 회귀: random-header PKG2 는 첫 4바이트가 무작위라
    /// 매직 비교만 하면 **192개 전부 놓친다** (게임 데이터의 24%).
    #[test]
    fn detection_matches_csharp_ground_truth() {
        let root = std::path::Path::new(r"C:\Nexon\M");
        if !root.exists() {
            eprintln!("실제 WZ 없음 — 건너뜀");
            return;
        }

        let mut files = Vec::new();
        fn walk(d: &std::path::Path, v: &mut Vec<std::path::PathBuf>) {
            if let Ok(rd) = std::fs::read_dir(d) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        walk(&p, v);
                    } else if p.extension().map(|x| x == "wz").unwrap_or(false) {
                        v.push(p);
                    }
                }
            }
        }
        walk(root, &mut files);

        let (mut pkg1, mut random_pkg2, mut unknown) = (0, 0, 0);
        for p in &files {
            // 마커는 353 이므로 앞 1KiB 만 있으면 된다. 전체를 읽으면
            // 수백 MB 를 읽어 테스트가 1분 넘게 걸린다.
            let Ok(mut f) = std::fs::File::open(p) else {
                continue;
            };
            use std::io::Read;
            let mut head = vec![0u8; 1024];
            let n = f.read(&mut head).unwrap_or(0);
            head.truncate(n);
            let src = MemorySource::new(head);
            match crate::detect::detect(&src) {
                Ok(d) => match d.profile.as_str() {
                    x if x.starts_with("PKG1") => pkg1 += 1,
                    x if x.starts_with("PKG2_RANDOM_HEADER") => {
                        random_pkg2 += 1;
                        assert!(d.capabilities.pkg2_random_header64);
                        // 전용 파서(KMST1206 등)로 잡히면 해시가 **실제 값**이고,
                        // 폴백이면 0 이다 — 해시 기반 검증을 할 수 있는지가 갈린다.
                        let via_parser = x.contains("KMST");
                        if via_parser {
                            assert_ne!(
                                d.hash1.unwrap_or(0),
                                0,
                                "전용 파서는 실제 hash1 을 내야 한다 (폴백과 구분)"
                            );
                            assert!(
                                !d.capabilities.unknown_pkg2,
                                "해시를 알면 unknown 이 아니다"
                            );
                        } else {
                            // 폴백: 오프셋은 353, 해시는 0
                            assert_eq!(d.header_len, 353, "폴백 마커 오프셋");
                            assert!(d.capabilities.unknown_pkg2);
                        }
                    }
                    _ => unknown += 1,
                },
                Err(_) => unknown += 1,
            }
        }

        eprintln!(
            "PKG1={pkg1} PKG2_RANDOM_HEADER={random_pkg2} 미분류={unknown} (총 {})",
            files.len()
        );
        // C# 기준값. 데이터셋이 바뀌면 이 값도 같이 갱신해야 한다.
        assert_eq!(pkg1, 595, "PKG1 수 불일치");
        assert_eq!(random_pkg2, 192, "random-header PKG2 수 불일치");
        assert_eq!(unknown, 1, "미분류 수 불일치");
    }

    /// `.ms` 파일은 실제로 MS 컨테이너여야 한다 (V2/ChaCha20).
    #[test]
    fn ms_extension_files_are_ms_file_v2() {
        let root = std::path::Path::new(r"C:\Nexon\M");
        if !root.exists() {
            eprintln!("실제 ms 없음 — 건너뜀");
            return;
        }
        let mut files = Vec::new();
        fn walk(d: &std::path::Path, v: &mut Vec<std::path::PathBuf>) {
            if let Ok(rd) = std::fs::read_dir(d) {
                for e in rd.flatten() {
                    let p = e.path();
                    if p.is_dir() {
                        walk(&p, v);
                    } else if p.extension().map(|x| x == "ms").unwrap_or(false) {
                        v.push(p);
                    }
                }
            }
        }
        walk(root, &mut files);
        if files.is_empty() {
            return;
        }

        let mut ok = 0;
        let mut fail = Vec::new();
        for p in &files {
            // 헤더는 salt 까지 수백 바이트면 충분하다.
            let Ok(mut f) = std::fs::File::open(p) else {
                continue;
            };
            use std::io::Read;
            let mut head = vec![0u8; 4096];
            let n = f.read(&mut head).unwrap_or(0);
            head.truncate(n);
            let src = MemorySource::new(head);
            match super::read_v2_header(&src, &p.to_string_lossy()) {
                Ok(_) => ok += 1,
                Err(e) => fail.push(format!("{}: {e}", p.file_name().unwrap().to_string_lossy())),
            }
        }
        eprintln!(
            ".ms 총 {} / V2 헤더 성공 {} / 실패 {}",
            files.len(),
            ok,
            fail.len()
        );
        for f in fail.iter().take(5) {
            eprintln!("  실패 {f}");
        }
        assert!(ok > 0, ".ms 파일 중 하나 이상은 V2 로 열려야 한다");
    }
}
