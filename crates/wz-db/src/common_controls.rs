//! `WzComparerR2.Common/Controls` + `Animation` 중 GDI/SharpDX 비의존 로직.
//!
//! 원본 `FrameAnimator`/`AnimationRecoder`/`AnimationControl`은
//! `Microsoft.Xna.Framework`/SharpDX 타입에 묶여 있어 macOS에서 타입 로드가
//! 불가능하다. 그래서 **순수 산술/목록 처리** 부분만 옮겼다.
//!
//! - `FrameAnimator.CreateTimeline` / `GetProcessFromTimeline`
//! - `AnimationRecoder.GetGifTimeLine` / `GetMaxLength`
//! - `AnimationControl.AddItem` 구간 계산, `UpdateTimeOffset`,
//!   재생 루프 리셋 조건, `GlobalScale` 클램프

/// `FrameAnimator.CreateTimeline` — 지연시간 누적 타임라인(`0`으로 시작).
pub fn create_timeline(delays: &[i32]) -> Vec<i32> {
    let mut timeline = Vec::with_capacity(delays.len() + 1);
    timeline.push(0);
    for ms in delays {
        let last = *timeline.last().unwrap_or(&0);
        timeline.push(last + ms);
    }
    timeline
}

/// `FrameAnimator.GetProcessFromTimeline` — `(index, progress)`.
///
/// 정확히 일치하면 `progress = 0`, 아니면 직전 구간의 비율.
pub fn get_process_from_timeline(timeline: &[i32], time_offset: i32) -> Option<(usize, f32)> {
    match timeline.binary_search(&time_offset) {
        Ok(index) => Some((index, 0.0)),
        Err(0) => None,
        Err(insert) => {
            let index = insert - 1;
            let start = *timeline.get(index)?;
            let end = *timeline.get(index + 1)?;
            let span = end - start;
            let progress = if span == 0 {
                0.0
            } else {
                (time_offset - start) as f32 / span as f32
            };
            Some((index, progress))
        }
    }
}

/// GIF 타임라인용 키프레임.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyFrame {
    pub length: i32,
    pub animated: bool,
}

/// `AnimationRecoder.GetGifTimeLine` — 프레임 지연을 GIF 지연 목록으로 분해.
///
/// `animated` 프레임은 `preferred` 단위로 쪼개고, 남는 조각은 직전 항목에
/// 더한다(첫 항목이면 새로 추가). 비애니메이션 프레임은 `max`가 있으면
/// 그 단위로 자른다.
pub fn gif_timeline(
    frames: &[KeyFrame],
    preferred_frame_delay: i32,
    max_frame_delay: Option<i32>,
) -> Vec<i32> {
    let preferred = if preferred_frame_delay <= 0 {
        1
    } else {
        preferred_frame_delay
    };
    let max = match max_frame_delay {
        Some(max) if max < preferred => Some(preferred),
        other => other,
    };

    let mut timeline: Vec<i32> = Vec::new();
    for frame in frames {
        if frame.animated {
            let mut ms = frame.length;
            while ms > 0 {
                if ms >= preferred {
                    timeline.push(preferred);
                    ms -= preferred;
                } else {
                    match timeline.last_mut() {
                        Some(last) => *last += ms,
                        None => timeline.push(ms),
                    }
                    ms = 0;
                }
            }
        } else if let Some(max) = max {
            let mut ms = frame.length;
            while ms > 0 {
                if ms >= max {
                    timeline.push(max);
                    ms -= max;
                } else {
                    timeline.push(ms);
                    ms = 0;
                }
            }
        } else {
            timeline.push(frame.length);
        }
    }
    timeline
}

/// `AnimationRecoder.GetMaxLength` — 구간 끝의 최댓값, 비어 있으면 0.
pub fn max_length(item_times: &[(i32, i32)]) -> i32 {
    item_times.iter().map(|(_, end)| *end).max().unwrap_or(0)
}

/// `AnimationControl.AddItem` — `(start, max(length, end) + start)`.
pub fn add_item_times(start: i32, length: i32, end: i32) -> (i32, i32) {
    (start, length.max(end) + start)
}

/// `AnimationControl.GetPlayingAni` 긴 구간에 해당하는 항목 인덱스.
///
/// `max_length == 0`이면 모든 항목이 재생 대상이다.
pub fn playing_indices(item_times: &[(i32, i32)], cur_time: i32, max_length: i32) -> Vec<usize> {
    item_times
        .iter()
        .enumerate()
        .filter(|(_, (start, end))| max_length == 0 || (cur_time >= *start && cur_time < *end))
        .map(|(index, _)| index)
        .collect()
}

/// `AnimationControl.UpdateTimeOffset` — 10ms 격자 반올림 + 상한 클램프.
pub fn update_time_offset(
    ms: i32,
    last_update_ms: i32,
    time_offset_ms: i32,
    max_length: i32,
    frame_interval: i32,
) -> i32 {
    let current_time = last_update_ms + time_offset_ms;
    let round = (10 - current_time % 10) % 10;
    let offset = ms + round + last_update_ms + time_offset_ms;
    let upper = (max_length - frame_interval) / 10 * 10;
    offset.min(upper).max(0)
}

/// `AnimationControl.Update` 재생 루프 리셋 조건.
pub fn should_reset(cur_time: i32, max_length: i32, margin: i32) -> bool {
    cur_time > max_length - margin
}

/// `AnimationControl.GlobalScale` — `MathHelper.Clamp(value, 0.1, 10)`.
pub fn clamp_scale(value: f32) -> f32 {
    value.clamp(0.1, 10.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timeline_is_prefix_sum() {
        assert_eq!(create_timeline(&[]), vec![0]);
        assert_eq!(create_timeline(&[30, 30, 30]), vec![0, 30, 60, 90]);
        assert_eq!(create_timeline(&[255, 0, 7]), vec![0, 255, 255, 262]);
    }

    #[test]
    fn process_uses_previous_segment() {
        let t = create_timeline(&[90]);
        assert_eq!(get_process_from_timeline(&t, 0), Some((0, 0.0)));
        let (index, progress) = get_process_from_timeline(&t, 45).unwrap();
        assert_eq!(index, 0);
        assert!((progress - 0.5).abs() < 1e-6);
        assert_eq!(get_process_from_timeline(&t, 90), Some((1, 0.0)));
    }

    #[test]
    fn gif_timeline_splits_and_merges() {
        let frame = |length, animated| KeyFrame { length, animated };
        // 애니메이션 90ms, 20ms 단위 → 20*4 + 10(마지막에 합산)
        assert_eq!(
            gif_timeline(&[frame(90, true)], 20, None),
            vec![20, 20, 20, 30]
        );
        // 첫 프레임이 단위보다 짧으면 그대로 추가
        assert_eq!(gif_timeline(&[frame(5, true)], 20, None), vec![5]);
        // 비애니메이션 + max 지정 시 분할
        assert_eq!(
            gif_timeline(&[frame(255, false)], 20, Some(30)),
            vec![30, 30, 30, 30, 30, 30, 30, 30, 15]
        );
        // 비애니메이션 + max 없음 → 원본 길이 그대로
        assert_eq!(gif_timeline(&[frame(255, false)], 20, None), vec![255]);
    }

    #[test]
    fn item_ranges_and_reset() {
        assert_eq!(add_item_times(0, 90, 0), (0, 90));
        assert_eq!(add_item_times(0, 0, 120), (0, 120));
        assert_eq!(add_item_times(50, 30, 200), (50, 250));
        assert_eq!(max_length(&[]), 0);
        assert_eq!(max_length(&[(0, 90), (100, 250)]), 250);
        assert_eq!(playing_indices(&[(0, 90)], 45, 90), vec![0]);
        assert_eq!(playing_indices(&[(0, 90)], 90, 90), Vec::<usize>::new());
        assert_eq!(playing_indices(&[(5, 5)], 99, 0), vec![0]);
    }

    #[test]
    fn offsets_and_scale() {
        assert_eq!(update_time_offset(10, 0, 0, 1000, 10), 10);
        assert_eq!(update_time_offset(1000, 0, 0, 500, 10), 490);
        assert_eq!(update_time_offset(-100, 50, 0, 1000, 10), 0);
        assert!(should_reset(91, 100, 10));
        assert!(!should_reset(90, 100, 10));
        assert!((clamp_scale(0.05) - 0.1).abs() < 1e-6);
        assert!((clamp_scale(42.0) - 10.0).abs() < 1e-6);
    }
}
