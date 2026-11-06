//! `ItemTooltipRender2`의 나머지 텍스트 슬라이스.
//!
//! - `spec/recipeValidDay`, `spec/recipeUseCount`
//! - `info/exp_minLev`·`exp_maxLev` 경험치량 (원본 `Character.ExpToNextLevel`)
//! - 펫 명령어 목록 (원본 `Item/Pet/<id>/interact` + `String/PetDialog.img`)
//!
//! 펫 명령어와 exp 제한은 이 클라이언트 데이터에 존재하지 않지만, 규칙은
//! 그대로 옮겨 두고 레시피 라인만 실데이터로 비교한다.

use serde::{Deserialize, Serialize};
use wz_core::image::{ImageValue, Node};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecipeSpec {
    pub id: Option<String>,
    pub valid_day: Option<i64>,
    pub use_count: Option<i64>,
}

fn child<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    node.object_children()
        .iter()
        .find(|c| c.name.as_ref() == name)
}

fn as_int(node: Option<&Node>) -> Option<i64> {
    node.and_then(|n| match &n.value {
        Some(ImageValue::Int(v)) => Some(i64::from(*v)),
        Some(ImageValue::Short(v)) => Some(i64::from(*v)),
        Some(ImageValue::Long(v)) => Some(*v),
        Some(ImageValue::Uint(v)) => Some(i64::from(*v)),
        Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => s.parse().ok(),
        _ => None,
    })
}

impl RecipeSpec {
    pub fn from_item(item: &Node) -> Option<Self> {
        let spec = child(item, "spec")?;
        let valid_day = as_int(child(spec, "recipeValidDay"));
        let use_count = as_int(child(spec, "recipeUseCount"));
        if valid_day.is_none() && use_count.is_none() {
            return None;
        }
        Some(Self {
            id: Some(item.name.to_string()),
            valid_day,
            use_count,
        })
    }

    /// 렌더러 라인: 값이 0보다 클 때만 그린다.
    pub fn lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(day) = self.valid_day.filter(|v| *v > 0) {
            lines.push(format!("( 제작 가능 기간 : {day}일 )"));
        }
        if let Some(count) = self.use_count.filter(|v| *v > 0) {
            lines.push(format!("( 제작 가능 횟수 : {count}회 )"));
        }
        lines
    }
}

/// 원본 `Character.ExpToNextLevel`.
pub fn exp_to_next_level(level: i32) -> i64 {
    const EXPTNL: [i64; 11] = [
        15,
        34,
        57,
        92,
        135,
        372,
        560,
        840,
        1242,
        2_207_026_470,
        2_902_427_248_153,
    ];
    if !(1..=300).contains(&level) {
        return -1;
    }
    if level < 10 {
        return EXPTNL[level as usize - 1];
    }
    let grow = |from: i32, factor: f64| -> i64 {
        let mut exp = exp_to_next_level(from);
        let mut cur = level;
        while cur > from {
            exp = (exp as f64 * factor) as i64;
            cur -= 1;
        }
        exp
    };
    match level {
        10..=14 => exp_to_next_level(9),
        15..=29 => grow(14, 1.2),
        30..=34 => exp_to_next_level(29),
        35..=39 => grow(34, 1.2),
        40..=59 => grow(39, 1.08),
        60..=64 => exp_to_next_level(59),
        65..=74 => grow(64, 1.075),
        75..=89 => grow(74, 1.07),
        90..=99 => grow(89, 1.065),
        100..=104 => exp_to_next_level(99),
        105..=139 => grow(104, 1.065),
        140..=169 => grow(139, 1.0625),
        170..=199 => grow(169, 1.05),
        200 => EXPTNL[9],
        201..=209 => grow(200, 1.12),
        210..=214 => {
            let mut exp = (exp_to_next_level(209) as f64 * 1.6) as i64;
            let mut cur = level;
            while cur > 210 {
                exp = (exp as f64 * 1.11) as i64;
                cur -= 1;
            }
            exp
        }
        215..=219 => {
            let mut exp = (exp_to_next_level(214) as f64 * 1.3) as i64;
            let mut cur = level;
            while cur > 215 {
                exp = (exp as f64 * 1.09) as i64;
                cur -= 1;
            }
            exp
        }
        220..=224 => {
            let mut exp = (exp_to_next_level(219) as f64 * 1.3) as i64;
            let mut cur = level;
            while cur > 220 {
                exp = (exp as f64 * 1.07) as i64;
                cur -= 1;
            }
            exp
        }
        225..=229 => {
            let mut exp = (exp_to_next_level(224) as f64 * 1.38) as i64;
            let mut cur = level;
            while cur > 225 {
                exp = (exp as f64 * 1.05) as i64;
                cur -= 1;
            }
            exp
        }
        230..=234 => {
            let mut exp = (exp_to_next_level(229) as f64 * 1.6) as i64;
            let mut cur = level;
            while cur > 230 {
                exp = (exp as f64 * 1.03) as i64;
                cur -= 1;
            }
            exp
        }
        235..=239 => {
            let mut exp = (exp_to_next_level(234) as f64 * 1.38) as i64;
            let mut cur = level;
            while cur > 235 {
                exp = (exp as f64 * 1.03) as i64;
                cur -= 1;
            }
            exp
        }
        240..=244 => {
            let mut exp = (exp_to_next_level(239) as f64 * 1.6) as i64;
            let mut cur = level;
            while cur > 240 {
                exp = (exp as f64 * 1.03) as i64;
                cur -= 1;
            }
            exp
        }
        245..=249 => {
            let mut exp = (exp_to_next_level(244) as f64 * 1.38) as i64;
            let mut cur = level;
            while cur > 245 {
                exp = (exp as f64 * 1.03) as i64;
                cur -= 1;
            }
            exp
        }
        250..=259 => {
            let mut exp = (exp_to_next_level(249) as f64 * 1.6 + 0.5) as i64;
            let mut cur = level;
            while cur > 250 {
                exp = (exp as f64 * 1.03) as i64;
                cur -= 1;
            }
            exp
        }
        260 => EXPTNL[10],
        261..=269 => grow(260, 1.01),
        270..=274 => {
            let mut exp = exp_to_next_level(269).saturating_mul(2);
            let mut cur = level;
            while cur > 269 {
                exp = (exp as f64 * 1.01) as i64;
                cur -= 1;
            }
            exp
        }
        275..=279 => {
            let mut exp = (exp_to_next_level(274) as f64 * 2.02) as i64;
            let mut cur = level;
            while cur > 275 {
                exp = (exp as f64 * 1.1) as i64;
                cur -= 1;
            }
            exp
        }
        280..=284 => {
            let mut exp = (exp_to_next_level(279) as f64 * 2.02) as i64;
            let mut cur = level;
            while cur > 280 {
                exp = (exp as f64 * 1.1) as i64;
                cur -= 1;
            }
            exp
        }
        285..=289 => {
            let mut exp = (exp_to_next_level(284) as f64 * 2.02) as i64;
            let mut cur = level;
            while cur > 285 {
                exp = (exp as f64 * 1.1) as i64;
                cur -= 1;
            }
            exp
        }
        290..=294 => {
            let mut exp = (exp_to_next_level(289) as f64 * 2.02) as i64;
            let mut cur = level;
            while cur > 290 {
                exp = (exp as f64 * 1.1) as i64;
                cur -= 1;
            }
            exp
        }
        295..=298 => {
            let mut exp = (exp_to_next_level(294) as f64 * 2.02) as i64;
            let mut cur = level;
            while cur > 295 {
                exp = (exp as f64 * 1.1) as i64;
                cur -= 1;
            }
            exp
        }
        299 => (exp_to_next_level(298) as f64 * 1.5) as i64,
        _ => 0,
    }
}

/// `exp_minLev`/`exp_maxLev`가 모두 0보다 클 때의 렌더러 라인.
pub fn exp_restriction_lines(
    min_lev: i64,
    max_lev: i64,
    cant_account_sharable: Option<&str>,
) -> Vec<String> {
    if min_lev <= 0 || max_lev <= 0 {
        return Vec::new();
    }
    let mut total = 0i64;
    for level in min_lev..max_lev {
        total += exp_to_next_level(level as i32);
    }
    let mut lines = vec![
        format!("총  경험치량 :{total}"),
        format!("잔여 경험치량:{total}"),
    ];
    if let Some(text) = cant_account_sharable {
        lines.push(text.to_string());
    }
    lines
}

/// 펫 명령어 라인.
///
/// `commands`는 `(명령어, 최소 레벨)` 목록이다(원본은 `interact`를 돌며
/// `String/PetDialog.img` 값에서 명령어를 얻고 레벨 최소값을 유지).
pub fn pet_command_lines(commands: &[(String, i32)]) -> Vec<String> {
    let mut merged: std::collections::BTreeMap<String, i32> = std::collections::BTreeMap::new();
    for (command, level) in commands {
        merged
            .entry(command.clone())
            .and_modify(|cur| *cur = (*cur).min(*level))
            .or_insert(*level);
    }
    let mut lines = vec!["[사용 가능한 명령어]".to_string()];
    let mut levels: Vec<i32> = merged.values().copied().collect();
    levels.sort_unstable();
    levels.dedup();
    for level in levels {
        let mut names: Vec<&String> = merged
            .iter()
            .filter(|(_, l)| **l == level)
            .map(|(name, _)| name)
            .collect();
        names.sort();
        lines.push(format!(
            "Lv. {level} 이상 : {}",
            names
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    lines.push(
        "Tip. 펫의 레벨이 15가 되면 특정 말을 하도록 시킬 수 있습니다. 펫이 하는 말은 다른 유저에게 보이지 않습니다."
            .to_string(),
    );
    lines.push("#c예) /펫 [할 말]#".to_string());
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipe_lines_skip_zero() {
        let spec = RecipeSpec {
            id: Some("02510000".into()),
            valid_day: Some(7),
            use_count: Some(0),
        };
        assert_eq!(spec.lines(), vec!["( 제작 가능 기간 : 7일 )"]);
    }

    #[test]
    fn exp_values_match_known_early_levels() {
        assert_eq!(exp_to_next_level(1), 15);
        assert_eq!(exp_to_next_level(9), 1242);
        assert_eq!(exp_to_next_level(10), 1242);
        assert_eq!(exp_to_next_level(200), 2_207_026_470);
        assert_eq!(exp_to_next_level(260), 2_902_427_248_153);
        assert_eq!(exp_to_next_level(0), -1);
    }

    #[test]
    fn pet_lines_group_by_level() {
        let lines = pet_command_lines(&[
            ("앉아".into(), 1),
            ("기다려".into(), 1),
            ("점프".into(), 15),
        ]);
        assert_eq!(lines[1], "Lv. 1 이상 : 기다려, 앉아");
        assert_eq!(lines[2], "Lv. 15 이상 : 점프");
        assert!(lines[3].starts_with("Tip."));
    }
}
