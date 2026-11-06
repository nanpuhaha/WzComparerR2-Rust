//! `ApplyFlip`/`ApplyRotation` CPU 베이킹을 C# 참조 구현과 비교한다.
//!
//! 입력은 실제 `Data/Character` 캔버스 6개의 픽셀/origin이고, 케이스는
//! flip 4조합 × 6 + 회전 5각도 × 6 = 54개다. 회전은 `rotation_bound`(f64 삼각함수,
//! `MathHelper.ToRadians`)와 역방향 최근접 샘플링(은행가 반올림)을 쓴다.

use serde::Deserialize;
use wz_avatar::{apply_flip, apply_rotation};

#[derive(Deserialize)]
struct Input {
    canvases: Vec<Canvas>,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Canvas {
    path: String,
    width: u32,
    height: u32,
    origin: [i32; 2],
    rgba_b64: String,
}

#[derive(Deserialize)]
struct Case {
    canvas: usize,
    kind: String,
    #[serde(default)]
    flip_x: bool,
    #[serde(default)]
    flip_y: bool,
    #[serde(default)]
    degrees: i32,
}

#[derive(Deserialize)]
struct Expected {
    canvas: usize,
    kind: String,
    width: u32,
    height: u32,
    origin: [i32; 2],
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

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

#[test]
fn frame_bake_matches_csharp() {
    let input: Input =
        serde_json::from_str(include_str!("fixtures/frame_bake_input.json")).unwrap();
    let expected: Vec<Expected> =
        serde_json::from_str(include_str!("fixtures/frame_bake_expected.json")).unwrap();
    assert_eq!(input.cases.len(), expected.len());
    assert!(input.cases.len() >= 50);

    let pixels: Vec<Vec<u8>> = input
        .canvases
        .iter()
        .map(|c| {
            let rgba = b64_decode(&c.rgba_b64);
            assert_eq!(rgba.len(), (c.width * c.height * 4) as usize, "{}", c.path);
            rgba
        })
        .collect();

    for (case, want) in input.cases.iter().zip(expected.iter()) {
        assert_eq!(case.canvas, want.canvas);
        assert_eq!(case.kind, want.kind);
        let canvas = &input.canvases[case.canvas];
        let baked = if case.kind == "flip" {
            apply_flip(
                &pixels[case.canvas],
                canvas.width,
                canvas.height,
                (canvas.origin[0], canvas.origin[1]),
                case.flip_x,
                case.flip_y,
            )
        } else {
            assert_eq!(case.kind, "rotate");
            apply_rotation(
                &pixels[case.canvas],
                canvas.width,
                canvas.height,
                (canvas.origin[0], canvas.origin[1]),
                case.degrees,
            )
        };
        assert_eq!(
            (baked.width, baked.height, baked.origin.0, baked.origin.1),
            (want.width, want.height, want.origin[0], want.origin[1]),
            "geometry for {} case {:?}",
            canvas.path,
            case.kind
        );
        assert_eq!(
            sha256_hex(&baked.rgba),
            want.sha256,
            "pixels for {} {:?}/{:?} degrees {}",
            canvas.path,
            case.kind,
            (case.flip_x, case.flip_y),
            case.degrees
        );
    }
}
