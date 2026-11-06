//! 검증된 ItemTooltip 파이프라인(parser → layout → icon pixel ops → 게임 폰트)을
//! **오프스크린**에서 렌더해 안정 해시로 고정한다.
//!
//! 입력은 실제 클라이언트 아이콘(27×30, origin `-3,30`)과 실제 `String_000.wz`
//! 서식 문자열이다. 게임 폰트는 `Data`에서 읽으므로 `WZ_DATA_ROOT`가 없으면
//! 건너뛴다.

use serde::Deserialize;
use wz_avatar::{render_item_tooltip, ItemTooltipSpec};

#[derive(Deserialize)]
struct Fixture {
    title: String,
    desc: String,
    icon_width: u32,
    icon_height: u32,
    origin: [i32; 2],
    icon_rgba_b64: String,
    width: u32,
    height: u32,
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
fn headless_render_hash_is_stable() {
    let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
        eprintln!("skip: WZ_DATA_ROOT not set");
        return;
    };
    let fixture: Fixture =
        serde_json::from_str(include_str!("fixtures/item_tooltip_canvas.json")).unwrap();
    let icon = b64_decode(&fixture.icon_rgba_b64);
    assert_eq!(
        icon.len(),
        (fixture.icon_width * fixture.icon_height * 4) as usize
    );

    let spec = ItemTooltipSpec {
        title: &fixture.title,
        desc: &fixture.desc,
        icon: Some((fixture.icon_width, fixture.icon_height, icon)),
        origin: (fixture.origin[0], fixture.origin[1]),
    };
    let (w, h, rgba) = render_item_tooltip(&root, &spec).expect("font in Data");
    assert_eq!((w, h), (fixture.width, fixture.height));
    assert_eq!(rgba.len(), (w * h * 4) as usize);

    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(&rgba);
    assert_eq!(
        format!("{:x}", hasher.finalize()),
        fixture.sha256,
        "offscreen render hash changed"
    );
}

/// 아이콘이 없어도 렌더되고, 태그가 사라진 텍스트만 그려진다.
#[test]
fn renders_without_icon() {
    let Some(root) = std::env::var_os("WZ_DATA_ROOT").map(std::path::PathBuf::from) else {
        return;
    };
    let spec = ItemTooltipSpec {
        title: "제목",
        desc: "#c빨강# 설명",
        icon: None,
        origin: (0, 0),
    };
    let (w, h, rgba) = render_item_tooltip(&root, &spec).unwrap();
    assert!(w >= 160 && h >= 60);
    assert_eq!(rgba.len(), (w * h * 4) as usize);
    // 배경 밖으로 나간 픽셀이 없고 알파가 모두 255.
    assert!(rgba.chunks(4).all(|p| p[3] == 255));
}
