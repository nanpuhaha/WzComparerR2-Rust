//! 뷰포트 합성 순서 (`waterBack` → scene → `waterFront`).
//!
//! 원본은 waterBack 을 배경 레이어로, waterFront 를 전면 레이어로 그린다.
//! 오프스크린 셰이더 출력은 알파가 있으므로 CPU 합성 순서를 명시적으로
//! 고정해 픽셀 회귀 테스트가 가능하게 한다.

#[allow(clippy::chunks_exact_to_as_chunks)]
pub fn alpha_over(dst: &mut [u8], src: &[u8]) {
    for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
        let a = s[3] as u32;
        if a == 0 {
            continue;
        }
        let inv = 255 - a;
        for c in 0..3 {
            d[c] = ((s[c] as u32 * a + d[c] as u32 * inv) / 255) as u8;
        }
        d[3] = ((a + d[3] as u32 * inv / 255).min(255)) as u8;
    }
}

/// waterBack 위에 scene 을 얹고, 마지막에 waterFront 를 얹는다.
pub fn compose(scene: &[u8], back: Option<&[u8]>, front: Option<&[u8]>) -> Vec<u8> {
    let mut out = match back {
        Some(back) => back.to_vec(),
        None => vec![0u8; scene.len()],
    };
    alpha_over(&mut out, scene);
    if let Some(front) = front {
        alpha_over(&mut out, front);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn back_is_behind_scene_and_front_is_over_it() {
        let scene = [255u8, 0, 0, 128];
        let back = [0u8, 0, 255, 255];
        let front = [0u8, 255, 0, 128];

        let behind = compose(&scene, Some(&back), None);
        assert!(behind[0] > 100 && behind[2] > 100, "{behind:?}");

        let both = compose(&scene, Some(&back), Some(&front));
        assert!(both[1] > behind[1], "{both:?} vs {behind:?}");
        // front 를 얹으면 alpha 가 커진다.
        assert!(both[3] >= behind[3]);
    }

    #[test]
    fn opaque_scene_hides_back_water() {
        let scene = [10u8, 20, 30, 255];
        let back = [0u8, 0, 255, 255];
        assert_eq!(compose(&scene, Some(&back), None), scene);
    }
}
