//! `ItemTooltipRender22.RenderItem`의 **결정적 세로 배치** 코어.
//!
//! 텍스트 높이는 `GearGraphics.DrawString`이 `Graphics.MeasureString`으로
//! 정하므로 GDI+ 없이는 알 수 없다. 그래서 여기서는 높이를 **주입**받고,
//! 원본의 `picH` 누적·`splitterH` 기록·하한 보정만 그대로 옮긴다.
//!
//! 원본 순서:
//! 1. `picH += 10` → 이름 `+= 20` → 라벨(원더/BTS/BLACKPINK) `+= 15`
//! 2. 상단 속성(각 높이만큼)
//! 3. 유효기간·생명의 물(각 18)
//! 4. `splitterH.Add(picH - 1)`, `picH += 7`, `iconY = picH`, `picH += 3`
//! 5. 요구레벨/펫스탯/설명/성향/레시피/정액제 (각 높이 + 3)
//! 6. 하단 속성이 있으면 `picH = max(picH, iconY + 88)`, splitter, `+= 10`, 각 높이
//! 7. `picH = max(iconY + 92, picH + 2)`

/// 한 아이템의 세로 배치 입력. 텍스트 높이는 호출자가 계산해 넣는다.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ItemLayoutInput {
    /// `info/wonderGrade` (0/1/4/5/6만 라벨이 그려진다).
    pub wonder_grade: i64,
    pub bts_label: i64,
    pub blackpink_label: i64,
    /// 상단 속성 줄 높이(각 항목이 `DrawString`으로 소비).
    pub top_attr_heights: Vec<i32>,
    pub expire: bool,
    pub no_revive: bool,
    /// 요구 레벨 줄 높이.
    pub req_level_height: Option<i32>,
    /// 펫 스탯 3줄 높이.
    pub pet_stats_height: Option<i32>,
    /// 설명 블록 높이.
    pub desc_height: Option<i32>,
    /// 성향 블록 높이.
    pub incline_height: Option<i32>,
    /// 레시피 블록 높이.
    pub recipe_height: Option<i32>,
    /// 기간 정액제 블록 높이.
    pub flat_rate_height: Option<i32>,
    /// 하단 속성 줄 높이.
    pub bottom_attr_heights: Vec<i32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemLayout {
    pub pic_h: i32,
    pub icon_y: i32,
    pub splitter_h: Vec<i32>,
}

/// `LineHeight`(18) × 줄 수.
pub fn line_height(lines: i32) -> i32 {
    18 * lines.max(0)
}

/// 결정적 줄 수: `\n`으로 나눈 뒤 각 줄을 40칸마다 접는다.
pub fn wrapped_lines(text: &str) -> i32 {
    if text.is_empty() {
        return 0;
    }
    text.split('\n')
        .map(|line| {
            let count = line.chars().count();
            if count == 0 {
                1
            } else {
                count.div_ceil(40) as i32
            }
        })
        .sum()
}

/// `RenderItem`의 `picH`/`splitterH` 진행.
pub fn layout_item(input: &ItemLayoutInput) -> ItemLayout {
    let mut pic_h = 0i32;
    let mut splitter = Vec::new();

    // compare-mode 여백은 폭 계산에 의존하므로 호출자가 top_attr 로 넣지 않는다.
    pic_h += 10;
    pic_h += 20;

    // 라벨
    if input.wonder_grade > 0 {
        if !matches!(input.wonder_grade, 1 | 4 | 5 | 6) {
            pic_h -= 15;
        }
        pic_h += 15;
    } else if input.bts_label > 0 || input.blackpink_label > 0 {
        pic_h += 15;
    }

    for height in &input.top_attr_heights {
        pic_h += height;
    }
    if input.expire {
        pic_h += 18;
    }
    if input.no_revive {
        pic_h += 18;
    }
    splitter.push(pic_h - 1);

    pic_h += 7;
    let icon_y = pic_h;
    pic_h += 3;

    for height in [
        input.req_level_height,
        input.pet_stats_height,
        input.desc_height,
        input.incline_height,
        input.recipe_height,
        input.flat_rate_height,
    ]
    .into_iter()
    .flatten()
    {
        pic_h += height;
        pic_h += 3;
    }

    if !input.bottom_attr_heights.is_empty() {
        pic_h = pic_h.max(icon_y + 88);
        splitter.push(pic_h - 1);
        pic_h += 10;
        for height in &input.bottom_attr_heights {
            pic_h += height;
        }
    }

    pic_h = (icon_y + 92).max(pic_h + 2);
    ItemLayout {
        pic_h,
        icon_y,
        splitter_h: splitter,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_item_has_one_splitter_and_icon_floor() {
        let layout = layout_item(&ItemLayoutInput::default());
        assert_eq!(layout.icon_y, 37);
        assert_eq!(layout.splitter_h, vec![29]);
        assert_eq!(layout.pic_h, 129);
    }

    #[test]
    fn wonder_label_branches() {
        // 1/4/5/6은 라벨 한 줄, 그 외 양수는 -15 후 +15라 순증 없음
        let base = layout_item(&ItemLayoutInput::default()).pic_h;
        let graded = ItemLayoutInput {
            wonder_grade: 1,
            ..Default::default()
        };
        assert_eq!(layout_item(&graded).pic_h, base + 15);
        let other = ItemLayoutInput {
            wonder_grade: 2,
            ..Default::default()
        };
        assert_eq!(layout_item(&other).pic_h, base);
        // wonderGrade가 있으면 BTS 라벨은 무시된다(else-if)
        let both = ItemLayoutInput {
            wonder_grade: 1,
            bts_label: 1,
            ..Default::default()
        };
        assert_eq!(layout_item(&both).pic_h, base + 15);
    }

    #[test]
    fn text_blocks_advance_by_height_plus_three() {
        let input = ItemLayoutInput {
            desc_height: Some(36),
            req_level_height: Some(18),
            ..Default::default()
        };
        let layout = layout_item(&input);
        assert_eq!(layout.splitter_h, vec![29]);
        // 하한(iconY+92=129)이 더 크면 그 값이 최종 높이다.
        assert_eq!(layout.pic_h, 129);
        // 블록이 하한을 넘으면 누적값 + 2.
        let tall = ItemLayoutInput {
            desc_height: Some(200),
            ..Default::default()
        };
        assert_eq!(layout_item(&tall).pic_h, 40 + 200 + 3 + 2);
    }

    #[test]
    fn bottom_attributes_add_second_splitter_and_respect_icon_floor() {
        let input = ItemLayoutInput {
            bottom_attr_heights: vec![18, 18],
            ..Default::default()
        };
        let layout = layout_item(&input);
        // iconY 37 + 88 = 125 하한
        assert_eq!(layout.splitter_h, vec![29, 124]);
        assert_eq!(layout.pic_h, 125 + 10 + 36 + 2);
    }

    #[test]
    fn wrapping_is_deterministic() {
        assert_eq!(wrapped_lines(""), 0);
        assert_eq!(wrapped_lines("abc"), 1);
        assert_eq!(wrapped_lines(&"x".repeat(40)), 1);
        assert_eq!(wrapped_lines(&"x".repeat(41)), 2);
        assert_eq!(wrapped_lines("a\nb"), 2);
        assert_eq!(wrapped_lines("a\n"), 2);
        assert_eq!(line_height(2), 36);
    }
}
