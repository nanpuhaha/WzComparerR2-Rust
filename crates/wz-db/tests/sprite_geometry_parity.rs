//! `AnimationGraphics.Draw` / `FrameAnimationData`의 스프라이트 호출 기하를
//! C# 참조 구현과 비교한다.
//!
//! 입력은 실제 `Data/Character` 캔버스 120개(width/height/origin)이고,
//! C# 하네스는 같은 공식을 `double` 정밀도로 계산한다(원본 `Math.Sin/Cos`).

use serde::Deserialize;
use wz_db::draw_math::{
    flipped_origin, frame_sprite_call, multiply_rgb, non_premultiplied_hidef, opaque_blend,
    rotation_bound, sprite_destination_scaled, src_alpha_mask, BlendFactor, BlendFactors,
    BlendFunction,
};

#[derive(Deserialize)]
struct Fixture {
    frames: Vec<FrameIn>,
}

#[derive(Deserialize)]
struct FrameIn {
    path: String,
    width: i32,
    height: i32,
    origin: [i32; 2],
}

#[derive(Deserialize)]
struct Expected {
    rows: Vec<Row>,
    blends: Blends,
    sprite_call: SpriteCallOut,
}

#[derive(Deserialize)]
struct Row {
    path: String,
    destinations: Vec<Dest>,
    flips: Vec<Flip>,
    rotations: Vec<Rot>,
}

#[derive(Deserialize)]
struct Dest {
    px: i32,
    py: i32,
    scale: f32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

#[derive(Deserialize)]
struct Flip {
    flip_x: bool,
    flip_y: bool,
    x: i32,
    y: i32,
}

#[derive(Deserialize)]
struct Rot {
    angle: i32,
    width: i32,
    height: i32,
    origin_x: i32,
    origin_y: i32,
}

#[derive(Deserialize)]
struct Blends {
    non_premultiplied_hidef: BlendOut,
    opaque: BlendOut,
    src_alpha_mask: BlendOut,
    multiply_rgb: BlendOut,
}

#[derive(Deserialize)]
struct BlendOut {
    color_src: String,
    color_dst: String,
    alpha_src: String,
    alpha_dst: String,
    function: String,
}

#[derive(Deserialize)]
struct SpriteCallOut {
    rotation_radians: f32,
    scale: f32,
    flip_h: bool,
    flip_v: bool,
    alpha: u8,
}

/// `MathHelper.ToRadians`의 원본 float 상수(정밀도 경고는 의도된 값).
#[allow(clippy::excessive_precision)]
const TO_RADIANS: f32 = 0.0174532924f32;

fn factor(name: &str) -> BlendFactor {
    match name {
        "Zero" => BlendFactor::Zero,
        "One" => BlendFactor::One,
        "SourceAlpha" => BlendFactor::SourceAlpha,
        "InverseSourceAlpha" => BlendFactor::InverseSourceAlpha,
        "SourceColor" => BlendFactor::SourceColor,
        other => panic!("unknown factor {other}"),
    }
}

fn blend(out: &BlendOut) -> BlendFactors {
    assert_eq!(out.function, "Add");
    BlendFactors {
        color_src: factor(&out.color_src),
        color_dst: factor(&out.color_dst),
        alpha_src: factor(&out.alpha_src),
        alpha_dst: factor(&out.alpha_dst),
        function: BlendFunction::Add,
    }
}

#[test]
fn sprite_call_geometry_matches_csharp() {
    let input: Fixture =
        serde_json::from_str(include_str!("fixtures/sprite_frames_input.json")).unwrap();
    let expected: Expected =
        serde_json::from_str(include_str!("fixtures/sprite_geometry.json")).unwrap();
    assert_eq!(input.frames.len(), expected.rows.len());
    assert!(input.frames.len() >= 100);

    for (frame, row) in input.frames.iter().zip(expected.rows.iter()) {
        assert_eq!(frame.path, row.path);
        let size = (frame.width, frame.height);
        let origin = (frame.origin[0], frame.origin[1]);

        for dest in &row.destinations {
            assert_eq!(
                sprite_destination_scaled((dest.px, dest.py), origin, size, dest.scale),
                (dest.x, dest.y, dest.w, dest.h),
                "destination for {} at ({},{}) scale {}",
                frame.path,
                dest.px,
                dest.py,
                dest.scale
            );
        }

        for flip in &row.flips {
            assert_eq!(
                flipped_origin(size, origin, flip.flip_x, flip.flip_y),
                (flip.x, flip.y),
                "flip {}/{} for {}",
                flip.flip_x,
                flip.flip_y,
                frame.path
            );
        }

        for rot in &row.rotations {
            // 원본 `MathHelper.ToRadians`와 같은 float 상수.
            let radians = rot.angle as f32 * TO_RADIANS;
            let ((w, h), new_origin) = rotation_bound(size, radians, origin);
            assert_eq!(
                (w, h, new_origin.0, new_origin.1),
                (rot.width, rot.height, rot.origin_x, rot.origin_y),
                "rotation {} for {}",
                rot.angle,
                frame.path
            );
        }

        // `AnimationGraphics.Draw` 한 번의 호출 메타데이터.
        let call = frame_sprite_call(
            (20, 10),
            Some((0, 0, frame.width, frame.height)),
            origin,
            size,
            255,
        );
        assert_eq!(call.rotation_radians, expected.sprite_call.rotation_radians);
        assert_eq!(call.scale, expected.sprite_call.scale);
        assert_eq!(call.flip_h, expected.sprite_call.flip_h);
        assert_eq!(call.flip_v, expected.sprite_call.flip_v);
        assert_eq!(call.alpha, expected.sprite_call.alpha);
        assert_eq!(call.blend, non_premultiplied_hidef());
    }
}

#[test]
fn blend_states_match_csharp() {
    let expected: Expected =
        serde_json::from_str(include_str!("fixtures/sprite_geometry.json")).unwrap();
    assert_eq!(
        non_premultiplied_hidef(),
        blend(&expected.blends.non_premultiplied_hidef)
    );
    assert_eq!(opaque_blend(), blend(&expected.blends.opaque));
    assert_eq!(src_alpha_mask(), blend(&expected.blends.src_alpha_mask));
    assert_eq!(multiply_rgb(), blend(&expected.blends.multiply_rgb));
}
