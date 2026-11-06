//! GDI 비의존 드로잉 계산.
//!
//! - `GearGraphics.DrawGearDetailNumber`의 글리프 전진 규칙
//! - `AnimationGraphics.Draw`의 목적 사각형(위치·원점·소스 크기)
//! - `SpriteBatchEx.DrawPath`의 1px 사각형 변환
//! - `GearGraphics.GetBorderPath`/`GetIconBorderPath`

/// 숫자 하나가 그려지는 위치.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlyphDraw {
    pub ch: char,
    pub x: i32,
    pub y: i32,
    /// 이 글리프에 쓰인 리소스 비트맵 폭.
    pub width: i32,
    pub percent: bool,
}

/// `DrawGearDetailNumber`의 글리프 배치.
///
/// - 숫자: `(x, y)`에 그리고 `x += width + 1`
/// - `-`: `(x, y+3)`에 그리고 `x += width + 1`
/// - `%`: `(x+1, y)`에 그리고 `x += width + 2`
/// - 그 외 문자: 원본 switch에 `default`가 없어 **아무 것도 그리지 않고
///   전진도 하지 않는다**.
pub fn gear_detail_number_glyphs(
    num: &str,
    x: i32,
    y: i32,
    can: bool,
    width_of: &dyn Fn(char, bool) -> Option<i32>,
) -> Vec<GlyphDraw> {
    let mut cursor = x;
    let mut out = Vec::new();
    for ch in num.chars() {
        match ch {
            '0'..='9' => {
                let Some(width) = width_of(ch, can) else {
                    continue;
                };
                out.push(GlyphDraw {
                    ch,
                    x: cursor,
                    y,
                    width,
                    percent: false,
                });
                cursor += width + 1;
            }
            '-' => {
                let Some(width) = width_of(ch, can) else {
                    continue;
                };
                out.push(GlyphDraw {
                    ch,
                    x: cursor,
                    y: y + 3,
                    width,
                    percent: false,
                });
                cursor += width + 1;
            }
            '%' => {
                let Some(width) = width_of(ch, can) else {
                    continue;
                };
                out.push(GlyphDraw {
                    ch,
                    x: cursor + 1,
                    y,
                    width,
                    percent: true,
                });
                cursor += width + 2;
            }
            _ => {}
        }
    }
    out
}

/// `AnimationGraphics.Draw`의 목적 사각형.
///
/// `SpriteBatch.Draw(position: Zero, origin: frame.Origin, scale: 1)`에
/// 월드 변환(아이템 `Position` 평행이동)을 적용한 결과와 같다.
pub fn sprite_destination(
    position: (i32, i32),
    origin: (i32, i32),
    source_size: (i32, i32),
) -> (i32, i32, i32, i32) {
    (
        position.0 - origin.0,
        position.1 - origin.1,
        source_size.0,
        source_size.1,
    )
}

/// `SpriteBatchEx.DrawPath` — 폴리라인을 1px 두께 사각형으로 바꾼다.
///
/// 축이 맞지 않는(대각선) 구간은 사각형이 만들어지지 않고, 폭/높이가 0 이하인
/// 항목은 그리지 않는다.
pub fn path_rects(path: &[(i32, i32)]) -> Vec<(i32, i32, i32, i32)> {
    if path.is_empty() {
        return Vec::new();
    }
    let mut rects = vec![(0i32, 0i32, 0i32, 0i32); path.len()];
    for i in 0..path.len().saturating_sub(1) {
        let (x0, y0) = path[i];
        let (x1, y1) = path[i + 1];
        if x0 == x1 {
            let dy = y1 - y0;
            if dy > 0 {
                rects[i] = (x0, y0, 1, dy);
            } else if dy < 0 {
                rects[i] = (x0, y1 + 1, 1, -dy);
            }
        } else if y0 == y1 {
            let dx = x1 - x0;
            if dx > 0 {
                rects[i] = (x0, y0, dx, 1);
            } else if dx < 0 {
                rects[i] = (x1 + 1, y0, -dx, 1);
            }
        }
    }
    let first = path[0];
    let last = path[path.len() - 1];
    if first != last || path.len() == 1 {
        rects[path.len() - 1] = (last.0, last.1, 1, 1);
    }
    rects
        .into_iter()
        .filter(|(_, _, w, h)| *w > 0 && *h > 0)
        .collect()
}

/// `GearGraphics.GetBorderPath`.
pub fn border_path(dx: i32, width: i32, height: i32) -> Vec<(i32, i32)> {
    vec![
        (dx + 1, 0),
        (dx + 1, 1),
        (dx, 1),
        (dx, height - 2),
        (dx + 1, height - 2),
        (dx + 1, height - 1),
        (dx + width - 2, height - 1),
        (dx + width - 2, height - 2),
        (dx + width - 1, height - 2),
        (dx + width - 1, 1),
        (dx + width - 2, 1),
        (dx + width - 2, 0),
        (dx + 1, 0),
    ]
}

/// `GearGraphics.GetIconBorderPath`.
pub fn icon_border_path(x: i32, y: i32) -> Vec<(i32, i32)> {
    vec![
        (x + 32, y + 31),
        (x + 32, y),
        (x, y),
        (x, y + 32),
        (x + 31, y + 32),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn widths(ch: char, can: bool) -> Option<i32> {
        let base = match ch {
            '0'..='9' => 7,
            '-' => 6,
            '%' => 9,
            _ => return None,
        };
        Some(if can { base } else { base + 2 })
    }

    #[test]
    fn digit_advance_is_width_plus_one() {
        let glyphs = gear_detail_number_glyphs("12", 10, 5, true, &widths);
        assert_eq!(glyphs.len(), 2);
        assert_eq!((glyphs[0].x, glyphs[0].y), (10, 5));
        assert_eq!((glyphs[1].x, glyphs[1].y), (18, 5));
    }

    #[test]
    fn minus_and_percent_offsets() {
        let glyphs = gear_detail_number_glyphs("-7%", 0, 0, true, &widths);
        assert_eq!((glyphs[0].ch, glyphs[0].x, glyphs[0].y), ('-', 0, 3));
        assert_eq!((glyphs[1].ch, glyphs[1].x), ('7', 7));
        // %는 x+1에 그려지고 width+2 전진
        // '-' 6+1, '7' 7+1 → cursor 15, %는 x+1
        assert_eq!((glyphs[2].ch, glyphs[2].x), ('%', 16));
        assert!(glyphs[2].percent);
    }

    #[test]
    fn other_chars_are_ignored_without_advance() {
        let glyphs = gear_detail_number_glyphs("1a2", 0, 0, true, &widths);
        assert_eq!(glyphs.len(), 2);
        assert_eq!(glyphs[1].x, 8, "문자는 전진하지 않는다");
    }

    #[test]
    fn destination_subtracts_origin() {
        assert_eq!(
            sprite_destination((20, 10), (5, -3), (27, 30)),
            (15, 13, 27, 30)
        );
    }

    #[test]
    fn path_rects_handle_direction_and_close() {
        // L자: 오른쪽 5, 아래로 3
        // 끝점이 시작점과 다르면 마지막 점에 1x1 사각형이 추가된다.
        let rects = path_rects(&[(0, 0), (5, 0), (5, 3)]);
        assert_eq!(rects, vec![(0, 0, 5, 1), (5, 0, 1, 3), (5, 3, 1, 1)]);
        // 역방향
        let rects = path_rects(&[(5, 0), (0, 0), (0, -2)]);
        assert_eq!(rects, vec![(1, 0, 5, 1), (0, -1, 1, 2), (0, -2, 1, 1)]);
        // 닫힌 경로는 마지막 점 사각형이 추가되지 않는다
        let rects = path_rects(&[(0, 0), (4, 0), (0, 0)]);
        assert_eq!(rects, vec![(0, 0, 4, 1), (1, 0, 4, 1)]);
    }

    #[test]
    fn border_paths_are_baked() {
        let border = border_path(2, 10, 8);
        assert_eq!(border.len(), 13);
        assert_eq!(border[0], (3, 0));
        assert_eq!(border[12], (3, 0));
        let icon = icon_border_path(4, 5);
        assert_eq!(icon[0], (36, 36));
        assert_eq!(icon[4], (35, 37));
    }
}

// ---------------------------------------------------------------------------
// `AnimationGraphics.Draw` / `FrameAnimationData` 스프라이트 호출 기하
// ---------------------------------------------------------------------------

use crate::controls_input::round_half_even;
// ---------------------------------------------------------------------------

/// XNA `Blend` 인자.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendFactor {
    Zero,
    One,
    SourceAlpha,
    InverseSourceAlpha,
    SourceColor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendFunction {
    Add,
}

/// `BlendState` — 색/알파 블렌드 인자.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlendFactors {
    pub color_src: BlendFactor,
    pub color_dst: BlendFactor,
    pub alpha_src: BlendFactor,
    pub alpha_dst: BlendFactor,
    pub function: BlendFunction,
}

/// `StateEx.NonPremultipled_Hidef` — `AnimationGraphics`가 항상 쓰는 상태.
pub fn non_premultiplied_hidef() -> BlendFactors {
    BlendFactors {
        color_src: BlendFactor::SourceAlpha,
        color_dst: BlendFactor::InverseSourceAlpha,
        alpha_src: BlendFactor::One,
        alpha_dst: BlendFactor::InverseSourceAlpha,
        function: BlendFunction::Add,
    }
}

/// `BlendState.Opaque`.
pub fn opaque_blend() -> BlendFactors {
    BlendFactors {
        color_src: BlendFactor::One,
        color_dst: BlendFactor::Zero,
        alpha_src: BlendFactor::One,
        alpha_dst: BlendFactor::Zero,
        function: BlendFunction::Add,
    }
}

/// `StateEx.SrcAlphaMask`.
pub fn src_alpha_mask() -> BlendFactors {
    BlendFactors {
        color_src: BlendFactor::Zero,
        color_dst: BlendFactor::InverseSourceAlpha,
        alpha_src: BlendFactor::Zero,
        alpha_dst: BlendFactor::InverseSourceAlpha,
        function: BlendFunction::Add,
    }
}

/// `StateEx.MultiplyRGB`.
pub fn multiply_rgb() -> BlendFactors {
    BlendFactors {
        color_src: BlendFactor::Zero,
        color_dst: BlendFactor::SourceColor,
        alpha_src: BlendFactor::Zero,
        alpha_dst: BlendFactor::One,
        function: BlendFunction::Add,
    }
}

/// 한 번의 `SpriteBatch.Draw` 호출.
#[derive(Debug, Clone, PartialEq)]
pub struct SpriteCall {
    /// `AtlasRect`(없으면 텍스처 전체).
    pub source: Option<(i32, i32, i32, i32)>,
    /// `position - origin*scale`과 `size*scale`.
    pub destination: (i32, i32, i32, i32),
    pub origin: (i32, i32),
    pub rotation_radians: f32,
    pub scale: f32,
    pub flip_h: bool,
    pub flip_v: bool,
    /// `frame.A0`.
    pub alpha: u8,
    pub blend: BlendFactors,
}

/// `position - origin*scale`, `size*scale` (반올림은 banker's).
pub fn sprite_destination_scaled(
    position: (i32, i32),
    origin: (i32, i32),
    source_size: (i32, i32),
    scale: f32,
) -> (i32, i32, i32, i32) {
    (
        round_half_even(position.0 as f32 - origin.0 as f32 * scale),
        round_half_even(position.1 as f32 - origin.1 as f32 * scale),
        round_half_even(source_size.0 as f32 * scale),
        round_half_even(source_size.1 as f32 * scale),
    )
}

/// `AnimationGraphics.Draw(FrameAnimator|MultiFrameAnimator)`의 호출 하나.
///
/// 원본은 회전 0, 배율 1, `SpriteEffects.None`, `Color.White * A0`,
/// `NonPremultipled_Hidef`로 그린다.
pub fn frame_sprite_call(
    position: (i32, i32),
    source: Option<(i32, i32, i32, i32)>,
    origin: (i32, i32),
    size: (i32, i32),
    alpha: u8,
) -> SpriteCall {
    SpriteCall {
        source,
        destination: sprite_destination_scaled(position, origin, size, 1.0),
        origin,
        rotation_radians: 0.0,
        scale: 1.0,
        flip_h: false,
        flip_v: false,
        alpha,
        blend: non_premultiplied_hidef(),
    }
}

/// `FrameAnimationData.ApplyFlip`의 새 origin.
pub fn flipped_origin(
    texture_size: (i32, i32),
    origin: (i32, i32),
    flip_x: bool,
    flip_y: bool,
) -> (i32, i32) {
    (
        if flip_x {
            texture_size.0 - origin.0
        } else {
            origin.0
        },
        if flip_y {
            texture_size.1 - origin.1
        } else {
            origin.1
        },
    )
}

/// `FrameAnimationData.GetRotationBound` — 회전 후 경계와 새 origin.
///
/// `((new_w, new_h), (origin_x, origin_y))`.
pub fn rotation_bound(
    source_size: (i32, i32),
    radians: f32,
    origin: (i32, i32),
) -> ((i32, i32), (i32, i32)) {
    // C# `Math.Sin/Cos(double)`와 같은 정밀도를 쓰기 위해 f64로 계산한다.
    let sin = f64::from(radians).sin();
    let cos = f64::from(radians).cos();
    let pts = [
        (f64::from(-origin.0), f64::from(-origin.1)),
        (f64::from(source_size.0 - origin.0), f64::from(-origin.1)),
        (f64::from(-origin.0), f64::from(source_size.1 - origin.1)),
        (
            f64::from(source_size.0 - origin.0),
            f64::from(source_size.1 - origin.1),
        ),
    ];
    let mut min_x = f64::INFINITY;
    let mut min_y = f64::INFINITY;
    let mut max_x = f64::NEG_INFINITY;
    let mut max_y = f64::NEG_INFINITY;
    for (x, y) in pts {
        let rx = x * cos - y * sin;
        let ry = x * sin + y * cos;
        min_x = min_x.min(rx);
        min_y = min_y.min(ry);
        max_x = max_x.max(rx);
        max_y = max_y.max(ry);
    }
    (
        ((max_x - min_x).ceil() as i32, (max_y - min_y).ceil() as i32),
        ((-min_x).ceil() as i32, (-min_y).ceil() as i32),
    )
}

#[cfg(test)]
mod frame_geometry_tests {
    use super::*;

    #[test]
    fn destination_uses_scale_and_origin() {
        assert_eq!(
            sprite_destination_scaled((20, 10), (5, -3), (27, 30), 2.0),
            (10, 16, 54, 60)
        );
    }

    #[test]
    fn frame_call_is_unrotated_and_non_premultiplied() {
        let call = frame_sprite_call((40, 30), Some((0, 0, 27, 30)), (13, 20), (27, 30), 255);
        assert_eq!(call.destination, (27, 10, 27, 30));
        assert_eq!(call.rotation_radians, 0.0);
        assert_eq!(call.scale, 1.0);
        assert!(!call.flip_h && !call.flip_v);
        assert_eq!(call.blend, non_premultiplied_hidef());
        assert_eq!(call.alpha, 255);
    }

    #[test]
    fn flip_mirrors_origin() {
        assert_eq!(flipped_origin((27, 30), (13, 20), true, false), (14, 20));
        assert_eq!(flipped_origin((27, 30), (13, 20), false, true), (13, 10));
        assert_eq!(flipped_origin((27, 30), (13, 20), true, true), (14, 10));
    }

    #[test]
    fn rotation_bound_matches_known_values() {
        // 90도: 폭/높이가 뒤집히고 origin도 회전한다.
        let ((w, h), origin) = rotation_bound((27, 30), std::f32::consts::FRAC_PI_2, (13, 20));
        // float π/2의 cos는 -4.37e-8이라 경계가 1px씩 밀린다(원본도 동일).
        assert_eq!((w, h), (31, 28));
        assert_eq!(origin, (11, 14));
        // 0도는 그대로.
        let ((w, h), origin) = rotation_bound((27, 30), 0.0, (13, 20));
        assert_eq!((w, h), (27, 30));
        assert_eq!(origin, (13, 20));
    }
}

/// `RenderHelper.DrawV6SkillDotline`의 브러시 타일 사각형.
///
/// 원본은 `TextureBrush(picCenter)`를 `(x1, y)`로 평행이동한 뒤
/// `FillRectangle(x1, y, x2 - x1, picCenter.Height)`를 채운다. 즉 점선은
/// `x1`에서 시작해 비트맵 폭 간격으로 타일링되고, 오른쪽 끝은 `x2`에서 잘린다.
///
/// 비트맵(`dot_width`/`dot_height`)은 GDI+ 리소스라 호출자가 제공한다.
pub fn skill_dotline_tiles(
    x1: i32,
    x2: i32,
    y: i32,
    dot_width: i32,
    dot_height: i32,
) -> Vec<(i32, i32, i32, i32)> {
    if x2 <= x1 || dot_width <= 0 || dot_height <= 0 {
        return Vec::new();
    }
    let mut tiles = Vec::new();
    let mut x = x1;
    while x < x2 {
        let width = (x2 - x).min(dot_width);
        tiles.push((x, y, width, dot_height));
        x += dot_width;
    }
    tiles
}

#[cfg(test)]
mod dotline_tests {
    use super::*;

    #[test]
    fn tiles_from_x1_and_clip_at_x2() {
        assert_eq!(
            skill_dotline_tiles(12, 22, 5, 4, 1),
            vec![(12, 5, 4, 1), (16, 5, 4, 1), (20, 5, 2, 1)]
        );
    }

    #[test]
    fn exact_multiple_has_no_partial_tile() {
        assert_eq!(
            skill_dotline_tiles(0, 8, 0, 4, 2),
            vec![(0, 0, 4, 2), (4, 0, 4, 2)]
        );
    }

    #[test]
    fn empty_or_invalid_inputs_produce_nothing() {
        assert!(skill_dotline_tiles(10, 10, 0, 4, 1).is_empty());
        assert!(skill_dotline_tiles(12, 11, 0, 4, 1).is_empty());
        assert!(skill_dotline_tiles(0, 10, 0, 0, 1).is_empty());
        assert!(skill_dotline_tiles(0, 10, 0, 4, 0).is_empty());
    }
}
