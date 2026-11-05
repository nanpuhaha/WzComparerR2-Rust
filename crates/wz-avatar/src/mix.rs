//! WZ hair/face mix color (`AvatarCanvas.MixBitmaps`).
//!
//! C# 은 채널당 `(base >> 4) * (1-r) + (mix >> 4) * r` 을 계산한 뒤
//! 다시 4비트 값을 `* 17` 로 8비트로 펼친다. 이 양자화를 그대로 보존한다.

/// `base` 를 `mix` 쪽으로 `ratio`% (0..=100) 섞는다. 알파가 양쪽에서 같으면 유지한다.
pub fn mix_rgba8(base: &mut [u8], mix: &[u8], ratio: i32) {
    let ratio = ratio.clamp(0, 100);
    if ratio == 0 {
        return;
    }
    let base_op = (100 - ratio) as f32 / 100.0;
    let mix_op = ratio as f32 / 100.0;

    let (base_chunks, _) = base.as_chunks_mut::<4>();
    let (mix_chunks, _) = mix.as_chunks::<4>();
    for (b, m) in base_chunks.iter_mut().zip(mix_chunks.iter()) {
        for i in 0..4 {
            // C#: `baseArgb[i+3] == mixArgb[i+3] ? base : Blend(...)`
            if i == 3 && b[3] == m[3] {
                continue;
            }
            let blended = (((b[i] >> 4) as f32 * base_op) + ((m[i] >> 4) as f32 * mix_op)) as u8;
            b[i] = blended * 17;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_mix_uses_csharp_four_bit_quantization() {
        let mut base = vec![255, 0, 0, 255];
        let mix = vec![0, 0, 255, 255];
        mix_rgba8(&mut base, &mix, 50);
        // 15*0.5 = 7 → 7*17 = 119
        assert_eq!(base, vec![119, 0, 119, 255]);
    }

    #[test]
    fn zero_ratio_is_a_noop() {
        let mut base = vec![12, 34, 56, 78];
        let original = base.clone();
        mix_rgba8(&mut base, &[1, 2, 3, 4], 0);
        assert_eq!(base, original);
    }

    #[test]
    fn full_ratio_uses_the_mix_color_quantized() {
        let mut base = vec![255, 255, 255, 255];
        let mix = vec![0, 0, 0, 255];
        mix_rgba8(&mut base, &mix, 100);
        assert_eq!(base, vec![0, 0, 0, 255]);
    }
}
