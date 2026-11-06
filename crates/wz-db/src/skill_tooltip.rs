//! `SkillTooltipRender2.RenderSkill`의 텍스트 헤더 모델.
//!
//! 렌더러가 그리는 순서:
//! 1. 스킬 이름 (`StringLinker.StringSkill`의 `name`)
//! 2. `[최대 레벨 : N]`(hyperStat) 또는 `[마스터 레벨 : N]`(일반 스킬)
//! 3. 설명(`SummaryParser`, `desc`) 뒤에 `필요 스킬` 줄
//!
//! 원본 `Skill.MaxLevel`은 `PreBBSkill`이면 `level` 자식 수, 아니면
//! `common/maxLevel`이고, `PreBBSkill`은 `level` 자식이 하나라도 있으면 참이다.

use serde::{Deserialize, Serialize};
use wz_core::image::{ImageValue, Node};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SkillTooltip {
    pub id: i32,
    pub name: String,
    pub desc: Option<String>,
    /// `Skill.MaxLevel`
    pub max_level: i32,
    /// `Skill.PreBBSkill` = `level` 자식이 존재
    pub pre_bb: bool,
    pub hyper_stat: bool,
    pub req_level: i32,
    pub req_amount: i32,
    /// `req/<스킬ID>` 요구 레벨.
    pub req_skills: Vec<(i32, i32)>,
    /// 렌더러가 그리는 헤더/설명 줄.
    pub lines: Vec<String>,
}

fn child<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    node.object_children()
        .iter()
        .find(|c| c.name.as_ref() == name)
}

fn as_int(node: Option<&Node>) -> Option<i32> {
    node.and_then(|n| match &n.value {
        Some(ImageValue::Int(v)) => Some(*v),
        Some(ImageValue::Short(v)) => Some(i32::from(*v)),
        Some(ImageValue::Long(v)) => Some(*v as i32),
        Some(ImageValue::Uint(v)) => Some(*v as i32),
        Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => s.parse().ok(),
        _ => None,
    })
}

/// `common/maxLevel` 문자열.
fn common_max_level(common: &Node) -> Option<i32> {
    as_int(child(common, "maxLevel"))
}

impl SkillTooltip {
    pub fn from_node(node: &Node, id: i32, name: String, desc: Option<String>) -> Self {
        let level = child(node, "level");
        let level_count = level
            .map(|lvl| {
                let mut count = 0;
                loop {
                    if child(lvl, &(count + 1).to_string()).is_none() {
                        break;
                    }
                    count += 1;
                }
                count
            })
            .unwrap_or(0);
        let pre_bb = level_count > 0;
        let hyper_stat = as_int(child(node, "hyperStat")).unwrap_or(0) != 0;
        let max_level = if pre_bb {
            level_count
        } else {
            child(node, "common")
                .and_then(common_max_level)
                .unwrap_or(0)
        };

        let mut req_level = as_int(child(node, "reqLev")).unwrap_or(0);
        let mut req_amount = 0;
        let mut req_skills = Vec::new();
        if let Some(req) = child(node, "req") {
            for entry in req.object_children() {
                let key = entry.name.to_string();
                match key.as_str() {
                    "level" => req_level = as_int(Some(entry)).unwrap_or(0),
                    "reqAmount" => req_amount = as_int(Some(entry)).unwrap_or(0),
                    _ => {
                        if let Ok(skill_id) = key.parse::<i32>() {
                            req_skills.push((skill_id, as_int(Some(entry)).unwrap_or(0)));
                        }
                    }
                }
            }
        }

        let mut lines = vec![name.clone()];
        if hyper_stat {
            lines.push(format!("[최대 레벨 : {max_level}]"));
        } else if !pre_bb {
            lines.push(format!("[마스터 레벨 : {max_level}]"));
        }
        SkillTooltip {
            id,
            name,
            desc,
            max_level,
            pre_bb,
            hyper_stat,
            req_level,
            req_amount,
            req_skills,
            lines,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn node(children: Vec<(&str, Option<i32>)>) -> Node {
        let mut node = Node::new("1001005");
        for (name, value) in children {
            let mut child = Node::new(name);
            if let Some(value) = value {
                child.value = Some(ImageValue::Int(value));
            }
            node.children.push(child);
        }
        node
    }

    #[test]
    fn normal_skill_uses_master_level() {
        let mut common = Node::new("common");
        let mut max = Node::new("maxLevel");
        max.value = Some(ImageValue::Int(20));
        common.children.push(max);
        let mut skill = node(vec![]);
        skill.children.push(common);
        let tooltip = SkillTooltip::from_node(&skill, 1001005, "파워 스트라이크".into(), None);
        assert!(!tooltip.pre_bb);
        assert_eq!(tooltip.max_level, 20);
        assert_eq!(tooltip.lines, vec!["파워 스트라이크", "[마스터 레벨 : 20]"]);
    }

    #[test]
    fn level_children_make_pre_bb_and_hide_header() {
        let mut level = Node::new("level");
        for i in 1..=3 {
            let name = i.to_string();
            level.children.push(Node::new(name.as_str()));
        }
        let mut skill = node(vec![]);
        skill.children.push(level);
        let tooltip = SkillTooltip::from_node(&skill, 1, "스킬".into(), None);
        assert!(tooltip.pre_bb);
        assert_eq!(tooltip.max_level, 3);
        assert_eq!(tooltip.lines, vec!["스킬"]);
    }

    #[test]
    fn hyper_stat_uses_max_level_label() {
        let mut skill = node(vec![("hyperStat", Some(1))]);
        let mut common = Node::new("common");
        let mut max = Node::new("maxLevel");
        max.value = Some(ImageValue::Int(10));
        common.children.push(max);
        skill.children.push(common);
        let tooltip = SkillTooltip::from_node(&skill, 2, "하이퍼".into(), None);
        assert_eq!(tooltip.lines, vec!["하이퍼", "[최대 레벨 : 10]"]);
    }
}
