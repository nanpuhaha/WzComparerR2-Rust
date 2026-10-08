//! WZ 문자열 디코딩 (원본 `WzComparerR2.WzLib/WzBinaryReader.cs:98-216`,
//! `Utilities/MathHelper.cs:90-215`).
//!
//! # 3가지 인코딩
//!
//! 모두 **XOR 복호화 후** 해석한다. 접두 1바이트의 부호가 인코딩을 고른다.
//!
//! | 접두 | 의미 | 페이로드 |
//! |---|---|---|
//! | `len < 0` | ASCII / cp1252 | `n = (len == -128) ? i32_le : -len` 바이트 |
//! | `len > 0` | UTF-16LE | `n = (len == 127) ? i32_le : len` 코드유닛 |
//! | `len == 0` | 빈 문자열 | 없음 |
//!
//! # XOR 마스크 (self-inverse → encode == decode)
//!
//! ```text
//! ASCII:  b[i]   ^= (byte)(0xAA + i)
//! UTF-16: u16[i] ^= (u16)(0xAAAA + i)
//! ```
//!
//! 인덱스는 **파일 절대 오프셋** 이 아니라 **문자열 시작 위치 기준** 이다.
//! 여기서도 동일하게 유지한다 (리포트 §14.4).
//!
//! # PKG2 전용 변형
//!
//! - `ReadPkg2DirString` (`:153-183`): 항상 UTF-16, 길이는 **i8**
//! - `ReadPkg2DirStringV2` (`:186-216`): 항상 UTF-16, 길이는 **i16** (KMST1202+)

use wz_error::{Result, WzCryptoError};
use wz_pool::{ReadCtx, ReadCtxExt};

use crate::crypto::Decryptor;

/// 문자열 인코딩 종류 (디코딩 후 판별).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrEncoding {
    /// 8비트 (원본은 cp1252 인코딩으로 가정)
    Ascii,
    /// UTF-16LE
    Utf16,
}

/// 디코딩 결과.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WzString {
    pub text: String,
    pub encoding: StrEncoding,
}

/// 바이트 배열을 Latin-1 로 해석한다.
///
/// 원본은 cp1252 를 쓰지만 실제 WZ 문자열은 cp1252 와 Latin-1 이 겹치는 범위에
/// 머문다. `String::from_utf8_lossiness` 로 깨뜨리지 않고 **보존**하기 위해
/// Latin-1 → UTF-16 변환을 한다 (1바이트 → 1 유니코드포인트).
///
/// 원본이 cp1252 전용 영역(0x80-0x9F)을 쓰면 여기서 차이가 난다. 그런 경우
/// [`decode_wz_string_bytes_codepage`] 를 쓰거나 `FromStr` 로 변환한다.
fn decode_8bit(raw: &[u8]) -> String {
    raw.iter().map(|&b| b as char).collect()
}

/// `Encoding.Unicode` (UTF-16LE) 를 흉내낸다.
///
/// 홀수 길이이거나 짝미합이 잘못되면 cp1252 로 되돌아간다 (원본 `ReadString` 의
/// `try_read_chars` 폴백).
fn decode_utf16le(raw: &[u8]) -> String {
    if !raw.len().is_multiple_of(2) {
        // 원본과 동일한 폴백: 바이트 단위로 해석
        return decode_8bit(raw);
    }
    let units: Vec<u16> = raw
        .as_chunks::<2>()
        .0
        .iter()
        .map(|c| u16::from_le_bytes(*c))
        .collect();
    match String::from_utf16(&units) {
        Ok(s) => s,
        Err(_) => decode_8bit(raw),
    }
}

/// ASCII(cp1252)용 XOR 마스크를 적용/해제한다.
pub fn apply_ascii_mask(buf: &mut [u8]) {
    for (i, b) in buf.iter_mut().enumerate() {
        *b ^= (0xAAu32.wrapping_add(i as u32) & 0xFF) as u8;
    }
}

/// UTF-16LE용 XOR 마스크를 적용/해제한다.
pub fn apply_utf16_mask(buf: &mut [u8]) {
    // `chunks_exact_mut` 와 동일하게 **완전한 2바이트 블록만** 변환하고
    // 홀수 꼬리 1바이트는 건드리지 않는다 (원본 `Transform` 과 같은 계약).
    for (i, chunk) in buf.as_chunks_mut::<2>().0.iter_mut().enumerate() {
        let v = u16::from_le_bytes([chunk[0], chunk[1]]);
        let mask = (0xAAAAu32.wrapping_add(i as u32) & 0xFFFF) as u16;
        chunk.copy_from_slice(&(v ^ mask).to_le_bytes());
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 이미지 프로퍼티 문자열 (BMS/KMS/GMS 키 사용)
// ─────────────────────────────────────────────────────────────────────────────

/// `WzBinaryReader.ReadImageString` — 암호 키로 XOR 된 3-인코딩 문자열.
///
/// `decryptor` 가 `None` 이면 마스킹 없는 것으로 본다 (BMS no-op 경로).
/// `None` 을 넘겨도 동일하게 동작해야 하므로 `NoOpCrypto` 를 쓰는 것이 안전하다.
pub fn read_image_string(c: &mut ReadCtx, decryptor: Option<&dyn Decryptor>) -> Result<WzString> {
    let len = c.read_i8()?;

    if len == 0 {
        return Ok(WzString {
            text: String::new(),
            encoding: StrEncoding::Utf16,
        });
    }

    if len > 0 {
        // UTF-16LE
        let n = if len == 127 {
            let v = c.read_i32_le()?;
            if v < 0 {
                return Err(WzCryptoError::StringDecode {
                    reason: format!("UTF-16 길이 {v} < 0"),
                }
                .into());
            }
            v as usize
        } else {
            len as usize
        };
        let mut raw = c.read_exact_slice(n * 2)?;
        if let Some(d) = decryptor {
            d.decrypt_at(&mut raw, 0);
        }
        apply_utf16_mask(&mut raw);
        Ok(WzString {
            text: decode_utf16le(&raw),
            encoding: StrEncoding::Utf16,
        })
    } else {
        // 8비트 (cp1252)
        let n = if len == -128 {
            let v = c.read_i32_le()?;
            if v < 0 {
                return Err(WzCryptoError::StringDecode {
                    reason: format!("ASCII 길이 {v} < 0"),
                }
                .into());
            }
            v as usize
        } else {
            (-(len as i32)) as usize
        };
        let mut raw = c.read_exact_slice(n)?;
        if let Some(d) = decryptor {
            d.decrypt_at(&mut raw, 0);
        }
        apply_ascii_mask(&mut raw);
        Ok(WzString {
            text: decode_8bit(&raw),
            encoding: StrEncoding::Ascii,
        })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PKG2 디렉터리 문자열
// ─────────────────────────────────────────────────────────────────────────────

/// `ReadPkg2DirString` — 항상 UTF-16LE, **i8** 길이 (`:153-183`)
pub fn read_pkg2_dir_string(
    c: &mut ReadCtx,
    decryptor: Option<&dyn Decryptor>,
) -> Result<WzString> {
    let len = c.read_i8()?;
    let n = len.max(0) as usize;
    let mut raw = c.read_exact_slice(n * 2)?;
    if let Some(d) = decryptor {
        d.decrypt_at(&mut raw, 0);
    }
    apply_utf16_mask(&mut raw);
    Ok(WzString {
        text: decode_utf16le(&raw),
        encoding: StrEncoding::Utf16,
    })
}

/// `ReadPkg2DirStringV2` — 항상 UTF-16LE, **i16** 길이 (KMST1202+, `:186-216`)
pub fn read_pkg2_dir_string_v2(
    c: &mut ReadCtx,
    decryptor: Option<&dyn Decryptor>,
) -> Result<WzString> {
    let len = c.read_i16_le()?;
    let n = len.max(0) as usize;
    let mut raw = c.read_exact_slice(n * 2)?;
    if let Some(d) = decryptor {
        d.decrypt_at(&mut raw, 0);
    }
    apply_utf16_mask(&mut raw);
    Ok(WzString {
        text: decode_utf16le(&raw),
        encoding: StrEncoding::Utf16,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// 인코딩 (테스트 WZ 빌더용)
// ─────────────────────────────────────────────────────────────────────────────

/// `read_image_string` 의 역연산 (마스킹 포함, 암호화 없음).
pub fn encode_ascii_wz_string(s: &str) -> Vec<u8> {
    let mut masked: Vec<u8> = s.chars().map(|c| c as u32 as u8).collect();
    let raw_len = masked.len();
    apply_ascii_mask(&mut masked);
    if raw_len <= 127 {
        let mut out = vec![(-(raw_len as i8)) as u8];
        out.extend_from_slice(&masked);
        out
    } else {
        let mut out = vec![0x80u8];
        out.extend_from_slice(&(raw_len as i32).to_le_bytes());
        out.extend_from_slice(&masked);
        out
    }
}

/// `read_image_string` 의 역연산 (UTF-16LE, 마스킹 포함).
///
/// # 함정 1: 접두 폭이 **가변**이다
///
/// | UTF-16 코드유닛 수 | 접두 |
/// |---|---|
/// | `< 127` | 1바이트 |
/// | `>= 127` | `0x7F` + `i32` = **5바이트** |
///
/// 암호화는 **페이로드에서만** 한다 (`WzBinaryReader.cs:112`). 접두가 가변이므로
/// 페이로드 시작 오프셋도 달라진다 — [`image_string_header_len`] 을 쓸 것.
///
/// # 함정 2: 경계값이 **127** 이다 (128 이 아니다)
///
/// 원본 리더는 `size == 127` 이면 이스케이프로 본다
/// (`WzBinaryReader.cs:126-129`). 즉 정확히 127 코드유닛인 문자열을
/// `[127, ...]` 로 직접 인코딩하면 **자체 모순**이 되어 리더가 뒤에 붙은
/// 4바이트를 길이로 착각한다. 그래서 경계는 `127` **이상**이어야 한다.
///
/// (ASCII 경로는 이스케이프 마커가 `-128` 이라 `127` 과 겹치지 않아 안전하다.)
pub fn encode_utf16_wz_string(s: &str) -> Vec<u8> {
    let units: Vec<u16> = s.encode_utf16().collect();
    let mut masked = Vec::with_capacity(units.len() * 2);
    for (i, u) in units.iter().enumerate() {
        let mask = (0xAAAAu32.wrapping_add(i as u32) & 0xFFFF) as u16;
        masked.extend_from_slice(&(u ^ mask).to_le_bytes());
    }
    if units.len() < WZ_UTF16_ESCAPE_AT {
        let mut out = vec![units.len() as u8];
        out.extend_from_slice(&masked);
        out
    } else {
        let mut out = vec![WZ_UTF16_ESCAPE_MARKER];
        out.extend_from_slice(&(units.len() as i32).to_le_bytes());
        out.extend_from_slice(&masked);
        out
    }
}

/// UTF-16 WZ 문자열의 이스케이프 마커 (`0x7F`).
pub const WZ_UTF16_ESCAPE_MARKER: u8 = 127;

/// UTF-16 WZ 문자열이 이스케이프 접두를 써야 하는 최소 코드유닛 수.
///
/// `127` **이상**. 원본 리더(`WzBinaryReader.cs:126`)가 `size == 127` 을
/// 이스케이프로 해석하기 때문에 `127` 을 직접 길이로 쓸 수 없다.
pub const WZ_UTF16_ESCAPE_AT: usize = 127;

/// `encode_utf16_wz_string` 결과에서 **암호화/복호화를 적용할 페이로드 시작 오프셋**.
///
/// # 왜 이게 함정인가
///
/// 짧은 문자열은 접두가 1바이트라 항상 `1` 이라고착각하기 쉽다. 하지만 127
/// 코드유닛 이상이면 접두가 5바이트가 되고, 그 안에 든 `i32` **길이까지
/// 암호화**하면 파서가 엉뚱한 길이를 읽는다 (관측: `len = 177392326`).
/// 이 오프셋을 1로 고정하면 **127자 넘는 문자열에서만** 깨진다.
pub fn image_string_header_len(s: &str) -> usize {
    if s.encode_utf16().count() < WZ_UTF16_ESCAPE_AT {
        1
    } else {
        5
    }
}

/// 빈 문자열 인코딩.
pub fn encode_empty() -> Vec<u8> {
    vec![0]
}

/// PKG2 디렉터리 문자열 (i8 길이) 인코딩.
pub fn encode_pkg2_dir_string(s: &str) -> Vec<u8> {
    let units: Vec<u16> = s.encode_utf16().collect();
    let mut masked = Vec::with_capacity(units.len() * 2);
    for (i, u) in units.iter().enumerate() {
        let mask = (0xAAAAu32.wrapping_add(i as u32) & 0xFFFF) as u16;
        masked.extend_from_slice(&(u ^ mask).to_le_bytes());
    }
    let mut out = vec![units.len() as i8 as u8];
    out.extend_from_slice(&masked);
    out
}

/// PKG2 디렉터리 문자열 (i16 길이) 인코딩.
pub fn encode_pkg2_dir_string_v2(s: &str) -> Vec<u8> {
    let units: Vec<u16> = s.encode_utf16().collect();
    let mut masked = Vec::with_capacity(units.len() * 2);
    for (i, u) in units.iter().enumerate() {
        let mask = (0xAAAAu32.wrapping_add(i as u32) & 0xFFFF) as u16;
        masked.extend_from_slice(&(u ^ mask).to_le_bytes());
    }
    let mut out = Vec::with_capacity(2 + masked.len());
    out.extend_from_slice(&(units.len() as i16).to_le_bytes());
    out.extend_from_slice(&masked);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::aes::{AesKeyStream, NoOpCrypto};
    use crate::crypto::Decryptor;
    fn ctx(bytes: Vec<u8>) -> ReadCtx {
        ReadCtx::from_bytes(bytes)
    }

    // ── 마스크 자체 ──────────────────────────────────────────────────────────

    #[test]
    fn ascii_mask_matches_reference_formula() {
        let mut buf = vec![0u8; 8];
        apply_ascii_mask(&mut buf);
        for (i, b) in buf.iter().enumerate() {
            assert_eq!(
                *b,
                (0xAAu32.wrapping_add(i as u32) & 0xFF) as u8,
                "인덱스 {i}"
            );
        }
    }

    #[test]
    fn utf16_mask_matches_reference_formula() {
        let mut buf = vec![0u8; 8]; // 4 유닛
        apply_utf16_mask(&mut buf);
        for i in 0..4 {
            let got = u16::from_le_bytes([buf[i * 2], buf[i * 2 + 1]]);
            assert_eq!(
                got,
                (0xAAAAu32.wrapping_add(i as u32) & 0xFFFF) as u16,
                "유닛 {i}"
            );
        }
    }

    #[test]
    fn masks_are_self_inverse() {
        let orig: Vec<u8> = b"Hello".to_vec();
        let mut buf = orig.clone();
        apply_ascii_mask(&mut buf);
        assert_ne!(buf, orig);
        apply_ascii_mask(&mut buf);
        assert_eq!(buf, orig, "마스크는 self-inverse");

        let mut buf2 = vec![0x41, 0x00, 0x42, 0x00];
        let orig2 = buf2.clone();
        apply_utf16_mask(&mut buf2);
        assert_ne!(buf2, orig2);
        apply_utf16_mask(&mut buf2);
        assert_eq!(buf2, orig2);
    }

    // ── 왕복 ────────────────────────────────────────────────────────────────

    #[test]
    fn ascii_roundtrip() {
        // 주의: 8비트 경로는 cp1252/Latin-1 이므로 비라틴 문자를 담을 수 없다.
        // 원본도 마찬가지다 — 한국어 문자열은 반드시 UTF-16 경로를 탄다.
        for s in ["", "a", "Hello", "abc/img.png", "Émile", "1234-5678"] {
            let enc = encode_ascii_wz_string(s);
            let mut c = ctx(enc);
            let got = read_image_string(&mut c, None).unwrap();
            assert_eq!(got.text, s, "ASCII 왕복 실패: {s:?}");
        }
    }

    #[test]
    fn ascii_path_truncates_non_latin1_like_csharp() {
        // 원본 `char as byte` 와 동일하게 하위 8비트만 남는다.
        let enc = encode_ascii_wz_string("가나다");
        let mut c = ctx(enc);
        let got = read_image_string(&mut c, None).unwrap();
        assert_ne!(
            got.text, "가나다",
            "cp1252 경로로는 한국어가 보존되지 않는다"
        );
        assert_eq!(got.encoding, StrEncoding::Ascii);
    }

    #[test]
    fn utf16_roundtrip() {
        for s in [
            "",
            "a",
            "Hello",
            "00002000.img",
            "다국어 문자열 테스트",
            "🎮 게임 데이터",
        ] {
            let enc = encode_utf16_wz_string(s);
            let mut c = ctx(enc);
            let got = read_image_string(&mut c, None).unwrap();
            assert_eq!(got.text, s, "UTF-16 왕복 실패: {s:?}");
        }
    }

    #[test]
    fn encoding_is_detected_from_sign() {
        let mut c = ctx(encode_ascii_wz_string("abc"));
        assert_eq!(
            read_image_string(&mut c, None).unwrap().encoding,
            StrEncoding::Ascii
        );

        let mut c = ctx(encode_utf16_wz_string("abc"));
        assert_eq!(
            read_image_string(&mut c, None).unwrap().encoding,
            StrEncoding::Utf16
        );

        let mut c = ctx(encode_empty());
        let got = read_image_string(&mut c, None).unwrap();
        assert_eq!(got.text, "");
        assert_eq!(
            got.encoding,
            StrEncoding::Utf16,
            "빈 문자열은 Utf16 로 분류"
        );
    }

    #[test]
    fn long_string_uses_escape_prefix() {
        // 127 유닛 이하는 1바이트 길이, 그 위는 127 + i32
        let s127 = "a".repeat(127);
        let s128 = "a".repeat(128);
        let e127 = encode_utf16_wz_string(&s127);
        let e128 = encode_utf16_wz_string(&s128);
        assert_eq!(e127[0], 127, "127 은 1바이트 길이");
        assert_eq!(e128[0], 127, "128 은 127 + i32 이스케이프");
        assert_eq!(i32::from_le_bytes(e128[1..5].try_into().unwrap()), 128);

        let mut c = ctx(e128);
        assert_eq!(read_image_string(&mut c, None).unwrap().text, s128);
    }

    #[test]
    fn empty_payload_consumes_only_prefix() {
        let mut c = ctx(vec![0]);
        let _ = read_image_string(&mut c, None).unwrap();
        assert_eq!(c.pos(), 1);
    }

    #[test]
    fn surrogate_pairs_survive_utf16_roundtrip() {
        // 이모티콘은 UTF-16 서로게이트 페어다. Latin-1 폴백에 빠지지 않아야 한다.
        let s = "🎮👾";
        let enc = encode_utf16_wz_string(s);
        let mut c = ctx(enc);
        assert_eq!(read_image_string(&mut c, None).unwrap().text, s);
    }

    #[test]
    fn odd_length_utf16_falls_back_like_csharp() {
        // 원본은 짝미합이 실패하면 cp1252 폴백한다. 바이트 0x00 도 문자로 남는다.
        let decoded = decode_utf16le(&[0x41, 0x00, 0x42]);
        assert_eq!(decoded, "A\u{0}B", "홀수 길이는 바이트 단위 폴백");
    }

    // ── 암호화 결합 ────────────────────────────────────────────────────────
    //
    // 중요: 원본 `WzBinaryReader.ReadString` 은 **길이 접두사만 평문**으로 읽고
    // (`WzBinaryReader.cs:102` `this.ReadSByte()`), 페이로드만 복호화한다
    // (`:112` ASCII, `:135` UTF-16). 그리고 `Decrypt(span)` 은 항상
    // `keyOffset = 0` 이므로 페이로드는 **키스트림 맨 앞**에서 시작한다.
    //
    // 따라서 "접두사는 그대로, 페이지만 암호화"가 정확한 재현이다.

    /// 인코딩된 문자열의 접두사 길이.
    fn prefix_len(enc: &[u8]) -> usize {
        match enc[0] {
            127 => 5,  // UTF-16 이스케이프: 127 + i32
            0x80 => 5, // ASCII 이스케이프: -128 + i32
            _ => 1,    // 직접 길이
        }
    }

    /// 페이로드만 키스트림으로 암호화한다 (원본의 암호화 방향 = 같은 XOR).
    fn encrypt_payload_only(enc: &mut [u8], ks: &AesKeyStream) {
        let p = prefix_len(enc);
        ks.decrypt_at(&mut enc[p..], 0);
    }

    #[test]
    fn works_with_kms_decryptor() {
        let plain = "KMS로 암호화된 문자열";
        let mut enc = encode_utf16_wz_string(plain);
        encrypt_payload_only(&mut enc, &AesKeyStream::kms());

        let mut c = ctx(enc);
        let got = read_image_string(&mut c, Some(&AesKeyStream::kms())).unwrap();
        assert_eq!(got.text, plain, "KMS 키스트림과 XOR 로 복호화돼야 한다");
    }

    /// 회귀 테스트: 127 코드유닛 **이상**이면 접두가 **5바이트**가 된다.
    ///
    /// # 왜 회귀인가
    ///
    /// 두 가지가 겹친다.
    ///
    /// 1. `image_string_header_len` 을 1로 고정하면 127자 넘는 문자열에서
    ///    접두 안의 `i32` **길이까지** 암호화된다 → 파서가
    ///    `len = 177392326` 을 읽고 OutOfBounds 로 죽는다.
    /// 2. 정확히 127 코드유닛을 `[127, ...]` 로 직접 인코딩하면 **자체 모순**이다.
    ///    원본 리더(`WzBinaryReader.cs:126`)는 `size == 127` 을 이스케이프로
    ///    해석하므로 뒤의 4바이트를 길이로 착각한다.
    ///
    /// 127 이하에서는 우연히 맞아서 **작은 문자열 테스트로는 둘 다 못 잡는다.**
    #[test]
    fn header_len_is_five_bytes_from_127_units() {
        for units in [0usize, 1, 126] {
            let s = "가".repeat(units); // 한 글 = 1 코드유닛
            let enc = encode_utf16_wz_string(&s);
            assert_eq!(
                image_string_header_len(&s),
                prefix_len(&enc),
                "코드유닛 {units} 에서 접두 폭 불일치"
            );
            assert_eq!(enc[0] as usize, units, "127 미만은 직접 길이");
        }
        for units in [127usize, 128, 129, 200, 1000] {
            let s = "가".repeat(units);
            let enc = encode_utf16_wz_string(&s);
            assert_eq!(
                image_string_header_len(&s),
                5,
                "코드유닛 {units} 는 5바이트 접두여야 한다"
            );
            assert_eq!(enc[0], WZ_UTF16_ESCAPE_MARKER, "이스케이프 마커");
            assert_eq!(
                i32::from_le_bytes(enc[1..5].try_into().expect("4바이트")),
                units as i32,
                "접두의 i32 이 코드유닛 수"
            );
        }
    }

    /// 경계값(126/127/128)을 포함해 페이로드만 암호화하면 왕복해야 한다.
    #[test]
    fn long_string_roundtrips_with_encryption() {
        let ks = AesKeyStream::kms();
        for units in [126usize, 127, 128, 300] {
            let plain: String = (0..units)
                .map(|i| char::from(b'a' + (i % 26) as u8))
                .collect();
            let mut enc = encode_utf16_wz_string(&plain);
            let hdr = image_string_header_len(&plain);
            assert_eq!(hdr, prefix_len(&enc), "코드유닛 {units}");
            ks.decrypt_at(&mut enc[hdr..], 0);

            let mut c = ctx(enc);
            let got = read_image_string(&mut c, Some(&ks)).unwrap();
            assert_eq!(got.text, plain, "코드유닛 {units} 왕복");
            assert_eq!(c.pos(), c.len().expect("len"), "남은 바이트 0");
        }
    }

    #[test]
    fn payload_decryption_starts_at_keystream_zero() {
        // 원본 `Decrypt(span)` 은 keyOffset 0. 페이로드 첫 바이트가
        // 키스트림 0 과 XOR 되어야 한다.
        let plain = "ABCDEFGH";
        let original = encode_utf16_wz_string(plain);
        let p = prefix_len(&original);
        let ks_bytes = AesKeyStream::kms().keystream_prefix(original.len() - p);

        let mut enc = original.clone();
        encrypt_payload_only(&mut enc, &AesKeyStream::kms());
        for i in 0..(enc.len() - p) {
            assert_eq!(
                enc[p + i],
                original[p + i] ^ ks_bytes[i],
                "페이로드 바이트 {i}"
            );
        }
    }

    #[test]
    fn noop_decryptor_is_identity() {
        // 8비트 경로에 라틴 문자만 넣어야 한다 (cp1252 한계).
        let plain = "BMS no encryption path";
        let enc = encode_ascii_wz_string(plain);
        let mut c = ctx(enc);
        assert_eq!(
            read_image_string(&mut c, Some(&NoOpCrypto)).unwrap().text,
            plain
        );
    }

    #[test]
    fn wrong_key_produces_garbage_not_panic() {
        let plain = "올바른 키";
        let mut enc = encode_utf16_wz_string(plain);
        encrypt_payload_only(&mut enc, &AesKeyStream::kms());
        let mut c = ctx(enc);
        let got = read_image_string(&mut c, Some(&AesKeyStream::gms())).unwrap();
        assert_ne!(got.text, plain, "잘못된 키면 다른 문자열");
    }

    // ── PKG2 변형 ───────────────────────────────────────────────────────────

    #[test]
    fn pkg2_dir_string_roundtrip_i8_len() {
        for s in ["", "Map", "00002000.img", "Korean 이름"] {
            let enc = encode_pkg2_dir_string(s);
            let mut c = ctx(enc);
            let got = read_pkg2_dir_string(&mut c, None).unwrap();
            assert_eq!(got.text, s, "PKG2(i8) 왕복 실패: {s:?}");
        }
    }

    #[test]
    fn pkg2_dir_string_v2_roundtrip_i16_len() {
        for s in ["", "Map", "00002000.img", "긴 이름 테스트"] {
            let enc = encode_pkg2_dir_string_v2(s);
            let mut c = ctx(enc);
            let got = read_pkg2_dir_string_v2(&mut c, None).unwrap();
            assert_eq!(got.text, s, "PKG2(i16) 왕복 실패: {s:?}");
        }
    }

    #[test]
    fn pkg2_v1_and_v2_use_different_length_fields() {
        let s = "ABCDE";
        let v1 = encode_pkg2_dir_string(s);
        let v2 = encode_pkg2_dir_string_v2(s);
        assert_eq!(v1[0], 5, "v1 은 i8 길이");
        assert_eq!(i16::from_le_bytes([v2[0], v2[1]]), 5, "v2 는 i16 길이");
        assert_eq!(v1.len(), 1 + 10);
        assert_eq!(v2.len(), 2 + 10);
    }

    #[test]
    fn truncated_string_is_error() {
        let mut c = ctx(vec![5, 0x41]); // 길이 5 인데 페이로드 1바이트
        assert!(read_pkg2_dir_string(&mut c, None).is_err());
    }

    #[test]
    fn negative_escape_length_is_rejected() {
        let mut c = ctx({
            let mut b = vec![0x80u8];
            b.extend_from_slice(&(-5i32).to_le_bytes());
            b
        });
        assert!(read_image_string(&mut c, None).is_err(), "음수 길이는 거부");
    }
}
