//! WZ 암호화 계층 (리포트 §2.3 / §14.4).
//!
//! WZ는 네 개의 독립적인 암호를 쓴다. 서로 섞이지 않는다.
//!
//! | 대상 | 모듈 |
//! |---|---|
//! | 이미지 프로퍼티 문자열, PKG1 디렉터리 이름, 청크 암호화 PNG | [`aes`] (BMS/KMS/GMS) |
//! | PKG2 디렉터리 이름 #0 | [`pkg2_key`] |
//! | MS v1 헤더/엔트리 테이블/이미지 앞 1024B | [`snow2`] |
//! | MS v2 헤더/엔트리 테이블 | [`chacha`] |
//!
//! # 왜 각각 다른가
//!
//! 이들은 클라이언트 세대마다 독립적으로 바뀌었기 때문에 하나의 추상화로
//! 묶을 수 없다. 다만 공통 계약은 하나다: **바이트 배열의 특정 오프셋 구간을
//! XOR 로 뒤집는다.** 그 계약만 [`Decryptor`] 로 표현하고, 구현은
//! [`Decryptor::decrypt_at`] 을 통해 노출한다.

use wz_error::WzCryptoError;

pub mod aes;
pub mod chacha;
pub mod pkg2_key;
pub mod snow2;
mod snow2_tables;

/// 바이트 구간 XOR 디크립터.
///
/// `decrypt_at` 은 `data` 의 위치 0 을 `key_offset` 위치의 키스트림에 대응시킨다.
pub trait Decryptor: Send + Sync + std::fmt::Debug {
    /// `data[..]` 를 `key_offset` 부터의 키스트림으로 XOR 한다.
    fn decrypt_at(&self, data: &mut [u8], key_offset: u64);

    /// `src` 를 `dst` 로 XOR 복호화한다. 길이는 같아야 한다.
    fn decrypt_into(&self, src: &[u8], dst: &mut [u8], key_offset: u64) {
        assert_eq!(src.len(), dst.len(), "복호화 길이 불일치");
        dst.copy_from_slice(src);
        self.decrypt_at(dst, key_offset);
    }

    /// 키스트림에서 `key_offset` 위치의 단일 바이트.
    fn key_byte(&self, key_offset: u64) -> u8;

    /// 이 디크립터가 아무것도 하지 않는가 (BMS no-op 등). 최적화 힌트.
    fn is_identity(&self) -> bool {
        false
    }

    /// 최대 키스트REAM 길이 (오프셋 범위 검증용).
    fn key_stream_len(&self) -> u64 {
        u64::MAX
    }
}

/// 키 종류. 원본 `Wz_Crypto.cs:22-30` 의 `Wz_CryptoKeyType` 과 1:1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum CryptoKeyType {
    Unknown = 0,
    Bms = 1,
    Kms = 2,
    Gms = 3,
    /// `Wz_Crypto.GetKeys` 는 이 값에 대해 `NotSupportedException` 을 던진다
    /// (`Wz_Crypto.cs:129-141`). KMST1199 는 PKG1 키 경로가 없다.
    Kmst1198 = 4,
    Kmst1199 = 5,
}

impl CryptoKeyType {
    pub fn from_u8(v: u8) -> Result<Self, WzCryptoError> {
        Ok(match v {
            1 => Self::Bms,
            2 => Self::Kms,
            3 => Self::Gms,
            4 => Self::Kmst1198,
            5 => Self::Kmst1199,
            _ => return Err(WzCryptoError::UnknownKeyType(v)),
        })
    }
}

/// 하드코딩된 상수 (원본과 **반드시** 일치해야 한다).
///
/// # 주의: IV 는 16 바이트가 아니라 **4 바이트**다
///
/// 원본 `Wz_Crypto.cs:105-106` 이 `byte[4]` 이고, 첫 AES 블록을 만들 때
/// `block[j] = iv[j % 4]` 로 4회 반복한다 (`Wz_Crypto.cs:198-201`).
/// 16 바이트 IV 라고 가정하면 모든 KMS/GMS 문자열 복호화가 깨진다.
pub mod consts {
    /// BMS/GMS 공통 AES-256 키 (원본 `Wz_Crypto.cs:290-297`)
    pub const AES_KEY: [u8; 32] = [
        0x13, 0x00, 0x00, 0x00, 0x08, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00, 0x00, 0xB4, 0x00, 0x00,
        0x00, 0x1B, 0x00, 0x00, 0x00, 0x0F, 0x00, 0x00, 0x00, 0x33, 0x00, 0x00, 0x00, 0x52, 0x00,
        0x00, 0x00,
    ];

    /// GMS IV — **4 바이트** (`Wz_Crypto.cs:105`)
    pub const GMS_IV: [u8; 4] = [0x4D, 0x23, 0xC7, 0x2B];

    /// KMS IV — **4 바이트** (`Wz_Crypto.cs:106`)
    pub const KMS_IV: [u8; 4] = [0xB9, 0x7D, 0x63, 0xE9];

    /// `Ms_FileV2.chacha20KeyObscure` (원본 `Ms_FileV2.cs:233-238`)
    pub const CHACHA20_KEY_OBSCURE: [u8; 32] = [
        0x7B, 0x2F, 0x35, 0x48, 0x43, 0x95, 0x02, 0xB9, 0xAE, 0x91, 0xA6, 0xE1, 0xD8, 0xD6, 0x24,
        0xB4, 0x33, 0x10, 0x1D, 0x3D, 0xC1, 0xBB, 0xC6, 0xF4, 0xA5, 0xFE, 0xB3, 0x69, 0x6B, 0x56,
        0xE4, 0x75,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_type_roundtrip() {
        for v in 1u8..=5 {
            let k = CryptoKeyType::from_u8(v).unwrap();
            assert_eq!(k as u8, v);
        }
        assert!(CryptoKeyType::from_u8(0).is_err());
        assert!(CryptoKeyType::from_u8(6).is_err());
        assert!(CryptoKeyType::from_u8(255).is_err());
    }

    #[test]
    fn constants_are_exact() {
        // 리포트 §14.4: 상수는 눈으로 못 검증한다. 원본 소스와 한 번 대조한다.
        assert_eq!(consts::AES_KEY.len(), 32);
        assert_eq!(
            &consts::AES_KEY[0..16],
            &[0x13, 0, 0, 0, 0x08, 0, 0, 0, 0x06, 0, 0, 0, 0xB4, 0, 0, 0]
        );
        // IV 는 4 바이트다 (16 바이트가 아니다 — 리포트 §2.3)
        assert_eq!(consts::GMS_IV.len(), 4);
        assert_eq!(consts::GMS_IV, [0x4D, 0x23, 0xC7, 0x2B]);
        assert_eq!(consts::KMS_IV.len(), 4);
        assert_eq!(consts::KMS_IV, [0xB9, 0x7D, 0x63, 0xE9]);
        assert_eq!(consts::CHACHA20_KEY_OBSCURE.len(), 32);
        assert_eq!(consts::CHACHA20_KEY_OBSCURE[0], 0x7B);
        assert_eq!(consts::CHACHA20_KEY_OBSCURE[31], 0x75);
    }
}
