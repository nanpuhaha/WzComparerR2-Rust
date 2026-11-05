//! 업적 데이터와 AchievementTooltip 텍스트 모델.
//!
//! 원본 `Achievement.CreateFromNode`(WzComparerR2.Common)와
//! `AchievementTooltipRenderer.Render`의 텍스트 라인을 옮겼다. 렌더러의
//! GDI 레이아웃/비트맵은 대상이 아니고, 문자열 순서만 맞춘다.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use wz_core::image::{ImageValue, Node};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Achievement {
    pub id: i32,
    pub name: String,
    pub desc: String,
    pub score: i32,
    pub difficulty: Option<String>,
    pub ui_form: Option<String>,
    pub block: Option<String>,
    pub start: Option<String>,
    pub end: Option<String>,
    pub prior_ids: Vec<i32>,
    pub missions: Vec<String>,
    pub rewards: Vec<AchievementReward>,
    pub main_category: String,
    pub sub_category: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AchievementReward {
    pub id: i32,
    pub desc: String,
}

fn child<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    node.object_children()
        .iter()
        .find(|c| c.name.as_ref() == name)
}

fn text(node: Option<&Node>) -> Option<String> {
    node.and_then(|n| match &n.value {
        Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => Some(s.to_string()),
        Some(ImageValue::Int(v)) => Some(v.to_string()),
        Some(ImageValue::Short(v)) => Some(v.to_string()),
        Some(ImageValue::Long(v)) => Some(v.to_string()),
        Some(ImageValue::Uint(v)) => Some(v.to_string()),
        _ => None,
    })
}

fn int(node: Option<&Node>, default: i32) -> i32 {
    node.and_then(|n| match &n.value {
        Some(ImageValue::Int(v)) => Some(*v),
        Some(ImageValue::Short(v)) => Some(i32::from(*v)),
        Some(ImageValue::Long(v)) => Some(*v as i32),
        Some(ImageValue::Uint(v)) => Some(*v as i32),
        Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => s.parse().ok(),
        _ => None,
    })
    .unwrap_or(default)
}

pub fn main_category(raw: Option<&str>) -> String {
    let raw = raw.unwrap_or("");
    match raw {
        "general" => "일반".into(),
        "growth" => "육성".into(),
        "job" => "직업".into(),
        "item" => "아이템".into(),
        "adventure" => "모험".into(),
        "battle" => "전투".into(),
        "social" => "소셜".into(),
        "event" => "이벤트".into(),
        "memory" => "추억".into(),
        other => other.to_string(),
    }
}

pub fn sub_category(main_raw: Option<&str>, sub_raw: Option<&str>) -> Option<String> {
    if matches!(main_raw, Some("general" | "event" | "memory")) {
        return None;
    }
    let sub = sub_raw?;
    Some(match sub {
        "level" => "레벨".into(),
        "stat" => "능력치".into(),
        "personality" => "성향".into(),
        "makingSkill" => "전문기술".into(),
        "union" => "유니온".into(),
        "story" => "스토리".into(),
        "jobChange" => "전직".into(),
        "skill" => "스킬".into(),
        "vMatrix" => "V 매트릭스".into(),
        "linkSkill" => "링크스킬".into(),
        "collection" => "수집".into(),
        "enchantment" => "강화".into(),
        "equip" => "착용".into(),
        "exploration" => "탐험".into(),
        "quest" => "퀘스트".into(),
        "cooperation" => "협동".into(),
        "special" => "스페셜".into(),
        "field" => "필드".into(),
        "boss" => "보스".into(),
        "loot" => "전리품".into(),
        "party" => "파티".into(),
        "guild" => "길드".into(),
        "trade" => "거래".into(),
        "etc" => "기타".into(),
        "progress" => "진행 중인 이벤트".into(),
        "complete" => "지나간 이벤트".into(),
        other => other.to_string(),
    })
}

impl Achievement {
    pub fn id_from_name(name: &str) -> Option<i32> {
        name.strip_suffix(".img")?.parse().ok()
    }

    /// `Achievement.CreateFromNode` 상당.
    pub fn from_node(node: &Node) -> Option<Self> {
        let id = Self::id_from_name(node.name.as_ref())?;
        let info = child(node, "info");
        let main_raw = text(info.and_then(|i| child(i, "mainCategory")));
        let sub_raw = text(info.and_then(|i| child(i, "subCategory")));
        let mut ac = Achievement {
            id,
            name: text(info.and_then(|i| child(i, "name"))).unwrap_or_default(),
            desc: text(info.and_then(|i| child(i, "desc"))).unwrap_or_default(),
            score: int(info.and_then(|i| child(i, "score")), 0),
            difficulty: info
                .and_then(|i| child(i, "difficulty"))
                .map(|n| text(Some(n)).unwrap_or_else(|| "normal".into())),
            ui_form: info
                .and_then(|i| child(i, "uiType"))
                .map(|t| text(child(t, "uiForm")).unwrap_or_else(|| "basic".into())),
            block: info
                .and_then(|i| child(i, "block"))
                .map(|n| text(Some(n)).unwrap_or_else(|| "none".into())),
            start: None,
            end: None,
            prior_ids: Vec::new(),
            missions: Vec::new(),
            rewards: Vec::new(),
            main_category: main_category(main_raw.as_deref()),
            sub_category: sub_category(main_raw.as_deref(), sub_raw.as_deref()),
        };

        if let Some(prior) = info.and_then(|i| child(i, "prior")) {
            if let Some(id_node) = child(prior, "achievement_id") {
                ac.prior_ids.push(int(Some(id_node), -1));
            } else if let Some(values) = child(prior, "values") {
                for value in values.object_children() {
                    if let Some(id_node) = child(value, "achievement_id") {
                        let prior_id = int(Some(id_node), -1);
                        if prior_id > -1 {
                            ac.prior_ids.push(prior_id);
                        }
                    }
                }
            }
        }
        if let Some(period) = info.and_then(|i| child(i, "period")) {
            ac.start = text(child(period, "start"));
            ac.end = text(child(period, "end"));
        }
        if let Some(mission_root) = child(node, "mission") {
            for mission in mission_root.object_children() {
                if let Some(name) = text(child(mission, "name")) {
                    if !name.is_empty() {
                        ac.missions.push(name);
                    }
                }
            }
        }
        if let Some(reward_root) = child(node, "reward") {
            for reward in reward_root.object_children() {
                let id = int(child(reward, "id"), -1);
                let desc = text(child(reward, "desc")).unwrap_or_default();
                if id >= 0 && !desc.is_empty() {
                    ac.rewards.push(AchievementReward { id, desc });
                }
            }
        }
        Some(ac)
    }

    pub fn show_missions(&self) -> bool {
        matches!(self.ui_form.as_deref(), Some("mission" | "all")) && !self.missions.is_empty()
    }

    pub fn hidden(&self) -> bool {
        self.block.as_deref() == Some("hide")
    }

    /// 렌더러가 그리는 텍스트 라인 순서.
    pub fn tooltip_lines(&self, names: &HashMap<i32, String>) -> Vec<String> {
        let mut lines = vec![self.name.clone()];
        if let Some(period) = self.period_text() {
            lines.push(period);
        }
        if !self.desc.is_empty() {
            lines.push(self.desc.clone());
        }
        if self.show_missions() {
            lines.extend(self.missions.iter().cloned());
        }
        let category = [
            Some(self.main_category.as_str()),
            self.sub_category.as_deref(),
        ]
        .into_iter()
        .flatten()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" / ");
        if !category.is_empty() {
            lines.push(format!("카테고리: {category}"));
        }
        if !self.prior_ids.is_empty() {
            let list = self
                .prior_ids
                .iter()
                .map(|id| {
                    let name = names.get(id).cloned().unwrap_or_else(|| "null".into());
                    format!("{name}({id:05})")
                })
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("선행 업적: {list}"));
        }
        if self.hidden() {
            lines.push("업적창에 표시되지 않음".into());
        }
        lines
    }

    fn period_text(&self) -> Option<String> {
        let start = parse_period(self.start.as_deref()?)?;
        let end = parse_period(self.end.as_deref()?)?;
        Some(format!("{start} ~ {end}"))
    }
}

/// `yyyyMMddHHmmss` → `yyyy년 MM월 dd일 HH시 mm분`.
fn parse_period(text: &str) -> Option<String> {
    if text.len() != 14 || !text.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let year = &text[0..4];
    let month = &text[4..6];
    let day = &text[6..8];
    let hour = &text[8..10];
    let minute = &text[10..12];
    let second = &text[12..14];
    // C# DateTime.TryParseExact 범위 검사.
    let m: u32 = month.parse().ok()?;
    let d: u32 = day.parse().ok()?;
    let h: u32 = hour.parse().ok()?;
    let mi: u32 = minute.parse().ok()?;
    let s: u32 = second.parse().ok()?;
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || h > 23 || mi > 59 || s > 59 {
        return None;
    }
    Some(format!("{year}년 {month}월 {day}일 {hour}시 {minute}분"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_and_period() {
        assert_eq!(main_category(Some("growth")), "육성");
        assert_eq!(
            sub_category(Some("growth"), Some("stat")).as_deref(),
            Some("능력치")
        );
        assert_eq!(sub_category(Some("general"), Some("stat")), None);
        assert_eq!(
            parse_period("20240102030405").as_deref(),
            Some("2024년 01월 02일 03시 04분")
        );
        assert_eq!(parse_period("2024"), None);
    }
}
