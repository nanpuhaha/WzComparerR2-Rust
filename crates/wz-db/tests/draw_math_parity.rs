//! GDI 비의존 드로잉 계산을 C# 참조 구현과 비교한다.
//!
//! C# 하네스는 `DrawGearDetailNumber`의 전진 루프, `AnimationGraphics.Draw`의
//! 목적 사각형, `SpriteBatchEx.DrawPath`, `GetBorderPath`/`GetIconBorderPath`를
//! 그대로 옮겼고, 사각형 판정에는 실제 `System.Drawing.Rectangle`을 쓴다.

use serde::Deserialize;
use wz_db::draw_math::{
    border_path, gear_detail_number_glyphs, icon_border_path, path_rects, sprite_destination,
};

#[derive(Deserialize)]
struct Fixture {
    glyphs: Vec<GlyphRow>,
    destinations: Vec<DestRow>,
    paths: Vec<PathRow>,
    borders: Vec<BorderRow>,
    icon_borders: Vec<IconBorderRow>,
}

#[derive(Deserialize)]
struct GlyphRow {
    num: String,
    can: bool,
    glyphs: Vec<Glyph>,
}

#[derive(Deserialize)]
struct Glyph {
    ch: String,
    x: i32,
    y: i32,
    w: i32,
    percent: bool,
}

#[derive(Deserialize)]
struct DestRow {
    px: i32,
    py: i32,
    ox: i32,
    oy: i32,
    sw: i32,
    sh: i32,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

#[derive(Deserialize)]
struct Point {
    x: i32,
    y: i32,
}

#[derive(Deserialize)]
struct Rect {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

#[derive(Deserialize)]
struct PathRow {
    path: Vec<Point>,
    rects: Vec<Rect>,
}

#[derive(Deserialize)]
struct BorderRow {
    dx: i32,
    width: i32,
    height: i32,
    points: Vec<Point>,
}

#[derive(Deserialize)]
struct IconBorderRow {
    x: i32,
    y: i32,
    points: Vec<Point>,
}

fn points(list: &[Point]) -> Vec<(i32, i32)> {
    list.iter().map(|p| (p.x, p.y)).collect()
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/draw_math.json")).unwrap()
}

fn width_of(ch: char, can: bool) -> Option<i32> {
    let base = match ch {
        '0'..='9' => 7,
        '-' => 6,
        '%' => 9,
        _ => return None,
    };
    Some(if can { base } else { base + 2 })
}

#[test]
fn gear_detail_number_advance_matches_csharp() {
    for row in fixture().glyphs {
        let got = gear_detail_number_glyphs(&row.num, 3, 4, row.can, &width_of);
        let want: Vec<(String, i32, i32, i32, bool)> = row
            .glyphs
            .iter()
            .map(|g| (g.ch.clone(), g.x, g.y, g.w, g.percent))
            .collect();
        let actual: Vec<(String, i32, i32, i32, bool)> = got
            .iter()
            .map(|g| (g.ch.to_string(), g.x, g.y, g.width, g.percent))
            .collect();
        assert_eq!(actual, want, "num {:?} can {}", row.num, row.can);
    }
}

#[test]
fn sprite_destination_matches_csharp() {
    for row in fixture().destinations {
        assert_eq!(
            sprite_destination((row.px, row.py), (row.ox, row.oy), (row.sw, row.sh)),
            (row.x, row.y, row.w, row.h)
        );
    }
}

#[test]
fn draw_path_rects_match_csharp() {
    for row in fixture().paths {
        let path: Vec<(i32, i32)> = row.path.iter().map(|p| (p.x, p.y)).collect();
        let got = path_rects(&path);
        let want: Vec<(i32, i32, i32, i32)> =
            row.rects.iter().map(|r| (r.x, r.y, r.w, r.h)).collect();
        assert_eq!(got, want, "path {path:?}");
    }
}

#[test]
fn border_paths_match_csharp() {
    let f = fixture();
    for row in f.borders {
        assert_eq!(
            border_path(row.dx, row.width, row.height),
            points(&row.points)
        );
    }
    for row in f.icon_borders {
        assert_eq!(icon_border_path(row.x, row.y), points(&row.points));
    }
}
