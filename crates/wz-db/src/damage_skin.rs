//! `DamageSkin.CreateFromNode`의 비트맵 제외 모델.
//!
//! `DamageSkinTooltipRenderer`는 텍스트 라인 없이 ID와 샘플만 그리므로,
//! 여기서는 툴팁/아이템이 참조하는 필드(ID, desc, extractID, customType,
//! 각 섹션의 키 집합/spacing/bottomFix/dotSpace)를 비교 가능한 형태로 만든다.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use wz_core::image::{ImageValue, Node};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamageSkinSection {
    pub keys: Vec<String>,
    pub number_space: i32,
    pub bottom_fix: bool,
    pub dot_space: i32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DamageSkin {
    pub id: i32,
    pub desc: Option<String>,
    pub extract_id: i32,
    pub custom_type: Option<String>,
    pub has_sample: bool,
    pub sections: BTreeMap<String, DamageSkinSection>,
}

fn int(node: &Node) -> i32 {
    match &node.value {
        Some(ImageValue::Int(v)) => *v,
        Some(ImageValue::Short(v)) => i32::from(*v),
        Some(ImageValue::Long(v)) => *v as i32,
        Some(ImageValue::Uint(v)) => *v as i32,
        Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => s.parse().unwrap_or(0),
        _ => 0,
    }
}

fn string(node: &Node) -> Option<String> {
    match &node.value {
        Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => Some(s.to_string()),
        _ => None,
    }
}

fn is_canvas_value(node: &Node) -> bool {
    matches!(
        node.value,
        Some(ImageValue::Png(_)) | Some(ImageValue::Uol(_))
    )
}

fn parse_section(node: &Node) -> DamageSkinSection {
    let mut section = DamageSkinSection::default();
    for digit in node.object_children() {
        if is_canvas_value(digit) {
            section.keys.push(digit.name.to_string());
            continue;
        }
        if digit.object_children().len() > 1 {
            if digit.object_children().iter().any(is_canvas_value) {
                section.keys.push(digit.name.to_string());
            }
            continue;
        }
        match digit.name.as_ref() {
            "numberSpace" => section.number_space = int(digit),
            "bottomFix" => section.bottom_fix = int(digit) > 0,
            "dotSpace" => section.dot_space = int(digit),
            _ => {}
        }
    }
    section.keys.sort();
    section
}

impl DamageSkin {
    pub fn from_node(node: &Node) -> Option<Self> {
        let id: i32 = node.name.as_ref().parse().ok()?;
        let mut skin = DamageSkin {
            id,
            ..Default::default()
        };
        for sub in node.object_children() {
            match sub.name.as_ref() {
                "desc" => skin.desc = string(sub),
                "extractID" => skin.extract_id = int(sub),
                "sample" => skin.has_sample = true,
                "effect" => {
                    for effect in sub.object_children() {
                        skin.add_effect(effect);
                    }
                }
                "NoRed0" | "NoRed1" | "NoCri0" | "NoCri1" | "NoCustom" => {
                    skin.add_effect(sub);
                }
                _ => {}
            }
        }
        Some(skin)
    }

    fn add_effect(&mut self, effect: &Node) {
        match effect.name.as_ref() {
            "NoRed0" | "NoRed1" | "NoCri0" | "NoCri1" => {
                self.sections
                    .insert(effect.name.to_string(), parse_section(effect));
            }
            "NoCustom" => {
                for sub in effect.object_children() {
                    if sub.name.as_ref() == "customType" {
                        self.custom_type = string(sub);
                    } else {
                        self.sections
                            .insert(sub.name.to_string(), parse_section(sub));
                    }
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_digits_and_spacing() {
        let mut node = Node::new("123");
        let mut effect = Node::new("NoRed0");
        let mut digit = Node::new("0");
        digit.value = Some(ImageValue::Png(wz_core::image::PngRef {
            width: 1,
            height: 1,
            format: 1,
            scale: 1,
            pages: 0,
            unknown1: 0,
            data_len: 0,
            offset: 0,
            sub_property: None,
        }));
        effect.children.push(digit);
        let mut space = Node::new("numberSpace");
        space.value = Some(ImageValue::Int(3));
        effect.children.push(space);
        node.children.push(effect);
        let skin = DamageSkin::from_node(&node).unwrap();
        let section = skin.sections.get("NoRed0").unwrap();
        assert_eq!(section.keys, vec!["0"]);
        assert_eq!(section.number_space, 3);
    }
}
