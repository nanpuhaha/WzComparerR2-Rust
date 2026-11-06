//! CPU 스프라이트 샘플링/합성을 C# 참조 구현과 비교한다.
//!
//! 입력은 실제 `Data/Character` 캔버스 6개의 픽셀과, 결정적으로 고른
//! AtlasRect 3종 × flip 2조합(블렌드는 케이스마다 교대)이다.

use serde::Deserialize;
use wz_db::draw_math::non_premultiplied_hidef;
use wz_db::sprite_compose::{compose_sprite, extract_rect};

#[derive(Deserialize)]
struct Fixture {
    canvases: Vec<Canvas>,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Canvas {
    path: String,
    width: u32,
    height: u32,
    rgba_b64: String,
}

#[derive(Deserialize)]
struct Case {
    canvas: usize,
    rect: [i32; 4],
    dest: [i32; 4],
    flip_h: bool,
    flip_v: bool,
    alpha: u8,
    blend: String,
}

#[derive(Deserialize)]
struct Expected {
    canvas: [u32; 2],
    rows: Vec<Row>,
}

#[derive(Deserialize)]
struct Row {
    canvas: usize,
    rect: [i32; 4],
    sha256: String,
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

#[test]
fn sprite_compose_matches_csharp() {
    let input: Fixture =
        serde_json::from_str(include_str!("fixtures/sprite_compose_input.json")).unwrap();
    let expected: Expected =
        serde_json::from_str(include_str!("fixtures/sprite_compose.json")).unwrap();
    assert_eq!(input.cases.len(), expected.rows.len());
    assert_eq!(input.canvases.len(), 6);
    assert_eq!(expected.canvas, [120, 120]);

    let pixels: Vec<(u32, u32, Vec<u8>)> = input
        .canvases
        .iter()
        .map(|c| {
            let rgba = b64_decode(&c.rgba_b64);
            assert_eq!(rgba.len(), (c.width * c.height * 4) as usize, "{}", c.path);
            (c.width, c.height, rgba)
        })
        .collect();

    for (case, row) in input.cases.iter().zip(expected.rows.iter()) {
        assert_eq!(case.canvas, row.canvas);
        assert_eq!(case.rect, row.rect);
        let (sw, sh, rgba) = &pixels[case.canvas];
        let (rw, rh, sprite) = extract_rect(
            rgba,
            *sw,
            *sh,
            (case.rect[0], case.rect[1], case.rect[2], case.rect[3]),
        )
        .unwrap();

        let mut canvas = vec![0u8; (expected.canvas[0] * expected.canvas[1] * 4) as usize];
        let blend = if case.blend == "opaque" {
            wz_db::draw_math::opaque_blend()
        } else {
            assert_eq!(case.blend, "non_premultiplied_hidef");
            non_premultiplied_hidef()
        };
        compose_sprite(
            &mut canvas,
            expected.canvas[0],
            expected.canvas[1],
            &sprite,
            rw,
            rh,
            (case.dest[0], case.dest[1], case.dest[2], case.dest[3]),
            case.flip_h,
            case.flip_v,
            case.alpha,
            &blend,
        )
        .unwrap();

        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&canvas);
        assert_eq!(
            format!("{:x}", hasher.finalize()),
            row.sha256,
            "canvas {} rect {:?} flip {}/{} blend {}",
            case.canvas,
            case.rect,
            case.flip_h,
            case.flip_v,
            case.blend
        );
    }
}
