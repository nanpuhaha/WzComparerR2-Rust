//! `WzComparerR2.Common/OpenAPI` 중 네트워크 없이 검증 가능한 부분.
//!
//! 원본 `NexonOpenAPI.Utils`의 `Decrypt`, `CheckVer`, `GetItemID`를 옮겼다.
//! API 키/HTTP 호출은 클라이언트 자격 증명이 필요해 대상에서 제외한다.

pub mod avatar;
pub mod http;

use aes::cipher::{BlockDecrypt, KeyInit};
use aes::Aes128;

const AES_KEY: [u8; 16] = [
    0x10, 0x04, 0x3F, 0x11, 0x17, 0xCD, 0x12, 0x15, 0x5D, 0x8E, 0x7A, 0x19, 0x80, 0x11, 0x4F, 0x14,
];
const IV: [u8; 16] = [
    0x11, 0x17, 0xCD, 0x10, 0x04, 0x3F, 0x8E, 0x7A, 0x12, 0x15, 0x80, 0x11, 0x5D, 0x19, 0x4F, 0x10,
];

const CHAR_TABLE: [&str; 8] = [
    "KL________",
    "FEHGBA____",
    "PONMLKJIHG",
    "CDABGHEFKL",
    "LKJIPONMDC",
    "HGFEDCBAPO",
    "OPMNKLIJGH",
    "BADCFEHGJI",
];

/// 원본 `Utils.Decrypt` — 16진 니블을 'A' 오프셋 문자로 인코딩한 문자열을
/// AES-128-CBC(패딩 없음)로 복호화한다.
pub fn decrypt(data: &str) -> Option<Vec<u8>> {
    if data.is_empty() {
        return None;
    }
    let bytes = data.as_bytes();
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let mut crypt = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.as_chunks::<2>().0 {
        let hi = pair[0].wrapping_sub(b'A');
        let lo = pair[1].wrapping_sub(b'A');
        crypt.push(hi.wrapping_shl(4) | lo);
    }
    if crypt.is_empty() || !crypt.len().is_multiple_of(16) {
        // 원본은 PaddingMode.None 이라 16배수가 아니면 예외가 난다.
        return None;
    }

    let cipher = Aes128::new((&AES_KEY).into());
    let mut prev = IV;
    let mut out = Vec::with_capacity(crypt.len());
    for chunk in crypt.as_chunks::<16>().0 {
        let mut block = aes::cipher::Block::<Aes128>::clone_from_slice(chunk);
        cipher.decrypt_block(&mut block);
        for i in 0..16 {
            out.push(block[i] ^ prev[i]);
        }
        prev.copy_from_slice(chunk);
    }
    Some(out)
}

/// 원본 `Utils.CheckVer`.
pub fn check_ver(pack: &[u8]) -> i32 {
    if pack.is_empty() {
        return 0;
    }
    i32::from(pack[pack.len() - pack.len() / 16 - 1])
}

/// 원본 `Utils.GetItemID`.
///
/// 원본은 8자보다 긴 문자열에서 `CharTable` 범위를 벗어나 예외를 던진다.
/// 여기서는 같은 조건을 `None`으로 표현한다.
pub fn get_item_id(text: &str) -> Option<String> {
    let mut id = String::new();
    for (count, c) in text.chars().enumerate() {
        let row = CHAR_TABLE.get(count)?;
        let idx = row.chars().position(|ch| ch == c).unwrap_or(0);
        id.push_str(&idx.to_string());
    }
    Some(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check_ver_reads_tail_index() {
        let pack = vec![0u8; 64];
        assert_eq!(check_ver(&pack), 0);
        let mut pack = vec![0u8; 64];
        pack[63 - 63 / 16 - 1] = 7;
        assert_eq!(check_ver(&pack), 7);
    }

    #[test]
    fn get_item_id_maps_char_table() {
        assert_eq!(get_item_id("KL").as_deref(), Some("00"));
        assert_eq!(get_item_id("FE").as_deref(), Some("01"));
        assert_eq!(get_item_id("??").as_deref(), Some("00"));
        assert_eq!(get_item_id("KLMNOPQRST"), None);
    }
}
