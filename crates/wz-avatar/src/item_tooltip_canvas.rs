//! 검증된 ItemTooltip 조각들을 실제 RGBA 캔버스로 합성한다.
//!
//! 사용하는 검증 완료 부품:
//! - `wz_db::format_parser` — `#c`/`#t`/`#fc`/`#@` 토큰 제거(원본 Parser와 일치)
//! - `wz_db::item_tooltip_layout` — `picH`/`splitterH`/`iconY` 누적
//! - `wz_db::pixel_ops` — 아이콘 2배 확대 + 배치 + straight-alpha source-over
//! - 게임 폰트 래스터(`draw_game_text`, GUI가 쓰는 것과 동일)
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
/// 설명 줄 간격(`LineHeight`).
pub const LINE_HEIGHT: i32 = 18;

pub struct ItemTooltipSpec<'a> {
    pub title: &'a str,
    /// 원본 `desc`(서식 문자열).
    pub desc: &'a str,
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

/// 아이템 설명 줄을 40칸 단위로 접는다(레이아웃과 같은 규칙).
fn wrap(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for line in text.split('\n') {
        let chars: Vec<char> = line.chars().collect();
        if chars.is_empty() {
            out.push(String::new());
            continue;
        }
        for chunk in chars.chunks(40) {
            out.push(chunk.iter().collect());
        }
    }
    out
}

/// ItemTooltip 캔버스를 만든다. 폰트가 없으면 `None`.
pub fn render_item_tooltip(
    data_root: &Path,
    spec: &ItemTooltipSpec<'_>,
) -> Option<(u32, u32, Vec<u8>)> {
    let font = load_game_font(data_root)?;

    let runs = parse(spec.desc);
    let text = plain_text(&runs);
    let lines = wrapped_lines(&text);
    let layout = layout_item(&ItemLayoutInput {
        desc_height: (lines > 0).then(|| line_height(lines)),
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

    // 제목/본문
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
    let has_icon = spec.icon.is_some();
    let text_x = if has_icon { 96 } else { 12 };
    let mut y = layout.icon_y + 6;
    for line in wrap(&text) {
        blit_text(
            &font,
            &line,
            12.0,
            &mut canvas,
            width,
            height,
            text_x,
            y,
            [200, 200, 200],
        );
        y += LINE_HEIGHT;
    }

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
    fn wrapping_is_40_columns() {
        assert_eq!(wrap("abc"), vec!["abc"]);
        assert_eq!(wrap(&"x".repeat(41)).len(), 2);
        assert_eq!(wrap("a\nb"), vec!["a", "b"]);
    }
}
