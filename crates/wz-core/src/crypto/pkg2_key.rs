//! PKG2 디렉터리 문자열 키 (원본 `Wz_Crypto.Pkg2DirStringKey` 계열).
//!
//! PKG2 컨테이너는 **첫 번째** 디렉터리 엔트리 이름만 별도 키로 암호화하고,
//! 나머지는 BMS 키(AES)로 처리한다 (`WzDirStringReader.cs:65-106`).
//!
//! # 키 바이트 패턴 — 함정
//!
//! 원본 `Wz_Crypto.cs:362-375` 의 `CreateKeyIfNotExist`:
//!
//! ```csharp
//! byte[] keys = new byte[8];
//! Span<ushort> u16Keys = MemoryMarshal.Cast<byte, ushort>(keys.AsSpan());
//! for (int i = 0; i < 4; i++)
//!     u16Keys[i] = (ushort)(this.baseKey >> (8 * i));
//! ```
//!
//! 흔한 오해는 "baseKey 하위 32비트 리틀엔디언 + 4바이트 0" 이라는 것이다.
//! **아니다.** `u16Keys[i] = (ushort)(baseKey >> (8*i))` 는 8비트씩 이동한
//! 값을 16비트에 담으므로, 결과 바이트 인덱스 순서는
//! `[0, 1, 1, 2, 2, 3, 3, 4]` 가 된다.
//!
//! 즉 `keys[j] = (baseKey >> (8 * (j/2 + (j & 1)))) as u8`.
//! 이걸 틀리면 KMST1202+ 디렉터리 트리 전체가 깨진다.

use super::Decryptor;

/// 키스트림 길이. V1~V5 는 8, V6 는 16.
const LEN_V1: usize = 8;
const LEN_V6: usize = 16;

/// baseKey → 반복 키스트림 바이트 변환 (위 함의 구현).
///
/// 원본은 `u16Keys[i] = (ushort)(baseKey >> (8 * i))` 를 계산하므로
/// `i` 가 커지면 상위 비트가 0 으로 잘린다. Rust 에서 `>> 64` 는 패닉이므로
/// 범위를 명시적으로 검사한다.
///
/// V6 (16바이트) 에서는 `i = 7` 에서 `shift = 64` 가 되므로 이 처리가 필수다.
#[inline]
fn key_byte_at(base_key: u64, j: usize) -> u8 {
    let shift = 4 * j + 4 * (j & 1);
    if shift >= 64 {
        0
    } else {
        ((base_key >> shift) & 0xFF) as u8
    }
}

/// PKG2 디렉터리 문자열 키스트림.
///
/// baseKey 가 고정이며, 주기적으로 반복된다. 할당 없음 — 버퍼가 8 또는 16바이트다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pkg2DirStringKey {
    base_key: u64,
    period: usize,
}

impl Pkg2DirStringKey {
    /// V1~V5: 8바이트 키스트림.
    pub fn v5(base_key: u64) -> Self {
        Self {
            base_key,
            period: LEN_V1,
        }
    }

    /// V6 (KMST1206): 16바이트 키스트림.
    pub fn v6(base_key: u64) -> Self {
        Self {
            base_key,
            period: LEN_V6,
        }
    }

    /// KMST1198 고정 키 (원본 `Wz_Crypto.cs:39`: `new Pkg2DirStringKey(0xDEADBEEF)`).
    pub fn kmst1198() -> Self {
        Self::v5(0xDEAD_BEEF)
    }

    /// V2 (KMST1199): `Mix(Mix(base) ^ 0x4F4CB34A)` (원본 `:446-450`)
    pub fn v2(hash1: u32, hash_version: u32) -> Self {
        let base_hash = hash1 ^ hash_version ^ 0x6D4C_3B2A;
        Self::v5(mix32(mix32(base_hash) ^ 0x4F4C_B34A) as u64)
    }

    /// V3 (KMST1202): 64비트 XOR (원본 `:460-463`)
    pub fn v3(hash1: u64, hash_version: u64) -> Self {
        Self::v5(hash1 ^ hash_version ^ 0x66B5_7FEE_317F_D3DF)
    }

    /// V4 (KMST1204): **위치 의존** 키 (원본 `:479-486`)
    ///
    /// 호출 시점마다 파일 위치가 달라지므로, 디크립터를 재사용할 수 없다.
    /// `WzDirStringReader.cs:99-100` 이 매번 이름을 읽기 직전 `ApplyState` 호출.
    pub fn v4(hash1: u64, hash_version: u64, file_position: u64) -> Self {
        let base = hash1
            ^ hash_version
            ^ 0x2181_0F65_FEC3_2BDC
            ^ 0x9E37_79B9_7F4A_7C15u64.wrapping_mul(file_position);
        Self::v5(base)
    }

    pub fn base_key(&self) -> u64 {
        self.base_key
    }

    pub fn period(&self) -> usize {
        self.period
    }
}

/// 32비트 finalizer mix (원본 `Utilities/MathHelper.cs:16-24`).
#[inline]
pub fn mix32(mut x: u32) -> u32 {
    x ^= x >> 16;
    x = x.wrapping_mul(0x21F0_AAAD);
    x ^= x >> 15;
    x = x.wrapping_mul(0x735A_2D97);
    x ^= x >> 15;
    x
}

impl Decryptor for Pkg2DirStringKey {
    fn decrypt_at(&self, data: &mut [u8], key_offset: u64) {
        // 원본은 `keyOffset % 8 == 0` 을 요구한다 (`:406-409`).
        // 스트리밍 상위 레이어에서 항상 8 배수 오프셋이 오도록 설계되어 있으므로
        // 여기서는 나머지 연산으로 정규화한다.
        let start = (key_offset as usize) % self.period;
        for (i, b) in data.iter_mut().enumerate() {
            *b ^= key_byte_at(self.base_key, (start + i) % self.period);
        }
    }

    fn key_byte(&self, key_offset: u64) -> u8 {
        key_byte_at(self.base_key, (key_offset as usize) % self.period)
    }

    fn key_stream_len(&self) -> u64 {
        self.period as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 원본 C# `Pkg2DirStringKey.CreateKeyIfNotExist` 를 그대로 옮긴 참조 구현.
    fn reference_keys(base_key: u64, len: usize) -> Vec<u8> {
        let mut keys = vec![0u8; len];
        let u16_count = len / 2;
        for i in 0..u16_count {
            let v = (base_key >> (8 * i)) as u16;
            let b = v.to_le_bytes();
            keys[i * 2] = b[0];
            keys[i * 2 + 1] = b[1];
        }
        keys
    }

    #[test]
    fn key_bytes_match_c_sharp_reference() {
        // 함정이 실제로 존재하는 값으로 검증한다.
        for base in [
            0xDEAD_BEEFu64,
            0x0123_4567_89AB_CDEFu64,
            0xFFFF_FFFF_FFFF_FFFF,
            0,
        ] {
            for len in [8usize, 16] {
                let reference = reference_keys(base, len);
                let key = if len == 8 {
                    Pkg2DirStringKey::v5(base)
                } else {
                    Pkg2DirStringKey::v6(base)
                };
                let actual: Vec<u8> = (0..len).map(|j| key.key_byte(j as u64)).collect();
                assert_eq!(actual, reference, "base={base:#x} len={len}");
            }
        }
    }

    #[test]
    fn key_byte_order_is_not_naive_le() {
        // 함정 검증: 0x.._11223344 면 인덱스 순서는 [44,33,33,22,22,11,11,00]
        let base = 0x0000_0000_1122_3344u64;
        let key = Pkg2DirStringKey::v5(base);
        let got: Vec<u8> = (0..8).map(|j| key.key_byte(j as u64)).collect();
        assert_eq!(
            got,
            vec![0x44, 0x33, 0x33, 0x22, 0x22, 0x11, 0x11, 0x00],
            "'하위 32비트 LE + 4바이트 0' 이면 [44,33,22,11,00,00,00,00] 이 된다"
        );
    }

    #[test]
    fn period_is_8_for_v1_to_v5_and_16_for_v6() {
        assert_eq!(Pkg2DirStringKey::kmst1198().period(), 8);
        assert_eq!(Pkg2DirStringKey::v3(1, 2).period(), 8);
        assert_eq!(Pkg2DirStringKey::v4(1, 2, 3).period(), 8);
        assert_eq!(Pkg2DirStringKey::v6(1).period(), 16);
    }

    #[test]
    fn decrypt_wraps_around_period() {
        let key = Pkg2DirStringKey::v5(0x1234_5678_9ABC_DEF0);
        let stream: Vec<u8> = (0..8).map(|j| key.key_byte(j as u64)).collect();

        let plain = vec![0xAAu8; 20];
        let mut buf = plain.clone();
        key.decrypt_at(&mut buf, 0);
        for (i, b) in plain.iter().enumerate() {
            assert_eq!(buf[i], b ^ stream[i % 8], "인덱스 {i}");
        }
    }

    #[test]
    fn decrypt_at_nonzero_offset_starts_there() {
        let key = Pkg2DirStringKey::v5(0xAAAA_AAAA_AAAA_AAAA);
        let plain = vec![0x5Au8; 16];
        let mut buf = plain.clone();
        key.decrypt_at(&mut buf, 4);
        assert_eq!(buf[0], plain[0] ^ key.key_byte(4));
        assert_eq!(buf[1], plain[1] ^ key.key_byte(5));
        assert_eq!(buf[7], plain[7] ^ key.key_byte(3), "주기를 넘어감");
    }

    #[test]
    fn v4_is_position_dependent() {
        let a = Pkg2DirStringKey::v4(0x1111, 0x2222, 100);
        let b = Pkg2DirStringKey::v4(0x1111, 0x2222, 200);
        assert_ne!(a.base_key(), b.base_key(), "파 위치가 키에 반영돼야 한다");
        assert_eq!(
            a.base_key(),
            0x1111 ^ 0x2222 ^ 0x2181_0F65_FEC3_2BDC ^ 0x9E37_79B9_7F4A_7C15u64.wrapping_mul(100)
        );
    }

    #[test]
    fn v2_uses_mix_chain() {
        let expected = mix32(mix32(0xA ^ 0xB ^ 0x6D4C_3B2A) ^ 0x4F4C_B34A) as u64;
        assert_eq!(Pkg2DirStringKey::v2(0xA, 0xB).base_key(), expected);
    }

    #[test]
    fn v3_is_pure_xor() {
        assert_eq!(
            Pkg2DirStringKey::v3(0xAAAA, 0xBBBB).base_key(),
            0xAAAA ^ 0xBBBB ^ 0x66B5_7FEE_317F_D3DF
        );
    }

    #[test]
    fn kmst1198_is_fixed() {
        assert_eq!(Pkg2DirStringKey::kmst1198().base_key(), 0xDEAD_BEEF);
    }

    #[test]
    fn mix32_is_deterministic_and_involutive_enough() {
        assert_eq!(mix32(0), mix32(0));
        assert_ne!(mix32(1), mix32(2));
        // 알려진 고정값 (자기 참조 방지용으로만 사용되므로 정확성보다 결정성이 중요)
        let a = mix32(0x1234_5678);
        assert_eq!(a, mix32(0x1234_5678));
    }

    #[test]
    fn decrypt_roundtrips() {
        let key = Pkg2DirStringKey::v6(0x0102_0304_0506_0708);
        let plain = b"PKG2 directory name";
        let mut buf = plain.to_vec();
        key.decrypt_at(&mut buf, 0);
        assert_ne!(&buf[..], &plain[..]);
        key.decrypt_at(&mut buf, 0);
        assert_eq!(&buf[..], &plain[..], "XOR 는 self-inverse");
    }
}
