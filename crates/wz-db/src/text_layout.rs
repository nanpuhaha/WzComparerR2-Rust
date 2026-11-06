//! `WzComparerR2.Text.TextRenderer.DrawPlainText`의 결정적 레이아웃 코어.
//!
//! 원본은 `Graphics.MeasureString`으로 폭을 재지만, 여기서는 **측정 함수를
//! 주입**받아 순수 계산만 한다. 줄바꿈/오프셋 누적/정렬 보정/문자 단위 분할은
//! 원본 `LayoutRuns`를 그대로 옮겼다.
//!
//! 원본 `TextRenderer`는 인덱스를 UTF-16 코드 유닛으로 다루므로 여기서도
//! `Vec<u16>` 위에서 동작한다.

/// 정렬. C# `TextAlignment`와 같은 값.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

/// `Flush`가 만드는 한 줄 조각.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LineRect {
    pub start: usize,
    pub len: usize,
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    Text,
    WhiteSpace,
    BreakLine,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Run {
    start: usize,
    len: usize,
    kind: Kind,
    x: i32,
    width: i32,
}

/// `StringReader.ReadLine`과 같은 줄 분리(`\r\n`, `\r`, `\n`).
fn read_lines(text: &[u16]) -> Vec<Vec<u16>> {
    let mut lines = Vec::new();
    let mut i = 0usize;
    while i < text.len() {
        let start = i;
        while i < text.len() && text[i] != b'\n' as u16 && text[i] != b'\r' as u16 {
            i += 1;
        }
        lines.push(text[start..i].to_vec());
        if i < text.len() {
            if text[i] == b'\r' as u16 && i + 1 < text.len() && text[i + 1] == b'\n' as u16 {
                i += 2;
            } else {
                i += 1;
            }
        }
    }
    lines
}

fn prepare_runs(text: &[u16]) -> (Vec<u16>, Vec<Run>) {
    let mut sb: Vec<u16> = Vec::new();
    let mut runs = Vec::new();
    for (row, line) in read_lines(text).into_iter().enumerate() {
        if row > 0 {
            runs.push(Run {
                start: sb.len(),
                len: 0,
                kind: Kind::BreakLine,
                x: 0,
                width: 0,
            });
        }
        if !line.is_empty() {
            let start = sb.len();
            sb.extend_from_slice(&line);
            runs.push(Run {
                start,
                len: line.len(),
                kind: Kind::Text,
                x: 0,
                width: 0,
            });
        }
    }
    (sb, runs)
}

fn split_words(sb: &[u16], run: &Run) -> Vec<Run> {
    if !matches!(run.kind, Kind::Text) {
        return vec![run.clone()];
    }
    let mut out = Vec::new();
    let i0 = run.start + run.len;
    let mut i = run.start;
    while i < i0 {
        let start = i;
        match sb[i] {
            c if c == b' ' as u16 || c == b'\t' as u16 => {
                loop {
                    i += 1;
                    if i >= i0 {
                        break;
                    }
                    if !(sb[i] == b' ' as u16 || sb[i] == b'\t' as u16) {
                        break;
                    }
                }
                let len = i - start;
                out.push(Run {
                    start,
                    len,
                    kind: Kind::WhiteSpace,
                    x: 0,
                    width: 0,
                });
                i = i.wrapping_sub(1);
            }
            c if c == b'\r' as u16 => {
                if i + 1 < i0 && sb[i + 1] == b'\n' as u16 {
                    i += 1;
                    let len = i - start + 1;
                    out.push(Run {
                        start,
                        len,
                        kind: Kind::BreakLine,
                        x: 0,
                        width: 0,
                    });
                } else {
                    out.push(Run {
                        start,
                        len: 1,
                        kind: Kind::WhiteSpace,
                        x: 0,
                        width: 0,
                    });
                }
            }
            c if c == b'\n' as u16 => {
                let len = i - start + 1;
                out.push(Run {
                    start,
                    len,
                    kind: Kind::BreakLine,
                    x: 0,
                    width: 0,
                });
            }
            _ => out.push(Run {
                start,
                len: 1,
                kind: Kind::Text,
                x: 0,
                width: 0,
            }),
        }
        i += 1;
    }
    out
}

fn slice(sb: &[u16], start: usize, len: usize) -> String {
    String::from_utf16_lossy(&sb[start..start + len])
}

/// `DrawPlainText` — `(줄 조각들, 끝 y)`.
pub fn draw_plain_text(
    text: &str,
    width: i32,
    start_y: i32,
    line_height: i32,
    alignment: Align,
    measure: &dyn Fn(&str) -> i32,
) -> (Vec<LineRect>, i32) {
    let units: Vec<u16> = text.encode_utf16().collect();
    let (sb, prepared) = prepare_runs(&units);
    let mut runs: Vec<Run> = Vec::new();
    for run in &prepared {
        runs.extend(split_words(&sb, run));
    }
    // MeasureRuns
    let mut x = 0i32;
    for run in runs.iter_mut() {
        if run.kind == Kind::BreakLine {
            run.x = x;
            run.len = 0;
            continue;
        }
        run.x = x;
        run.width = measure(&slice(&sb, run.start, run.len));
        x += run.width;
    }
    layout(&runs, &sb, width, start_y, line_height, alignment, measure)
}

fn measure_chars(
    sb: &[u16],
    start: usize,
    len: usize,
    measure: &dyn Fn(&str) -> i32,
) -> Vec<(i32, i32)> {
    let mut out = Vec::with_capacity(len);
    let mut x = 0i32;
    for i in 0..len {
        let w = measure(&slice(sb, start + i, 1));
        out.push((x, x + w));
        x += w;
    }
    out
}

#[allow(clippy::too_many_arguments, unused_assignments)]
fn layout(
    runs: &[Run],
    sb: &[u16],
    width: i32,
    start_y: i32,
    height: i32,
    alignment: Align,
    measure: &dyn Fn(&str) -> i32,
) -> (Vec<LineRect>, i32) {
    let mut draw_x = 0i32;
    let mut draw_y = start_y;
    let mut start: i64 = -1;
    let mut end: i64 = -1;
    let mut x_offset = 0i32;
    let mut cur_x = draw_x;
    let mut result: Vec<LineRect> = Vec::new();
    let mut last_line_start = 0usize;
    let mut base_draw_y = draw_y;
    // 이미지 런을 아직 다루지 않으므로 값만 유지한다(원본은 이미지에서 사용).

    macro_rules! flush {
        ($newline:expr) => {{
            if start > -1 && end > start {
                result.push(LineRect {
                    start: start as usize,
                    len: (end - start) as usize,
                    x: draw_x,
                    y: draw_y,
                });
            }
            if $newline {
                if last_line_start < result.len() && alignment != Align::Left {
                    let content_width = cur_x;
                    let adjust = match alignment {
                        Align::Center => (width - content_width) / 2,
                        Align::Right => width - content_width,
                        Align::Left => 0,
                    };
                    for item in result.iter_mut().skip(last_line_start) {
                        item.x += adjust;
                    }
                }
                draw_x = 0;
                cur_x = 0;
                draw_y += height;
                last_line_start = result.len();
                base_draw_y = draw_y;
            } else {
                draw_x = cur_x;
            }
            // 원본 `flush` 마지막 줄: start/end 초기화
            start = -1;
            end = -1;
        }};
    }

    let mut r = 0usize;
    while r < runs.len() {
        let mut run = &runs[r];
        if run.kind == Kind::BreakLine {
            flush!(true);
            if r < runs.len() - 1 {
                x_offset = runs[r + 1].x;
            }
            r += 1;
            continue;
        }

        if start < 0 {
            start = run.start as i64;
        }
        if !(run.kind == Kind::WhiteSpace && run.width <= 0) {
            cur_x = run.x - x_offset;
            if cur_x >= width {
                if cur_x > 0 {
                    // 줄 끝 구두점은 현재 줄에 붙인다.
                    if run.len == 1 {
                        let ch = sb[run.start];
                        if ch == b',' as u16 || ch == b'.' as u16 {
                            end = (run.start + run.len) as i64;
                            r += 1;
                            if r >= runs.len() {
                                break;
                            }
                            run = &runs[r];
                        }
                    }
                    flush!(true);
                    if r < runs.len() {
                        start = run.start as i64;
                    }
                    x_offset = run.x;
                }

                if width - cur_x < run.width {
                    let rects = measure_chars(sb, run.start, run.len, measure);
                    let mut i = 0usize;
                    let mut ir = run.start;
                    while i < rects.len() {
                        let (_, right) = rects[i];
                        let right = right + run.x;
                        if start < 0 {
                            start = ir as i64;
                            x_offset = run.x;
                        }
                        if right - x_offset > width {
                            if ir as i64 - start <= 0 {
                                end = start + 1;
                                cur_x = right - x_offset;
                                flush!(true);
                                x_offset = right;
                                i += 1;
                                ir += 1;
                                continue;
                            } else {
                                end = ir as i64;
                                cur_x = rects[i].0 + run.x - x_offset;
                                flush!(true);
                                start = ir as i64;
                                x_offset = rects[i].0 + run.x;
                            }
                        }
                        i += 1;
                        ir += 1;
                    }
                    end = (run.start + run.len) as i64;
                    cur_x = rects.last().map(|(_, right)| right + run.x).unwrap_or(0) - x_offset;
                    flush!(false);
                    r += 1;
                    continue;
                }
            }
            end = (run.start + run.len) as i64;
            cur_x = run.x + run.width - x_offset;
        }
        r += 1;
    }
    flush!(true);
    let _ = base_draw_y;
    (result, draw_y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn measure(s: &str) -> i32 {
        7 * s.encode_utf16().count() as i32
    }

    #[test]
    fn single_line_no_wrap() {
        let (rects, y) = draw_plain_text("hello world", 1000, 0, 16, Align::Left, &measure);
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].start, 0);
        assert_eq!(rects[0].len, 11);
        assert_eq!(y, 16);
    }

    #[test]
    fn wraps_by_char_because_word_wrap_is_disabled() {
        // GearGraphics는 `WordWrapEnabled = false`라서 단어가 아니라 글자 단위로
        // 넘긴다. 7px/char, width=50 → 8글자(56px)까지 넣고 다음 글자에서 줄바꿈.
        let (rects, y) = draw_plain_text("hello world", 50, 0, 16, Align::Left, &measure);
        assert_eq!(rects.len(), 2);
        assert_eq!((rects[0].start, rects[0].len, rects[0].y), (0, 8, 0));
        assert_eq!((rects[1].start, rects[1].len, rects[1].y), (8, 3, 16));
        assert_eq!(y, 32);
    }

    #[test]
    fn breaks_long_word_by_char() {
        let (rects, _) = draw_plain_text("abcdefghij", 21, 0, 16, Align::Left, &measure);
        assert!(rects.len() >= 3);
        assert_eq!(rects[0].len, 3, "7px per char, 21px width");
    }

    #[test]
    fn explicit_newlines_advance_y() {
        let (rects, y) = draw_plain_text("ab\ncd", 1000, 5, 16, Align::Left, &measure);
        assert_eq!(rects.len(), 2);
        assert_eq!(rects[1].y, 21);
        assert_eq!(y, 37);
    }
}
