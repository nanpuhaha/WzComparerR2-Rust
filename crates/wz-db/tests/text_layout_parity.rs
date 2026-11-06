//! `TextRenderer.DrawPlainText` 레이아웃을 실제 C# 원본 루프와 비교한다.
//!
//! C# 하네스는 `WzComparerR2.Text.TextRenderer<string>`를 상속해 측정 함수만
//! `7 * UTF-16 길이`로 바꾼 뒤 `DrawPlainText`를 호출한다. 즉 줄바꿈/오프셋
//! 누적/정렬 보정은 **원본 코드 그 자체**다.

use serde::Deserialize;
use wz_db::text_layout::{draw_plain_text, Align, LineRect};

#[derive(Deserialize)]
struct Case {
    text: String,
    width: i32,
    start_y: i32,
    line_height: i32,
    alignment: String,
    rects: Vec<Rect>,
    end_y: i32,
}

#[derive(Deserialize, Debug, PartialEq, Eq)]
struct Rect {
    start: usize,
    len: usize,
    x: i32,
    y: i32,
}

fn measure(text: &str) -> i32 {
    7 * text.encode_utf16().count() as i32
}

fn align(name: &str) -> Align {
    match name {
        "Center" => Align::Center,
        "Right" => Align::Right,
        other => {
            assert_eq!(other, "Left");
            Align::Left
        }
    }
}

#[test]
fn layout_matches_csharp_text_renderer() {
    let cases: Vec<Case> = serde_json::from_str(include_str!("fixtures/text_layout.json")).unwrap();
    assert!(cases.len() >= 12);
    for case in cases {
        let (rects, end_y) = draw_plain_text(
            &case.text,
            case.width,
            case.start_y,
            case.line_height,
            align(&case.alignment),
            &measure,
        );
        let got: Vec<Rect> = rects
            .iter()
            .map(|r: &LineRect| Rect {
                start: r.start,
                len: r.len,
                x: r.x,
                y: r.y,
            })
            .collect();
        assert_eq!(
            got, case.rects,
            "text {:?} width {} align {}",
            case.text, case.width, case.alignment
        );
        assert_eq!(
            end_y, case.end_y,
            "end_y for {:?} width {}",
            case.text, case.width
        );
    }
}
