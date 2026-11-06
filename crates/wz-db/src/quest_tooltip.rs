//! `QuestTooltipRenderer`가 쓰는 `Quest`/`QuestReward`의 비트맵 제외 모델.
//!
//! `Quest.CreateFromNode`와 `QuestReward.CreateFromAct1Node`에서 GDI가
//! 필요한 NPC 아이콘만 제외하고 필드를 그대로 옮긴다.

use serde::{Deserialize, Serialize};
use wz_core::image::{ImageValue, Node};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestItem {
    pub id: i32,
    pub count: i32,
    pub prob: Option<i32>,
    pub job: i32,
    pub gender: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestSp {
    pub sp: i32,
    pub jobs: Vec<i32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestReward {
    pub exp: i32,
    pub meso: i32,
    pub pop: i32,
    pub pet_tameness: i32,
    pub attr_exps: Vec<(String, i32)>,
    pub items: Vec<QuestItem>,
    pub sps: Vec<QuestSp>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Check1Item {
    pub key: String,
    pub id: i32,
    pub count: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuestTooltip {
    pub id: i32,
    pub name: Option<String>,
    pub desc: Vec<Option<String>>,
    pub blocked: bool,
    pub lvmin: i32,
    pub lvmax: i32,
    pub lv_limit: bool,
    pub recommend_except: bool,
    pub demand_base: Option<String>,
    pub check1_npc: i32,
    pub check1_items: Vec<Check1Item>,
    pub check1_infoex: Vec<(String, bool)>,
    pub act1_reward: QuestReward,
}

fn child<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    node.object_children()
        .iter()
        .find(|c| c.name.as_ref() == name)
}

fn int(node: Option<&Node>) -> i32 {
    node.and_then(|n| match &n.value {
        Some(ImageValue::Int(v)) => Some(*v),
        Some(ImageValue::Short(v)) => Some(i32::from(*v)),
        Some(ImageValue::Long(v)) => Some(*v as i32),
        Some(ImageValue::Uint(v)) => Some(*v as i32),
        Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => s.parse().ok(),
        _ => None,
    })
    .unwrap_or(0)
}

fn string(node: Option<&Node>) -> Option<String> {
    node.and_then(|n| match &n.value {
        Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => Some(s.to_string()),
        _ => None,
    })
}

fn act1_reward(node: Option<&Node>) -> QuestReward {
    let Some(node) = node else {
        return QuestReward::default();
    };
    let mut reward = QuestReward {
        exp: int(child(node, "exp")),
        meso: int(child(node, "money")),
        pop: int(child(node, "pop")),
        pet_tameness: int(child(node, "pettameness")),
        ..Default::default()
    };
    for attr in [
        "charismaEXP",
        "insightEXP",
        "willEXP",
        "craftEXP",
        "senseEXP",
        "charmEXP",
    ] {
        let exp = int(child(node, attr));
        if exp > 0 {
            reward.attr_exps.push((attr.to_string(), exp));
        }
    }
    if let Some(items) = child(node, "item") {
        for item in items.object_children() {
            let id = int(child(item, "id"));
            let count = int(child(item, "count"));
            if id > 0 && count > 0 {
                reward.items.push(QuestItem {
                    id,
                    count,
                    prob: child(item, "prop").map(|n| int(Some(n))),
                    job: int(child(item, "job")),
                    gender: child(item, "gender").map(|n| int(Some(n))).unwrap_or(2),
                });
            }
        }
    }
    if let Some(sp) = child(node, "sp") {
        for entry in sp.object_children() {
            let sp_value = int(child(entry, "sp_value"));
            let mut jobs = Vec::new();
            if let Some(jobs_node) = child(entry, "job") {
                for job in jobs_node.object_children() {
                    let job_id = int(Some(job));
                    if job_id > 0 {
                        jobs.push(job_id);
                    }
                }
            }
            if sp_value > 0 {
                reward.sps.push(QuestSp { sp: sp_value, jobs });
            }
        }
    }
    reward
}

impl QuestTooltip {
    pub fn from_node(node: &Node) -> Option<Self> {
        let id: i32 = node.name.as_ref().strip_suffix(".img")?.parse().ok()?;
        let mut quest = QuestTooltip {
            id,
            desc: vec![None, None, None],
            ..Default::default()
        };
        let mut recommend_node = None;
        if let Some(info) = child(node, "QuestInfo") {
            quest.name = string(child(info, "name"));
            for (i, key) in ["0", "1", "2"].iter().enumerate() {
                quest.desc[i] = string(child(info, key));
            }
            quest.blocked = int(child(info, "blocked")) == 1;
            if let Some(demand) = child(info, "demand") {
                quest.demand_base = string(child(demand, "base"));
            }
            recommend_node = child(info, "recommend");
        }
        if let Some(check0) = child(node, "Check").and_then(|c| child(c, "0")) {
            quest.lvmin = int(child(check0, "lvmin"));
            quest.lvmax = int(child(check0, "lvmax"));
        }
        if let Some(check1) = child(node, "Check").and_then(|c| child(c, "1")) {
            quest.check1_npc = int(child(check1, "npc"));
            for kind in ["mob", "item"] {
                if let Some(group) = child(check1, kind) {
                    for sub in group.object_children() {
                        quest.check1_items.push(Check1Item {
                            key: format!("{kind}{}", sub.name),
                            id: int(child(sub, "id")),
                            count: int(child(sub, "count")),
                        });
                    }
                }
            }
            if let Some(infoex) = child(check1, "infoex") {
                for sub in infoex.object_children() {
                    let var = string(child(sub, "exVariable")).unwrap_or_default();
                    let cond = int(child(sub, "cond"));
                    if !quest.check1_infoex.iter().any(|(k, _)| k == &var) {
                        quest.check1_infoex.push((var, cond > 0));
                    }
                }
            }
        }
        quest.act1_reward = act1_reward(child(node, "Act").and_then(|a| child(a, "1")));

        // 원본은 check0/act1 이후 recommend 를 적용해 lvmin/lvmax 를 덮어쓴다.
        if let Some(recommend) = recommend_node {
            quest.lvmin = int(child(recommend, "lvmin"));
            if let Some(lvmax) = child(recommend, "lvmax") {
                quest.lvmax = int(Some(lvmax));
                quest.lv_limit = true;
            }
            quest.recommend_except = child(recommend, "except").is_some();
        }
        Some(quest)
    }

    /// 렌더러가 표시하는 핵심 텍스트(이름/설명/수요/보상 요약).
    pub fn summary_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if let Some(name) = &self.name {
            lines.push(name.clone());
        }
        for desc in self.desc.iter().flatten() {
            lines.push(desc.clone());
        }
        if let Some(base) = self.demand_base.as_ref().filter(|s| !s.is_empty()) {
            lines.push(base.clone());
        }
        if self.act1_reward.exp > 0 {
            lines.push(format!("경험치 {}", self.act1_reward.exp));
        }
        if self.act1_reward.meso > 0 {
            lines.push(format!("메소 {}", self.act1_reward.meso));
        }
        for item in &self.act1_reward.items {
            lines.push(format!("아이템 {} x{}", item.id, item.count));
        }
        lines
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_info_and_act_reward() {
        let mut node = Node::new("1000.img");
        let mut info = Node::new("QuestInfo");
        let mut name = Node::new("name");
        name.value = Some(ImageValue::Str("퀘스트".into()));
        info.children.push(name);
        let mut desc = Node::new("0");
        desc.value = Some(ImageValue::Str("설명".into()));
        info.children.push(desc);
        node.children.push(info);
        let mut act = Node::new("Act");
        let mut act1 = Node::new("1");
        let mut exp = Node::new("exp");
        exp.value = Some(ImageValue::Int(100));
        act1.children.push(exp);
        act.children.push(act1);
        node.children.push(act);
        let q = QuestTooltip::from_node(&node).unwrap();
        assert_eq!(q.name.as_deref(), Some("퀘스트"));
        assert_eq!(q.act1_reward.exp, 100);
        assert_eq!(q.summary_lines(), vec!["퀘스트", "설명", "경험치 100"]);
    }
}
