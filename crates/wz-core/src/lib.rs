//! WZ 포맷 계층.
//!
//! # 책임
//!
//! **바이트 → 노드 트리.** 게임 규칙(WZ 타입, 아이템, 스킬 등)을 **모른다**.
//! 그 규칙은 상위 크레이트(`wz-chara` 등)의 책임이다 (리포트 §4.1 원칙 2).
//!
//! # 지원 포맷 (원본 `Compatibility/WzVersionProfile.cs:171-184`)
//!
//! | 컨테이너 | 세대 | 위치 |
//! |---|---|---|
//! | `Wz_File` PKG1 | `encver` 유무 | [`pkg1`] (구현 예정) |
//! | `Wz_File` PKG2 32-bit | v1~v5 (KMST1196~1200) | [`pkg2`] (구현 예정) |
//! | `Wz_File` PKG2 random-header | KMST1201/1202/1204/1205/1206 | [`pkg2`] (구현 예정) |
//! | `Ms_File` (MS v1) | `version == 2`, SNOW2 | [`ms`] (구현 예정) |
//! | `Ms_FileV2` (MS v2) | `version == 4`, ChaCha20 | [`ms`] (구현 예정) |
//!
//! # 진행 상태
//!
//! Phase 1 진행 중. 완료된 것:
//!
//! - ✅ [`crypto`] — AES-ECB 키스트림(BMS/KMS/GMS), PKG2 디렉터리 문자열 키,
//!   SNOW2, ChaCha20(카운터 리셋 포함)
//! - ✅ [`read`] — 압축 정수(LE128), 3종 문자열 인코딩, PKG2 i8/i16 변형
//! - ⏳ 컨테이너 파서 (`pkg1`/`pkg2`/`ms`) — 다음 스텝
//! - ⏳ 포맷 프로파일 탐지 (`detect`) — 다음 스텝

pub mod archive;
pub mod crypto;
pub mod detect;
pub mod detect_version;
pub mod image;
pub mod offset;
pub mod patch;
pub mod profile;
pub mod read;
pub mod sound;
pub mod text;

pub use archive::WzArchive;
pub use image::{ImageParser, ImageValue, Node, PngRef, Vec2};
pub use offset::{
    OffsetCalc, Pkg1OffsetCalc, Pkg2OffsetCalc32, Pkg2OffsetCalc64V1, Pkg2OffsetCalc64V2,
};
pub use patch::{compute_patch_checksum, PatchHeader, PatchPart, PatchPartType};
pub use sound::{decode_wave_format, extract_sound, ExtractedSound, SoundCodec, WaveFormat};
pub use wz_error as error;

#[cfg(test)]
mod tests {
    /// 통합 스모크 테스트: 암호 → 압축 정수/문자열 파이프라인이 서로 맞물리는지.
    #[test]
    fn crypto_and_read_pipeline_interoperate() {
        use crate::crypto::aes::AesKeyStream;
        use crate::crypto::Decryptor;
        use crate::read::{
            encode_compressed_i32, encode_utf16_wz_string, read_compressed_i32, read_image_string,
            StrEncoding,
        };

        let mut buf = Vec::new();
        buf.extend_from_slice(&encode_compressed_i32(-42));

        // 원본 규칙: 접두사는 평문, 페이로드만 키스트림 오프셋 0 부터 XOR.
        let mut s = encode_utf16_wz_string("00002000.img");
        let payload = &mut s[1..];
        AesKeyStream::kms().decrypt_at(payload, 0);
        buf.extend_from_slice(&s);

        let mut c = wz_pool::ReadCtx::from_bytes(buf);
        assert_eq!(read_compressed_i32(&mut c).unwrap(), -42);

        let got = read_image_string(&mut c, Some(&AesKeyStream::kms())).unwrap();
        assert_eq!(got.text, "00002000.img");
        assert_eq!(got.encoding, StrEncoding::Utf16);
        assert_eq!(c.pos(), c.len().unwrap(), "전부 소비돼야 한다");
    }
}
pub mod dirtree;
pub mod ms;
pub mod verify;
