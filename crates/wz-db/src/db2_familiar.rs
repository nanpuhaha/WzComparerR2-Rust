//! DB2 Familiar 그리드 (`DB2Form.LoadFamiliar`)의 텍스트 필드.
//!
//! 이 클라이언트에는 `Character/Familiar`가 없어 실데이터 비교가 불가능하다
//! (`wz parity-scan`: Familiar files 0). 그래서 노드 규칙을 그대로 옮기고
//! 합성 fixture로만 검증한다.
//!
//! 원본 열: `ID, MobPic, "", SkillName, SkillDesc, CardID, Icon, CardName`.
//! 여기서는 비트맵 대신 존재 여부만 기록한다.

use serde::{Deserialize, Serialize};
use wz_core::image::{ImageValue, Node};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FamiliarRow {
    pub id: String,
    pub mob_id: String,
    pub skill_id: String,
    /// `"{skillID}:{name}"`, 스킬 문자열이 없으면 빈 문자열.
    pub skill_name: String,
    pub skill_desc: String,
    pub card_id: String,
    pub card_name: String,
    pub has_mob_pic: bool,
    pub has_icon: bool,
}

/// 외부 조회(몹 이미지/카드 아이콘/문자열)를 주입받기 위한 훅.
pub struct FamiliarLookups<'a> {
    /// `Etc/FamiliarInfo.img` 존재 여부와 노드.
    pub familiar_info: Option<&'a Node>,
    /// `Mob/<id>.img`에 `stand/0` 또는 `fly/0`이 있는가.
    pub mob_pic: &'a dyn Fn(&str) -> bool,
    /// `String/FamiliarSkill.img/skill/<id>` 또는 `String/Familiar.img/skill/<id>`.
    pub skill: &'a dyn Fn(&str) -> Option<(String, String)>,
    /// `Item/Consume/0287.img`/`0238.img`의 카드 아이콘 존재 여부.
    pub card_icon: &'a dyn Fn(&str) -> bool,
    /// `String/Consume.img/<card>/name`.
    pub card_name: &'a dyn Fn(&str) -> Option<String>,
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

fn pad7(text: &str) -> String {
    match text.parse::<i64>() {
        Ok(value) => format!("{value:07}"),
        Err(_) => text.to_string(),
    }
}

/// `Character/Familiar` 루트 노드에서 행을 만든다.
pub fn familiar_rows(root: &Node, lookups: &FamiliarLookups<'_>) -> Vec<FamiliarRow> {
    let mut rows = Vec::new();
    // 원본은 MobID를 루프 밖에서 선언해, 두 분기 모두 실패하면 이전 값을 유지한다.
    let mut mob_id = String::new();
    for image in root.object_children() {
        let id = image
            .name
            .as_ref()
            .strip_suffix(".img")
            .unwrap_or(image.name.as_ref())
            .to_string();
        let entry = root
            .object_children()
            .iter()
            .find(|c| c.name.as_ref() == image.name.as_ref());

        if let Some(value) = entry
            .and_then(|e| child(e, "info"))
            .and_then(|i| child(i, "MobID"))
            .and_then(|n| text(Some(n)))
        {
            mob_id = pad7(&value);
        } else if let Some(info_root) = lookups.familiar_info {
            let value = child(info_root, &id)
                .and_then(|n| child(n, "mob"))
                .and_then(|n| text(Some(n)))
                .unwrap_or_else(|| "100100".to_string());
            mob_id = pad7(&value);
        }

        let skill_id = entry
            .and_then(|e| child(e, "info"))
            .and_then(|i| child(i, "skill"))
            .and_then(|s| child(s, "id"))
            .and_then(|n| text(Some(n)))
            .unwrap_or_default();
        let (skill_name, skill_desc) = match (lookups.skill)(&skill_id) {
            Some((name, desc)) => (format!("{skill_id}:{name}"), desc),
            None => (String::new(), String::new()),
        };

        let card_id = if let Some(info_root) = lookups.familiar_info {
            child(info_root, &id)
                .and_then(|n| child(n, "consume"))
                .and_then(|n| text(Some(n)))
                .unwrap_or_default()
        } else {
            entry
                .and_then(|e| child(e, "info"))
                .and_then(|i| child(i, "monsterCardID"))
                .and_then(|n| text(Some(n)))
                .unwrap_or_default()
        };

        rows.push(FamiliarRow {
            id,
            mob_id: mob_id.clone(),
            skill_id: skill_id.clone(),
            skill_name,
            skill_desc,
            card_name: (lookups.card_name)(&card_id).unwrap_or_default(),
            has_icon: (lookups.card_icon)(&card_id),
            has_mob_pic: (lookups.mob_pic)(&mob_id),
            card_id,
        });
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(name: &str, children: Vec<Node>) -> Node {
        let mut node = Node::new(name);
        node.children = children;
        node
    }

    fn field(name: &str, value: &str) -> Node {
        let mut node = Node::new(name);
        node.value = Some(ImageValue::Str(value.into()));
        node
    }

    fn field_int(name: &str, value: i32) -> Node {
        let mut node = Node::new(name);
        node.value = Some(ImageValue::Int(value));
        node
    }

    #[test]
    fn resolves_mob_skill_and_card_fields() {
        // Character/Familiar/9990001.img/info { MobID, skill/id }
        let info = image(
            "info",
            vec![
                field_int("MobID", 100100),
                image("skill", vec![field("id", "80000001")]),
            ],
        );
        let root = image("Familiar", vec![image("9990001.img", vec![info])]);

        let info_root = image(
            "FamiliarInfo.img",
            vec![image(
                "9990001",
                vec![field("consume", "02870001"), field("mob", "100101")],
            )],
        );
        let lookup_mob = |id: &str| id == "0100100";
        let lookup_skill = |id: &str| {
            (id == "80000001").then(|| ("돌격".to_string(), "적을 밀어낸다".to_string()))
        };
        let lookup_icon = |id: &str| id == "02870001";
        let lookup_name = |id: &str| (id == "02870001").then(|| "돼지 카드".to_string());
        let lookups = FamiliarLookups {
            familiar_info: Some(&info_root),
            mob_pic: &lookup_mob,
            skill: &lookup_skill,
            card_icon: &lookup_icon,
            card_name: &lookup_name,
        };

        let rows = familiar_rows(&root, &lookups);
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.id, "9990001");
        // info/MobID(100100)가 FamiliarInfo.mob(100101)보다 우선한다.
        assert_eq!(row.mob_id, "0100100");
        assert_eq!(row.skill_id, "80000001");
        assert_eq!(row.skill_name, "80000001:돌격");
        assert_eq!(row.skill_desc, "적을 밀어낸다");
        assert_eq!(row.card_id, "02870001");
        assert_eq!(row.card_name, "돼지 카드");
        assert!(row.has_icon);
        assert!(row.has_mob_pic);
    }

    #[test]
    fn falls_back_to_familiar_info_and_keeps_previous_mob_id() {
        // 첫 이미지는 info가 없어 FamiliarInfo.mob(기본값 포함)을 쓰고,
        // 두 번째 이미지는 어떤 분기도 없어 이전 MobID를 유지한다(원본 동작).
        let root = image(
            "Familiar",
            vec![image("1234567.img", vec![]), image("1234568.img", vec![])],
        );
        let info_root = image("FamiliarInfo.img", vec![image("1234567", vec![])]);
        let no_mob = |_: &str| false;
        let no_skill = |_: &str| None;
        let no_icon = |_: &str| false;
        let no_name = |_: &str| None;
        let lookups = FamiliarLookups {
            familiar_info: Some(&info_root),
            mob_pic: &no_mob,
            skill: &no_skill,
            card_icon: &no_icon,
            card_name: &no_name,
        };

        let rows = familiar_rows(&root, &lookups);
        assert_eq!(rows[0].mob_id, "0100100", "FamiliarInfo 기본값");
        assert_eq!(rows[1].mob_id, "0100100", "이전 값 유지");
        assert!(rows[0].skill_name.is_empty());
        assert!(rows[0].card_id.is_empty());
    }
}
