//! ItemTooltip 아이콘 블록 합성(불투명 blit)을 C# 바이트 연산과 비교한다.
//!
//! 입력은 실제 클라이언트 `Item/Consume/_Canvas` 아이콘 캔버스 RGBA + 원본
//! 파일의 `origin`이다. 좌표식은 `ItemTooltipRender22`의
//! `iconX + 6 + (1 - originX) * 2`, `picH + 6 + (33 - originY) * 2`를 쓴다.

use serde::Deserialize;
use wz_db::pixel_ops::{compose_icon_block, IconBlock};

#[derive(Deserialize)]
struct Input {
    canvas: Canvas,
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
    enlarged_width: u32,
    enlarged_height: u32,
    canvas_width: u32,
    canvas_height: u32,
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
fn icon_block_matches_csharp_blit() {
    let input: Input = serde_json::from_str(include_str!("fixtures/pixel_ops_input.json")).unwrap();
    let expected: Vec<Expected> =
        serde_json::from_str(include_str!("fixtures/icon_block.json")).unwrap();
    assert_eq!(input.icons.len(), expected.len());

    for (icon, want) in input.icons.iter().zip(expected.iter()) {
        assert_eq!(icon.path, want.path);
        let rgba = b64_decode(&icon.rgba_b64);
        let block = IconBlock {
            rgba: &rgba,
            width: icon.width,
            height: icon.height,
            origin: (icon.origin[0], icon.origin[1]),
        };
        let (dx, dy) = block.offset(input.canvas.icon_x, input.canvas.pic_h);
        assert_eq!((dx, dy), (want.dx, want.dy), "offset for {}", icon.path);
        assert_eq!(
            (icon.width * 2, icon.height * 2),
            (want.enlarged_width, want.enlarged_height)
        );

        let canvas = compose_icon_block(
            &block,
            input.canvas.width,
            input.canvas.height,
            input.canvas.icon_x,
            input.canvas.pic_h,
        );
        assert_eq!(
            (input.canvas.width, input.canvas.height),
            (want.canvas_width, want.canvas_height)
        );
        assert_eq!(canvas.len(), want.out_len, "length for {}", icon.path);
        assert_eq!(
            sha256_hex(&canvas),
            want.out_sha256,
            "composited pixels for {}",
            icon.path
        );
    }
}
