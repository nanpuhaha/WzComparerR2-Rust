//! 메이플스토리 프리즘(Prism) HSV 색조/채도/명도 변환 엔진.
//!
//! 원본 `WzComparerR2/AvatarCommon/Prism.cs` 및 `PrismData.cs` 의 v2 색공간 변환과
//! 16비트 스프라이트(17단위 양자화, 임계값 `0.985`) 보존 규칙을 1:1 이식한다.

/// 프리즘 색상 필터 설정 (`PrismData.cs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrismData {
    /// 색상 계열 (0: 전체, 1: 빨강, 2: 노랑, 3: 초록, 4: 청록, 5: 파랑, 6: 자주).
    pub color_type: i32,
    /// 색조 회전 (0..=359).
    pub hue: i32,
    /// 채도 (1..=199, 기본값 100).
    pub saturation: i32,
    /// 명도 (1..=199, 기본값 100).
    pub brightness: i32,
    /// 순수 검정(`R=G=B=0`) 픽셀도 변환할지 여부.
    pub convert_pure_black: bool,
}

impl Default for PrismData {
    fn default() -> Self {
        Self {
            color_type: 0,
            hue: 0,
            saturation: 100,
            brightness: 100,
            convert_pure_black: false,
        }
    }
}

impl PrismData {
    pub fn new(color_type: i32, hue: i32, saturation: i32, brightness: i32) -> Self {
        Self {
            color_type,
            hue,
            saturation,
            brightness,
            convert_pure_black: false,
        }
    }

    /// 원본 `Prism.Valid(type, hue, saturation, brightness)` 검증 규칙.
    pub fn is_valid(&self) -> bool {
        if !(0..=6).contains(&self.color_type) {
            return false;
        }
        if !(0..=359).contains(&self.hue) {
            return false;
        }
        if !(1..=199).contains(&self.saturation) {
            return false;
        }
        if !(1..=199).contains(&self.brightness) {
            return false;
        }
        !(self.hue == 0 && self.saturation == 100 && self.brightness == 100)
    }

    pub fn color_type_name(&self) -> Option<&'static str> {
        if !self.is_valid() {
            return None;
        }
        match self.color_type {
            0 => Some("전체 색상 계열"),
            1 => Some("빨간색 계열"),
            2 => Some("노란색 계열"),
            3 => Some("초록색 계열"),
            4 => Some("청록색 계열"),
            5 => Some("파란색 계열"),
            6 => Some("자주색 계열"),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Hsv {
    h: f32,
    s: f32,
    v: f32,
}

fn rgb_to_hsv_v2(r: u8, g: u8, b: u8) -> Hsv {
    let rf = r as f32 / 255.0;
    let gf = g as f32 / 255.0;
    let bf = b as f32 / 255.0;

    let max = rf.max(gf.max(bf));
    let min = rf.min(gf.min(bf));
    let d = max - min;

    let v = max;
    let s = if v > 0.0 { d / v } else { 0.0 };

    let mut h = if r == g && g == b {
        0.0
    } else {
        let mut h_raw = if (rf - max).abs() <= f32::EPSILON {
            (gf - bf) / d
        } else if (gf - max).abs() <= f32::EPSILON {
            2.0 + (bf - rf) / d
        } else {
            4.0 + (rf - gf) / d
        };
        h_raw *= 60.0;
        if h_raw < 0.0 {
            h_raw += 360.0;
        }
        h_raw
    };
    if h >= 360.0 {
        h %= 360.0;
    }

    Hsv { h, s, v }
}

#[inline]
fn custom_round(value: f32, threshold: f32) -> f32 {
    if value - value.floor() >= threshold {
        value.ceil()
    } else {
        value.floor()
    }
}

#[inline]
fn apply_step(value: f32, step: f32) -> f32 {
    custom_round(value / step, 0.985) * step
}

fn hsv_to_rgb_v2(hsv: Hsv, step_1: bool) -> [u8; 3] {
    let c = hsv.v * hsv.s;
    let x = c * (1.0 - (((hsv.h / 60.0) % 2.0) - 1.0).abs());
    let m = hsv.v - c;

    let (r, g, b) = match (hsv.h as i32) / 60 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        5 => (c, 0.0, x),
        _ => (0.0, 0.0, 0.0),
    };

    let step = if step_1 { 1.0 } else { 17.0 };
    let r_out = apply_step((r + m) * 255.0, step).clamp(0.0, 255.0) as u8;
    let g_out = apply_step((g + m) * 255.0, step).clamp(0.0, 255.0) as u8;
    let b_out = apply_step((b + m) * 255.0, step).clamp(0.0, 255.0) as u8;
    [r_out, g_out, b_out]
}

fn check_color_type(color_type: i32, hsv: &Hsv) -> bool {
    let hue = hsv.h as i32;
    match color_type {
        0 => true,
        1 => !((30..=330).contains(&hue) || hsv.s == 0.0),
        2 => (30..=90).contains(&hue),
        3 => (90..=150).contains(&hue),
        4 => (150..=210).contains(&hue),
        5 => (210..=270).contains(&hue),
        6 => (270..=330).contains(&hue),
        _ => true,
    }
}

/// RGBA8 픽셀 버퍼(`4 * N` 바이트)에 프리즘 필터(`Prism.Apply`)를 인플레이스 적용한다.
pub fn apply_prism_rgba8(rgba: &mut [u8], data: &PrismData, is_effect: bool) {
    if !data.is_valid() {
        return;
    }

    // 16비트(ARGB4444 유래 17의 배수) 컬러 여부 판별
    let not_16bit_color = rgba
        .as_chunks::<4>()
        .0
        .iter()
        .any(|px| px[0] % 17 != 0 || px[1] % 17 != 0 || px[2] % 17 != 0);

    for px in rgba.as_chunks_mut::<4>().0 {
        let r = px[0];
        let g = px[1];
        let b = px[2];
        let a = px[3];

        let mut hsv = rgb_to_hsv_v2(r, g, b);
        let mut convert = if data.convert_pure_black && hsv.s == 0.0 {
            true
        } else {
            check_color_type(data.color_type, &hsv)
        };

        if (!data.convert_pure_black && r == 0 && g == 0 && b == 0)
            || (r == 255 && g == 255 && b == 255)
            || a == 0
        {
            convert = false;
        }

        if convert {
            hsv.h = (hsv.h + data.hue as f32) % 360.0;

            let (ds, dv) = if data.saturation < 100 {
                let factor = (data.saturation - 100) as f32 / 100.0;
                (factor * hsv.s, factor * hsv.s * hsv.v / 2.0)
            } else if hsv.s > 0.0 && hsv.v < 1.0 {
                let factor = (data.saturation - 100) as f32 / 100.0;
                (
                    factor * (2.0 - hsv.s) * (1.0 - hsv.v),
                    factor * (1.0 - hsv.s) * hsv.v,
                )
            } else {
                (0.0, 0.0)
            };
            let temp_s = (hsv.s + ds).clamp(0.0, 1.0);
            let temp_v = (hsv.v + dv).clamp(0.0, 1.0);

            let (ds2, dv2) = if data.brightness < 100 {
                let factor = (data.brightness - 100) as f32 / 100.0;
                (0.0, factor * hsv.s * temp_v)
            } else {
                let factor = (data.brightness - 100) as f32 / 100.0;
                (
                    factor * -temp_s,
                    factor * (15.0 - 12.0 * hsv.s) / 15.0 * (1.0 - temp_v),
                )
            };
            hsv.s = (temp_s + ds2).clamp(0.0, 1.0);
            hsv.v = (temp_v + dv2).clamp(0.0, 1.0);

            let [nr, ng, nb] = hsv_to_rgb_v2(hsv, is_effect || not_16bit_color);
            px[0] = nr;
            px[1] = ng;
            px[2] = nb;
        }
    }
}
