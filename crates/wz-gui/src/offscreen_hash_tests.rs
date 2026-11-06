//! 오프스크린 wgpu 렌더 해시 고정.
//!
//! GPU 어댑터가 없는 환경에서는 skip한다(창/서피스 없이 동작).

#[cfg(test)]
mod tests {
    use crate::offscreen::{render_sprite, render_sprite_sha256, test_sprite, SampleMode};
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Fixture {
        nearest_4x4: String,
        nearest_8x8: String,
        linear_8x8: String,
    }

    fn fixture() -> Fixture {
        serde_json::from_str(include_str!("../tests/fixtures/offscreen_sprite.json")).unwrap()
    }

    #[test]
    fn readback_hashes_match_fixture() {
        let sprite = test_sprite();
        let Some(nearest4) = render_sprite_sha256(&sprite, 4, 4, SampleMode::Nearest) else {
            eprintln!("skip: no wgpu adapter");
            return;
        };
        let Some(nearest8) = render_sprite_sha256(&sprite, 8, 8, SampleMode::Nearest) else {
            return;
        };
        let Some(linear8) = render_sprite_sha256(&sprite, 8, 8, SampleMode::Linear) else {
            return;
        };
        let f = fixture();
        assert_eq!(nearest4, f.nearest_4x4);
        assert_eq!(nearest8, f.nearest_8x8);
        assert_eq!(linear8, f.linear_8x8);
        assert_ne!(nearest8, linear8);
        // premultiplied readback 확인.
        let (_, _, rgba) = render_sprite(&sprite, 4, 4, SampleMode::Nearest).unwrap();
        assert_eq!(&rgba[32..36], &[0, 0, 128, 128]);
    }
}
