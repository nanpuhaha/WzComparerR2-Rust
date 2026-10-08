//! BMS/KMS/GMS AES-256-ECB 키스트림 (원본 `Wz_Crypto.Wz_CryptoKey`).
//!
//! # 알고리즘
//!
//! 원본은 AES 를 **암호 블록 생성기**로 쓰고, 결과를 **XOR 마스크**로 쓴다.
//! ECB 모드인 AES 를 CBC 체인처럼 연결해 키스트림을 만든다:
//!
//! ```text
//! K(-1) = iv[0..4] 를 4회 반복한 16 바이트
//! K(0)  = AES-256-ECB-Encrypt(aesKey, K(-1))
//! K(1)  = AES-256-ECB-Encrypt(aesKey, K(0))
//! K(2)  = AES-256-ECB-Encrypt(aesKey, K(1))
//! ...
//! keystream = K(0) || K(1) || K(2) || ...
//!
//! 복호화: out[i] = in[i] ^ keystream[keyOffset + i]
//! ```
//!
//! 원본 `Wz_Crypto.cs:193-208` 의 `CryptoStream` + ECB 조합이 정확히 이
//! 결과를 만든다 (ECB 는 블록 단독 암호화이므로 체인은 위와 같다).
//!
//! 크기 증가는 64 바이트 단위로 반올림한다 (`Wz_Crypto.cs:172`) —
//! 관찰 가능한 동작이 아니지만 정확히 재현한다.

use std::sync::Mutex;

use aes::cipher::{BlockEncrypt, KeyInit};
use aes::Aes256;

use super::{consts, CryptoKeyType, Decryptor};

/// 키스트림을 필요에 따라 확장하는 AES 디크립터.
///
/// `decrypt_at` 은 `&self` 이지만 내부 버퍼가 자란다. 잠금으로 보호한다.
#[derive(Debug)]
pub struct AesKeyStream {
    /// 복호화 시 XOR 하는 키스트림.
    keys: Mutex<Vec<u8>>,
    /// 첫 블록의 시드 (4 바이트, 4회 반복되어 16 바이트 블록이 된다).
    seed: [u8; 4],
}

impl AesKeyStream {
    /// 키 종류에 맞는 디크립터를 만든다.
    pub fn for_key_type(ty: CryptoKeyType) -> Option<Self> {
        match ty {
            CryptoKeyType::Kms => Some(Self::kms()),
            CryptoKeyType::Gms => Some(Self::gms()),
            // BMS 는 키스트림이 전부 0 (무 Cipher). `None` 으로 표현한다.
            _ => None,
        }
    }

    pub fn kms() -> Self {
        Self::with_seed(consts::KMS_IV)
    }

    pub fn gms() -> Self {
        Self::with_seed(consts::GMS_IV)
    }

    pub fn with_seed(seed: [u8; 4]) -> Self {
        Self {
            keys: Mutex::new(Vec::new()),
            seed,
        }
    }

    /// 키스트림이 `end` 바이트까지 확보되도록 확장한다.
    ///
    /// 원본의 증분 확장(`startIndex = keys.Length`)과 동일한 결과를 낸다.
    /// ECB 는 블록 단독이므로 새로 붙이는 블록의 입력은 이미 있는 마지막 블록이다.
    fn ensure(&self, end: usize) -> std::sync::MutexGuard<'_, Vec<u8>> {
        let mut keys = self.keys.lock().expect("AES 키스트림 lock poisoned");
        if keys.len() >= end {
            return keys;
        }
        // 64 바이트 단위 반올림 (원본 `(size + 63) & ~63`)
        let target = (end + 63) & !63;
        let cipher = Aes256::new_from_slice(&consts::AES_KEY).expect("32바이트 AES-256 키");

        // 첫 블록은 IV 4회 반복
        if keys.is_empty() {
            let mut block = [0u8; 16];
            for (j, b) in block.iter_mut().enumerate() {
                *b = self.seed[j % 4];
            }
            let mut buf = aes::cipher::Block::<Aes256>::clone_from_slice(&block);
            cipher.encrypt_block(&mut buf);
            keys.extend_from_slice(&buf);
        }

        // 이후 블록: 이전 블록(암문)을 입력으로
        while keys.len() < target {
            let mut buf = aes::cipher::Block::<Aes256>::clone_from_slice(&keys[keys.len() - 16..]);
            cipher.encrypt_block(&mut buf);
            keys.extend_from_slice(&buf);
        }
        keys
    }

    /// 키스트림의 앞 `len` 바이트를 복사한다 (디버그/테스트용).
    pub fn keystream_prefix(&self, len: usize) -> Vec<u8> {
        self.ensure(len).clone()[..len].to_vec()
    }

    pub fn cached_len(&self) -> usize {
        self.keys.lock().map(|k| k.len()).unwrap_or(0)
    }

    /// 같은 키의 새 인스턴스. 캐시된 키스트림은 공유하지 않는다.
    ///
    /// `Mutex<Vec<u8>>` 때문에 `Clone` 을 파생할 수 없다. 이 방식은
    /// **같은 키로 다시 생성**하되 상태를 공유하지 않으므로, 한쪽에서
    /// `decrypt_at` 을 호출해도 다른 쪽에 영향이 없다.
    pub fn try_clone(&self) -> Self {
        Self::with_seed(self.seed)
    }
}

impl Decryptor for AesKeyStream {
    fn decrypt_at(&self, data: &mut [u8], key_offset: u64) {
        let start = key_offset as usize;
        let end = start + data.len();
        let keys = self.ensure(end);
        for (i, b) in data.iter_mut().enumerate() {
            *b ^= keys[start + i];
        }
    }

    fn key_byte(&self, key_offset: u64) -> u8 {
        let keys = self.ensure(key_offset as usize + 1);
        keys[key_offset as usize]
    }

    fn key_stream_len(&self) -> u64 {
        u64::MAX
    }
}

/// WZ 키 종류 (원본 `Wz_CryptoKeyType`).
///
/// # 함정: BMS 는 AES 가 아니다
///
/// `BMS` 는 키스트림이 전부 0 인 **항등** 변환이다 (`Wz_NonOpCryptoKey`).
/// `kms()`/`gms()` 처럼 AES 로 만들면 안 된다 — 이미지 키 선별이
/// 잘못 동작한다 (`image.rs` 의 `detect_key`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyKind {
    /// 항등 (키스트림 0).
    Bms,
    /// AES `kms()`.
    Kms,
    /// AES `gms()`.
    Gms,
}

/// 키 종류에 대응하는 디크립터를 만든다.
///
/// BMS 는 [`NoOpCrypto`] 라 복호화 비용이 0 이고, 나머지는 AES 키스트림이다.
pub fn key_stream(kind: KeyKind) -> Box<dyn Decryptor> {
    match kind {
        KeyKind::Bms => Box::new(NoOpCrypto),
        KeyKind::Kms => Box::new(AesKeyStream::kms()),
        KeyKind::Gms => Box::new(AesKeyStream::gms()),
    }
}

/// BMS: 키스트림이 전부 0 인 항등 디크립터 (원본 `Wz_NonOpCryptoKey`).
#[derive(Debug, Default, Clone, Copy)]
pub struct NoOpCrypto;

impl Decryptor for NoOpCrypto {
    fn decrypt_at(&self, _data: &mut [u8], _key_offset: u64) {
        // 원본 `Wz_NonOpCryptoKey.Decrypt` 은 아무것도 하지 않는다.
        // 단, `Decrypt(in, out, off)` 만 `inputBuffer.CopyTo(outputBuffer)` 한다.
        // in-place 호출 경로만 쓰므로 no-op 이 정확히 일치한다.
    }

    fn key_byte(&self, _key_offset: u64) -> u8 {
        0
    }

    fn is_identity(&self) -> bool {
        true
    }

    fn key_stream_len(&self) -> u64 {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 원본 C# `Wz_CryptoKey.EnsureKeySize` 를 그대로 옮긴 참조 구현.
    /// 이 구현과 Rust 구현이 **바이트 단위로** 같아야 한다.
    fn reference_keystream(iv: [u8; 4], blocks: usize) -> Vec<u8> {
        use aes::cipher::{BlockEncrypt, KeyInit};
        let cipher = Aes256::new_from_slice(&consts::AES_KEY).unwrap();
        let mut keys = vec![0u8; blocks * 16];
        for i in 0..blocks {
            let off = i * 16;
            let mut block = [0u8; 16];
            if i == 0 {
                for (j, b) in block.iter_mut().enumerate() {
                    *b = iv[j % 4];
                }
            } else {
                block.copy_from_slice(&keys[off - 16..off]);
            }
            let mut buf = aes::cipher::Block::<Aes256>::clone_from_slice(&block);
            cipher.encrypt_block(&mut buf);
            keys[off..off + 16].copy_from_slice(&buf);
        }
        keys
    }

    #[test]
    fn keystream_matches_reference_implementation() {
        for (name, ks) in [("GMS", AesKeyStream::gms()), ("KMS", AesKeyStream::kms())] {
            let iv = if name == "GMS" {
                consts::GMS_IV
            } else {
                consts::KMS_IV
            };
            let reference = reference_keystream(iv, 8);
            let actual = ks.keystream_prefix(128);
            assert_eq!(actual, reference, "{name} 키스트림 불일치");
        }
    }

    #[test]
    fn gms_and_kms_differ() {
        let g = AesKeyStream::gms().keystream_prefix(32);
        let k = AesKeyStream::kms().keystream_prefix(32);
        assert_ne!(g, k, "IV 가 다르면 키스트림도 달라야 한다");
    }

    #[test]
    fn first_block_is_deterministic_and_iv_dependent() {
        // IV 4바이트가 4회 반복되어 16바이트 블록을 이룬다.
        let a = AesKeyStream::with_seed([1, 2, 3, 4]).keystream_prefix(16);
        let b = AesKeyStream::with_seed([1, 2, 3, 4]).keystream_prefix(16);
        assert_eq!(a, b, "결정적이어야 한다");
        assert_eq!(a, reference_keystream([1, 2, 3, 4], 1));
    }

    #[test]
    fn grow_is_incremental_and_stable() {
        let ks = AesKeyStream::kms();
        // 작은 요청
        let small = ks.keystream_prefix(16);
        // 더 큰 요청 → 앞부분이 바뀌면 안 된다
        let large = ks.keystream_prefix(256);
        assert_eq!(
            &large[..16],
            &small[..16],
            "증가해도 앞부분이 유지되어야 한다"
        );
        assert_eq!(large, reference_keystream(consts::KMS_IV, 16));
    }

    #[test]
    fn growth_rounds_up_to_64() {
        // 원본 `(size + 63) & ~63`
        let ks = AesKeyStream::kms();
        drop(ks.ensure(1));
        assert_eq!(ks.cached_len(), 64);
        drop(ks.ensure(65));
        assert_eq!(ks.cached_len(), 128);
        drop(ks.ensure(128));
        assert_eq!(ks.cached_len(), 128, "충분하면 확장하지 않는다");
    }

    #[test]
    fn decrypt_is_xor_with_keystream() {
        let ks = AesKeyStream::kms();
        let plain = b"Hello, MapleStory!";

        let mut buf = plain.to_vec();
        ks.decrypt_at(&mut buf, 0);
        assert_ne!(&buf[..], &plain[..], "평문이 바뀌어야 한다");

        // 같은 키스트림으로 되돌리면 평문 복구 (XOR 는 self-inverse)
        ks.decrypt_at(&mut buf, 0);
        assert_eq!(&buf[..], &plain[..]);
    }

    #[test]
    fn decrypt_at_offset_uses_that_offset() {
        let ks = AesKeyStream::kms();
        let stream = ks.keystream_prefix(256);

        let plain = b"offset test";
        let mut buf = plain.to_vec();
        ks.decrypt_at(&mut buf, 64);

        for (i, b) in plain.iter().enumerate() {
            assert_eq!(buf[i], b ^ stream[64 + i], "오프셋 64 기준");
        }
    }

    #[test]
    fn decrypt_into_matches_in_place() {
        let ks = AesKeyStream::kms();
        let src: Vec<u8> = (0..100u32).map(|v| v as u8).collect();
        let mut inplace = src.clone();
        ks.decrypt_at(&mut inplace, 0);

        let mut out = vec![0u8; src.len()];
        ks.decrypt_into(&src, &mut out, 0);
        assert_eq!(inplace, out);
    }

    #[test]
    fn key_byte_matches_stream() {
        let ks = AesKeyStream::gms();
        let stream = ks.keystream_prefix(128);
        for i in [0u64, 1, 15, 16, 17, 63, 100] {
            assert_eq!(ks.key_byte(i), stream[i as usize], "오프셋 {i}");
        }
    }

    #[test]
    fn noop_is_identity() {
        let n = NoOpCrypto;
        let mut data = *b"untouched";
        n.decrypt_at(&mut data, 999);
        assert_eq!(&data[..], b"untouched");
        assert!(n.is_identity());
        assert_eq!(n.key_byte(12345), 0);
    }

    #[test]
    fn noop_decrypt_into_copies() {
        let n = NoOpCrypto;
        let src = *b"copy me";
        let mut dst = [0u8; 7];
        n.decrypt_into(&src, &mut dst, 0);
        assert_eq!(&dst, &src);
    }

    #[test]
    fn bms_maps_to_none_and_kms_gms_map_to_some() {
        // BMS 는 키스트림이 0 이므로 None (호출부가 NoOpCrypto 사용)
        assert!(AesKeyStream::for_key_type(CryptoKeyType::Bms).is_none());
        assert!(AesKeyStream::for_key_type(CryptoKeyType::Kms).is_some());
        assert!(AesKeyStream::for_key_type(CryptoKeyType::Gms).is_some());
    }

    #[test]
    fn concurrent_decrypt_is_consistent() {
        let ks = std::sync::Arc::new(AesKeyStream::kms());
        let expect = ks.keystream_prefix(1024);
        let plain: Vec<u8> = (0..1024u32).map(|v| v as u8).collect();
        let expect_ct: Vec<u8> = plain
            .iter()
            .zip(expect.iter())
            .map(|(a, b)| a ^ b)
            .collect();

        std::thread::scope(|s| {
            for _ in 0..4 {
                let ks = std::sync::Arc::clone(&ks);
                let plain = plain.clone();
                let expect_ct = expect_ct.clone();
                s.spawn(move || {
                    for _ in 0..20 {
                        let mut buf = plain.clone();
                        ks.decrypt_at(&mut buf, 0);
                        assert_eq!(buf, expect_ct, "동시 복호화 결과가 달라지면 안 된다");
                    }
                });
            }
        });
    }
}
