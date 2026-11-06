//! ItemTooltip 아이콘 블록의 straight-alpha source-over 합성을 C# 참조 구현과 비교한다.
//!
//! 배경은 fixture에 담긴 결정적 반투명 패턴이고, 아이콘은 실제 클라이언트
//! `_Canvas` 아이콘(27×30/27×27)을 확대해 배치식으로 올린다.

use serde::Deserialize;
use wz_db::pixel_ops::{compose_icon_block_alpha, IconBlock};

#[derive(Deserialize)]
struct Input {
    canvas: Canvas,
    background_b64: String,
    icons: Vec<Icon>,
}

#[derive(Deserialize)]
struct Canvas {
    width: u32,
    height: u32,
    icon_x: i32,
    pic_h: i32,
}

#[derive(Deserialize)]
struct Icon {
    path: String,
    width: u32,
    height: u32,
    origin: [i32; 2],
    rgba_b64: String,
}

#[derive(Deserialize)]
struct Expected {
    path: String,
    dx: i32,
    dy: i32,
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
fn icon_blend_matches_csharp_source_over() {
    let input: Input = serde_json::from_str(include_str!("fixtures/pixel_ops_input.json")).unwrap();
    let expected: Vec<Expected> =
        serde_json::from_str(include_str!("fixtures/icon_blend.json")).unwrap();
    assert_eq!(input.icons.len(), expected.len());
    let background = b64_decode(&input.background_b64);
    assert_eq!(
        background.len(),
        (input.canvas.width * input.canvas.height * 4) as usize
    );

    for (icon, want) in input.icons.iter().zip(expected.iter()) {
        assert_eq!(icon.path, want.path);
        let rgba = b64_decode(&icon.rgba_b64);
        let block = IconBlock {
            rgba: &rgba,
            width: icon.width,
            height: icon.height,
            origin: (icon.origin[0], icon.origin[1]),
        };
        assert_eq!(
            block.offset(input.canvas.icon_x, input.canvas.pic_h),
            (want.dx, want.dy)
        );
        let canvas = compose_icon_block_alpha(
            &block,
            &background,
            input.canvas.width,
            input.canvas.height,
            input.canvas.icon_x,
            input.canvas.pic_h,
        );
        assert_eq!(canvas.len(), want.out_len);
        assert_eq!(
            sha256_hex(&canvas),
            want.out_sha256,
            "blended pixels for {}",
            icon.path
        );
    }
}
