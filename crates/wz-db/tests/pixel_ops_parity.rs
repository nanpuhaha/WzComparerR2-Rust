//! ItemTooltip 아이콘 확대(`GearGraphics.EnlargeBitmap`) 픽셀 비교.
//!
//! 입력은 실제 클라이언트 `Item/Consume/_Canvas`의 아이콘 캔버스를 디코드한
//! RGBA이고, C# 하네스는 원본 바이트 루프를 그대로 옮긴 참조 구현으로 확대해
//! SHA-256을 낸다.

use serde::Deserialize;
use wz_db::pixel_ops::enlarge_2x;

#[derive(Deserialize)]
struct Input {
    icons: Vec<Icon>,
}

#[derive(Deserialize)]
struct Icon {
    path: String,
    width: u32,
    height: u32,
    rgba_b64: String,
}

#[derive(Deserialize)]
struct Expected {
    path: String,
    width: u32,
    height: u32,
    out_width: u32,
    out_height: u32,
    out_len: usize,
    out_sha256: String,
}

fn b64_decode(text: &str) -> Vec<u8> {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::new();
    let mut acc = 0u32;
    let mut bits = 0u32;
    for byte in text.bytes().filter(|b| *b != b'=') {
        let value = T.iter().position(|c| *c == byte).expect("base64") as u32;
        acc = (acc << 6) | value;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
        }
    }
    out
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[test]
fn enlarge_matches_csharp_bytes() {
    let input: Input = serde_json::from_str(include_str!("fixtures/pixel_ops_input.json")).unwrap();
    let expected: Vec<Expected> =
        serde_json::from_str(include_str!("fixtures/pixel_ops.json")).unwrap();
    assert_eq!(input.icons.len(), expected.len());

    for (icon, want) in input.icons.iter().zip(expected.iter()) {
        assert_eq!(icon.path, want.path);
        assert_eq!((icon.width, icon.height), (want.width, want.height));
        let rgba = b64_decode(&icon.rgba_b64);
        assert_eq!(
            rgba.len(),
            (icon.width * icon.height * 4) as usize,
            "input size for {}",
            icon.path
        );
        let out = enlarge_2x(&rgba, icon.width, icon.height);
        assert_eq!(out.len(), want.out_len, "length for {}", icon.path);
        assert_eq!(
            (icon.width * 2, icon.height * 2),
            (want.out_width, want.out_height)
        );
        assert_eq!(
            sha256_hex(&out),
            want.out_sha256,
            "pixels for {} ({}x{})",
            icon.path,
            icon.width,
            icon.height
        );
    }
}
