//! 검증된 ItemTooltip 조각들을 실제 RGBA 캔버스로 합성한다.
//!
//! 사용하는 검증 완료 부품:
//! - `wz_db::format_parser` — `#c`/`#t`/`#fc`/`#@` 토큰(원본 Parser와 일치)
//! - `wz_db::item_tooltip_layout` — `picH`/`splitterH`/`iconY` 누적
//! - `wz_db::pixel_ops` — 아이콘 2배 확대 + 배치 + straight-alpha source-over
//! - 게임 폰트 래스터(`draw_game_text`, GUI가 쓰는 것과 동일)
//!
//! 색상은 원본 `ItemTooltipRender22`의 `item22ColorTable`과 같은 값이다
//! (`c` #FFCC00, `$r` #FF8A18, `$g` #CCFF00, `$S` #77CCFF).
//!
//! GDI+의 premultiplied/감마는 재현하지 않는다. 여기서는 **결정적** 결과를
//! 만들어 해시로 고정하는 것이 목적이다.

use std::path::Path;
use wz_db::format_parser::{parse, DocElement};
use wz_db::item_tooltip_layout::{layout_item, line_height, wrapped_lines, ItemLayoutInput};
use wz_db::pixel_ops::{compose_icon_block_alpha, IconBlock};

use crate::loader::{draw_game_text, load_game_font};

/// 캔버스 폭(원본 `DefaultWidth`).
pub const CANVAS_WIDTH: u32 = 300;
/// 줄 간격(`LineHeight`).
pub const LINE_HEIGHT: i32 = 18;
/// 설명 접기 폭(레이아웃과 같은 규칙).
pub const WRAP_COLUMNS: usize = 40;

/// `ItemTooltipRender22`의 `item22ColorTable`.
pub fn color_for_id(id: &str) -> Option<[u8; 3]> {
    Some(match id {
        "c" => [0xFF, 0xCC, 0x00],
        "$r" => [0xFF, 0x8A, 0x18],
        "$g" => [0xCC, 0xFF, 0x00],
        "$S" => [0x77, 0xCC, 0xFF],
        hex if hex.len() == 8 && hex.chars().all(|c| c.is_ascii_hexdigit()) => {
            let r = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[6..8], 16).unwrap_or(0);
            [r, g, b]
        }
        _ => return None,
    })
}

#[derive(Debug, Clone, Default)]
pub struct ItemTooltipSpec<'a> {
    pub title: &'a str,
    /// 원본 `desc`(서식 문자열).
    pub desc: &'a str,
    /// `GetItemTopAttributeString`이 만든 태그들.
    pub top_attributes: &'a [String],
    /// `GetItemBottomAttributeString`이 만든 태그들.
    pub bottom_attributes: &'a [String],
    /// 유효기간 줄(`info/consumableFrom` 등).
    pub expire_lines: &'a [String],
    /// 생명의 물 줄.
    pub no_revive_lines: &'a [String],
    /// `(width, height, rgba)` — 아이콘 캔버스.
    pub icon: Option<(u32, u32, Vec<u8>)>,
    /// 아이콘 `origin`.
    pub origin: (i32, i32),
}

/// run 목록에서 표시 문자열만 뽑는다. 개행은 `\n`으로 보존한다.
fn plain_text(runs: &[DocElement]) -> String {
    let mut out = String::new();
    for run in runs {
        match run {
            DocElement::Span(span) => out.push_str(&span.text),
            DocElement::LineBreak => out.push('\n'),
        }
    }
    out
}

/// 폰트 비트맵을 `color`로 틴트해 알파 합성한다.
#[allow(clippy::too_many_arguments)]
fn blit_text(
    font: &fontdue::Font,
    text: &str,
    px: f32,
    canvas: &mut [u8],
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    color: [u8; 3],
) {
    let Some((tw, th, rgba)) = draw_game_text(font, text, px) else {
        return;
    };
    for gy in 0..th as i32 {
        let ty = y + gy;
        if ty < 0 || ty >= height as i32 {
            continue;
        }
        for gx in 0..tw as i32 {
            let tx = x + gx;
            if tx < 0 || tx >= width as i32 {
                continue;
            }
            let src = ((gy as u32 * tw + gx as u32) * 4) as usize;
            let alpha = u32::from(rgba[src + 3]);
            if alpha == 0 {
                continue;
            }
            let dst = ((ty as u32 * width + tx as u32) * 4) as usize;
            let inv = 255 - alpha;
            for c in 0..3 {
                let blended =
                    (u32::from(color[c]) * alpha + u32::from(canvas[dst + c]) * inv + 127) / 255;
                canvas[dst + c] = blended as u8;
            }
            canvas[dst + 3] = 255;
        }
    }
}

fn char_width(font: &fontdue::Font, ch: char, px: f32) -> f32 {
    font.metrics(ch, px).advance_width
}

/// run 목록을 색상별로 그린다. 40칸에서 접고 `\n`은 줄바꿈이다.
///
/// 그린 줄 수를 돌려준다.
#[allow(clippy::too_many_arguments)]
fn draw_runs(
    font: &fontdue::Font,
    runs: &[DocElement],
    px: f32,
    canvas: &mut [u8],
    width: u32,
    height: u32,
    x: i32,
    y: i32,
    default_color: [u8; 3],
) -> i32 {
    let mut line = 0i32;
    let mut col = 0usize;
    let mut cursor_x = 0f32;
    // 같은 색/줄 구간을 모아 한 번에 그린다.
    let mut segment = String::new();
    let mut segment_color = default_color;

    fn flush(
        font: &fontdue::Font,
        segment: &mut String,
        color: [u8; 3],
        px: f32,
        canvas: &mut [u8],
        width: u32,
        height: u32,
        x: i32,
        y: i32,
        line: i32,
        cursor_x: &mut f32,
    ) {
        if segment.is_empty() {
            return;
        }
        let advance: f32 = segment.chars().map(|c| char_width(font, c, px)).sum();
        blit_text(
            font,
            segment,
            px,
            canvas,
            width,
            height,
            x + *cursor_x as i32,
            y + line * LINE_HEIGHT,
            color,
        );
        *cursor_x += advance;
        segment.clear();
    }

    for run in runs {
        match run {
            DocElement::LineBreak => {
                flush(
                    font,
                    &mut segment,
                    segment_color,
                    px,
                    canvas,
                    width,
                    height,
                    x,
                    y,
                    line,
                    &mut cursor_x,
                );
                line += 1;
                col = 0;
                cursor_x = 0.0;
            }
            DocElement::Span(span) => {
                let color = color_for_id(&span.color_id).unwrap_or(default_color);
                for ch in span.text.chars() {
                    if col == WRAP_COLUMNS {
                        flush(
                            font,
                            &mut segment,
                            segment_color,
                            px,
                            canvas,
                            width,
                            height,
                            x,
                            y,
                            line,
                            &mut cursor_x,
                        );
                        line += 1;
                        col = 0;
                        cursor_x = 0.0;
                    }
                    if color != segment_color {
                        flush(
                            font,
                            &mut segment,
                            segment_color,
                            px,
                            canvas,
                            width,
                            height,
                            x,
                            y,
                            line,
                            &mut cursor_x,
                        );
                        segment_color = color;
                    }
                    segment.push(ch);
                    col += 1;
                }
            }
        }
    }
    flush(
        font,
        &mut segment,
        segment_color,
        px,
        canvas,
        width,
        height,
        x,
        y,
        line,
        &mut cursor_x,
    );
    line + 1
}

/// 한 태그 문자열의 줄 수(색 토큰 포함, 40칸 접기).
fn tag_lines(text: &str) -> i32 {
    wrapped_lines(&plain_text(&parse(text))).max(1)
}

/// ItemTooltip 캔버스를 만든다. 폰트가 없으면 `None`.
pub fn render_item_tooltip(
    data_root: &Path,
    spec: &ItemTooltipSpec<'_>,
) -> Option<(u32, u32, Vec<u8>)> {
    let font = load_game_font(data_root)?;

    let desc_runs = parse(spec.desc);
    let desc_text = plain_text(&desc_runs);
    let desc_lines = wrapped_lines(&desc_text);
    let top_heights: Vec<i32> = spec
        .top_attributes
        .iter()
        .map(|text| line_height(tag_lines(text)))
        .collect();
    let bottom_heights: Vec<i32> = spec
        .bottom_attributes
        .iter()
        .map(|text| line_height(tag_lines(text)))
        .collect();
    let layout = layout_item(&ItemLayoutInput {
        top_attr_heights: top_heights,
        expire: !spec.expire_lines.is_empty(),
        no_revive: !spec.no_revive_lines.is_empty(),
        desc_height: (desc_lines > 0).then(|| line_height(desc_lines)),
        bottom_attr_heights: bottom_heights,
        ..Default::default()
    });
    let height = layout.pic_h.max(60) as u32;
    let width = CANVAS_WIDTH;

    // 배경(원본 툴팁 배경색 계열) + 테두리
    let mut canvas = vec![0u8; (width * height * 4) as usize];
    for y in 0..height {
        for x in 0..width {
            let border = x < 2 || y < 2 || x + 2 >= width || y + 2 >= height;
            let color = if border {
                [90, 110, 150, 255]
            } else {
                [24, 26, 36, 255]
            };
            let off = ((y * width + x) * 4) as usize;
            canvas[off..off + 4].copy_from_slice(&color);
        }
    }

    // 구분선
    for split in &layout.splitter_h {
        let y = *split;
        if y < 0 || y >= height as i32 {
            continue;
        }
        for x in 3..width - 3 {
            let off = ((y as u32 * width + x) * 4) as usize;
            canvas[off..off + 4].copy_from_slice(&[90, 110, 150, 255]);
        }
    }

    // 제목
    blit_text(
        &font,
        spec.title,
        14.0,
        &mut canvas,
        width,
        height,
        12,
        10,
        [255, 255, 255],
    );

    // 상단 속성(원본은 picH=30부터)
    let mut y = 30;
    for text in spec.top_attributes {
        let lines = draw_runs(
            &font,
            &parse(text),
            12.0,
            &mut canvas,
            width,
            height,
            12,
            y,
            [255, 255, 255],
        );
        y += line_height(lines);
    }
    for text in spec.expire_lines {
        let lines = draw_runs(
            &font,
            &parse(text),
            12.0,
            &mut canvas,
            width,
            height,
            12,
            y,
            [255, 255, 255],
        );
        y += line_height(lines);
    }
    for text in spec.no_revive_lines {
        let lines = draw_runs(
            &font,
            &parse(text),
            12.0,
            &mut canvas,
            width,
            height,
            12,
            y,
            [255, 255, 255],
        );
        y += line_height(lines);
    }

    // 아이콘/본문
    let has_icon = spec.icon.is_some();
    let text_x = if has_icon { 96 } else { 12 };
    let mut text_y = layout.icon_y + 6;
    let lines = draw_runs(
        &font,
        &desc_runs,
        12.0,
        &mut canvas,
        width,
        height,
        text_x,
        text_y,
        [200, 200, 200],
    );
    text_y += line_height(lines);

    // 하단 속성(두 번째 splitter 아래 10px)
    if !spec.bottom_attributes.is_empty() {
        if let Some(second) = layout.splitter_h.get(1) {
            let mut by = second + 11;
            for text in spec.bottom_attributes {
                let lines = draw_runs(
                    &font,
                    &parse(text),
                    12.0,
                    &mut canvas,
                    width,
                    height,
                    15,
                    by,
                    [200, 200, 200],
                );
                by += line_height(lines);
            }
        }
    }
    let _ = text_y;

    // 아이콘 블록(검증된 확대 + 배치 + source-over)
    if let Some((iw, ih, rgba)) = &spec.icon {
        let block = IconBlock {
            rgba,
            width: *iw,
            height: *ih,
            origin: spec.origin,
        };
        canvas = compose_icon_block_alpha(&block, &canvas, width, height, 15, layout.icon_y);
    }

    Some((width, height, canvas))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_drops_tags_and_keeps_breaks() {
        let runs = parse("#c빨강#\n보통");
        assert_eq!(plain_text(&runs), "빨강\n보통");
        let runs = parse("#@1/10/10@x");
        assert_eq!(plain_text(&runs), "x");
    }

    #[test]
    fn color_table_matches_item22() {
        assert_eq!(color_for_id("c"), Some([0xFF, 0xCC, 0x00]));
        assert_eq!(color_for_id("$r"), Some([0xFF, 0x8A, 0x18]));
        assert_eq!(color_for_id("$g"), Some([0xCC, 0xFF, 0x00]));
        assert_eq!(color_for_id("$S"), Some([0x77, 0xCC, 0xFF]));
        assert_eq!(color_for_id("80FF00FF"), Some([0xFF, 0x00, 0xFF]));
        assert_eq!(color_for_id(""), None);
        assert_eq!(color_for_id("$x"), None);
    }

    #[test]
    fn tag_line_count_uses_wrapping() {
        assert_eq!(tag_lines("abc"), 1);
        assert_eq!(tag_lines("#c빨강#"), 1);
        assert_eq!(tag_lines(&format!("#c{}#", "x".repeat(41))), 2);
        assert_eq!(tag_lines("a\nb"), 2);
    }
}
