//! WZ 압축 정수 (원본 `WzComparerR2.WzLib/WzBinaryReader.cs:49-86`).
//!
//! # 포맷
//!
//! 1바이트 부호 있는 길이 접두사를 쓴다. `-128` (`0x80`) 이면 뒤에 고정폭 정수가 온다.
//!
//! ```text
//! ReadCompressedInt32:  s = i8;  s == -128 ? i32_le(next 4 bytes) : (int)s
//! ReadCompressedInt64:  s = i8;  s == -128 ? i64_le(next 8 bytes) : (long)s
//! ReadCompressedUInt32: (uint)ReadCompressedInt32()
//! ```
//!
//! 이건 LEB128도 varint도 **아니다** — 값 128 이상이 아니라 `-128` 만 이스케이프다.
//! 흔한 오해이므로 테스트로 고정한다.

use wz_error::Result;
use wz_pool::ReadCtx;

/// `ReadCompressedInt32`
pub fn read_compressed_i32(c: &mut ReadCtx) -> Result<i32> {
    let s = c.read_i8()?;
    if s == -128 {
        Ok(c.read_i32_le()?)
    } else {
        Ok(s as i32)
    }
}

/// `ReadCompressedInt64`
pub fn read_compressed_i64(c: &mut ReadCtx) -> Result<i64> {
    let s = c.read_i8()?;
    if s == -128 {
        Ok(c.read_i64_le()?)
    } else {
        Ok(s as i64)
    }
}

/// `ReadCompressedUInt32` — 부호 있는 32비트로 읽은 뒤 재해석.
pub fn read_compressed_u32(c: &mut ReadCtx) -> Result<u32> {
    Ok(read_compressed_i32(c)? as u32)
}

/// `ReadCompressedUInt64`
pub fn read_compressed_u64(c: &mut ReadCtx) -> Result<u64> {
    Ok(read_compressed_i64(c)? as u64)
}

// ─────────────────────────────────────────────────────────────────────────────
// 인코딩 (테스트 WZ 빌더와 Patcher 용)
// ─────────────────────────────────────────────────────────────────────────────

/// `ReadCompressedInt32` 의 역연산.
pub fn encode_compressed_i32(v: i32) -> Vec<u8> {
    if (-127..=127).contains(&v) {
        vec![v as i8 as u8]
    } else {
        let mut out = vec![0x80u8];
        out.extend_from_slice(&v.to_le_bytes());
        out
    }
}

/// `ReadCompressedInt64` 의 역연산.
pub fn encode_compressed_i64(v: i64) -> Vec<u8> {
    if (-127..=127).contains(&v) {
        vec![v as i8 as u8]
    } else {
        let mut out = vec![0x80u8];
        out.extend_from_slice(&v.to_le_bytes());
        out
    }
}

/// `ReadCompressedUInt32` 의 역연산.
pub fn encode_compressed_u32(v: u32) -> Vec<u8> {
    encode_compressed_i32(v as i32)
}

/// 이 모듈이 쓰는 고정 폭 바이트를 읽는다. 디버깅용.
pub fn read_fixed_i32(c: &mut ReadCtx) -> Result<i32> {
    c.read_i32_le()
}

#[cfg(test)]
mod tests {
    use super::*;
    fn ctx(bytes: Vec<u8>) -> ReadCtx {
        ReadCtx::from_bytes(bytes)
    }

    fn roundtrip_i32(v: i32) {
        let enc = encode_compressed_i32(v);
        let mut c = ctx(enc.clone());
        let got = read_compressed_i32(&mut c).unwrap();
        assert_eq!(got, v, "i32 왕복 실패 v={v} enc={enc:02X?}");
        assert_eq!(c.pos(), enc.len() as u64, "v={v} 에서 전부 소비돼야 한다");
    }

    #[test]
    fn roundtrip_all_boundary_values_i32() {
        for v in [
            0,
            1,
            -1,
            127,
            126,
            -127,
            -126,
            128,
            -128,
            129,
            -129,
            255,
            256,
            i32::MAX,
            i32::MIN,
            i32::MAX - 1,
            i32::MIN + 1,
            100_000,
            -100_000,
        ] {
            roundtrip_i32(v);
        }
    }

    #[test]
    fn roundtrip_all_boundary_values_i64() {
        for v in [
            0i64,
            1,
            -1,
            127,
            128,
            -128,
            -129,
            i64::MAX,
            i64::MIN,
            i64::MAX - 1,
            1_000_000_000_000i64,
            -1_000_000_000_000i64,
        ] {
            let enc = encode_compressed_i64(v);
            let mut c = ctx(enc.clone());
            assert_eq!(read_compressed_i64(&mut c).unwrap(), v, "i64 왕복 v={v}");
            assert_eq!(c.pos(), enc.len() as u64);
        }
    }

    #[test]
    fn roundtrip_u32() {
        for v in [0u32, 1, 127, 128, 65535, u32::MAX, u32::MAX - 1] {
            let enc = encode_compressed_u32(v);
            let mut c = ctx(enc);
            assert_eq!(read_compressed_u32(&mut c).unwrap(), v, "u32 왕복 v={v}");
        }
    }

    #[test]
    fn small_values_are_single_byte() {
        assert_eq!(encode_compressed_i32(0), vec![0x00]);
        assert_eq!(encode_compressed_i32(1), vec![0x01]);
        assert_eq!(encode_compressed_i32(127), vec![0x7F]);
        assert_eq!(encode_compressed_i32(-1), vec![0xFF]);
        assert_eq!(encode_compressed_i32(-127), vec![0x81]);
        // 경계 밖은 이스케이프
        assert_eq!(encode_compressed_i32(128).len(), 5);
        assert_eq!(encode_compressed_i32(-128).len(), 5);
    }

    #[test]
    fn escape_marker_is_0x80_not_0x81() {
        // 자주 하는 오류: -127(0x81) 을 이스케이프로 착각.
        // 원본은 오직 -128(0x80) 만 이스케이프다.
        let mut c = ctx(vec![0x81, 0xAA, 0xBB]);
        assert_eq!(read_compressed_i32(&mut c).unwrap(), -127);
        assert_eq!(c.pos(), 1, "0x81 은 이스케이프가 아니라 값 -127");

        let mut c = ctx(vec![0x80, 0x01, 0x00, 0x00, 0x00]);
        assert_eq!(read_compressed_i32(&mut c).unwrap(), 1);
        assert_eq!(c.pos(), 5);
    }

    #[test]
    fn not_leb128() {
        // LEB128이라면 128 이 0x80 0x01 로 인코딩되지만, WZ 는 고정폭 4바이트를 쓴다.
        let enc = encode_compressed_i32(128);
        assert_eq!(enc, vec![0x80, 0x80, 0x00, 0x00, 0x00], "고정폭 i32 LE");
    }

    #[test]
    fn truncated_escape_is_error_not_panic() {
        let mut c = ctx(vec![0x80, 0x01, 0x00]); // 4바이트 중 3바이트만
        assert!(read_compressed_i32(&mut c).is_err());
    }

    #[test]
    fn empty_input_is_error() {
        let mut c = ctx(vec![]);
        assert!(read_compressed_i32(&mut c).is_err());
        assert!(read_compressed_i64(&mut c).is_err());
    }

    #[test]
    fn sequential_reads_advance_correctly() {
        let mut buf = Vec::new();
        for v in [1i32, 300, -5, i32::MAX] {
            buf.extend_from_slice(&encode_compressed_i32(v));
        }
        let mut c = ctx(buf);
        for v in [1i32, 300, -5, i32::MAX] {
            assert_eq!(read_compressed_i32(&mut c).unwrap(), v);
        }
        assert_eq!(c.pos(), c.len().unwrap());
    }

    #[test]
    fn this_is_not_varint() {
        // 문서화 목적: 주석이 정확함을 고정한다.
        // WZ 압축 정수는 LE128 이다. (LEB128 과 혼동하지 말 것)
        assert_eq!(encode_compressed_i32(0x7F).len(), 1);
        assert_eq!(encode_compressed_i32(0x80).len(), 5);
        assert_eq!(encode_compressed_i64(0x7F).len(), 1);
        assert_eq!(encode_compressed_i64(0x80).len(), 9);
    }
}
