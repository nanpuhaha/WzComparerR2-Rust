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
