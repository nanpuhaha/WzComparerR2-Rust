//! `Potential` 요약/등급 판정과 `GearType` 숫자 값.
//!
//! GDI와 무관하게 계산되는 부분만 옮겼다(문자열 치환/정수 판정).

use std::collections::BTreeMap;
use wz_core::image::{ImageValue, Node};

/// `(GearType 이름) -> enum 값`.
pub fn gear_type_value(name: &str) -> Option<i32> {
    Some(match name {
        "body" => 0,
        "head" => 1,
        "face" => 2,
        "hair" => 3,
        "hair2" => 4,
        "face2" => 5,
        "hair3" => 6,
        "hair4" => 7,
        "face3" => 8,
        "faceAccessory" => 101,
        "eyeAccessory" => 102,
        "earrings" => 103,
        "pendant" => 112,
        "belt" => 113,
        "medal" => 114,
        "shoulderPad" => 115,
        "cap" => 100,
        "cape" => 110,
        "coat" => 104,
        "dragonMask" => 194,
        "dragonPendant" => 195,
        "dragonWings" => 196,
        "dragonTail" => 197,
        "glove" => 108,
        "longcoat" => 105,
        "machineEngine" => 161,
        "machineArms" => 162,
        "machineLegs" => 163,
        "machineBody" => 164,
        "machineTransistors" => 165,
        "android" => 166,
        "machineHeart" => 167,
        "pocket" => 116,
        "badge" => 118,
        "emblem" => 119,
        "powerSource" => 119020,
        "pants" => 106,
        "ring" => 111,
        "shield" => 109,
        "soulShield" => 1098,
        "demonShield" => 1099,
        "shoes" => 107,
        "shiningRod" => 1212,
        "tuner" => 1213,
        "breathShooter" => 1214,
        "longSword" => 1215,
        "carta" => 1216,
        "soulShooter" => 122,
        "desperado" => 123,
        "energySword" => 124,
        "memorialStaff" => 1252,
        "celestialLight" => 1253,
        "onmyouSen" => 1254,
        "magicStick" => 1259,
        "espLimiter" => 126,
        "chain2" => 127,
        "magicGauntlet" => 128,
        "handFan" => 129,
        "ohSword" => 130,
        "ohAxe" => 131,
        "ohBlunt" => 132,
        "dagger" => 133,
        "katara" => 134,
        "magicArrow" => 135200,
        "card" => 135210,
        "heroMedal" => 135220,
        "rosario" => 135221,
        "chain" => 135222,
        "book1" => 135223,
        "book2" => 135224,
        "book3" => 135225,
        "bowMasterFeather" => 135226,
        "crossBowThimble" => 135227,
        "shadowerSheath" => 135228,
        "nightLordPoutch" => 135229,
        "box" => 135230,
        "orb" => 135240,
        "novaMarrow" => 135250,
        "soulBangle" => 135260,
        "mailin" => 135270,
        "kodachi" => 135280,
        "whistle" => 135281,
        "boxingClaw" => 135282,
        "kodachi2" => 135283,
        "whistle2" => 135284,
        "boxingSky" => 135286,
        "compass" => 135287,
        "keir" => 135288,
        "viperWristband" => 135290,
        "captainSight" => 135291,
        "connonGunPowder" => 135292,
        "aranPendulum" => 135293,
        "evanPaper" => 135294,
        "battlemageBall" => 135295,
        "wildHunterArrowHead" => 135296,
        "cygnusGem" => 135297,
        "connonGunPowder2" => 135298,
        "controller" => 135300,
        "foxPearl" => 135310,
        "chess" => 135320,
        "transmitter" => 135330,
        "ExplosivePill" => 135340,
        "magicWing" => 135350,
        "pathOfAbyss" => 135360,
        "relic" => 135370,
        "fanTassel" => 135380,
        "bracelet" => 135400,
        "weaponBelt" => 135401,
        "ornament" => 135402,
        "hexSeeker" => 135403,
        "sacredJewel" => 135404,
        "kannaReifu" => 135430,
        "cane" => 136,
        "wand" => 137,
        "staff" => 138,
        "barehand" => 139,
        "thSword" => 140,
        "boxingCannon" => 1403,
        "chakram" => 1404,
        "thAxe" => 141,
        "thBlunt" => 142,
        "spear" => 143,
        "gram" => 1433,
        "polearm" => 144,
        "bow" => 145,
        "crossbow" => 146,
        "throwingGlove" => 147,
        "knuckle" => 148,
        "gun" => 149,
        "shovel" => 150,
        "pickaxe" => 151,
        "dualBow" => 152,
        "handCannon" => 153,
        "katana" => 154,
        "fan" => 155,
        "swordZB" => 156,
        "swordZL" => 157,
        "GauntletBuster" => 158,
        "ancientBow" => 159,
        "bit" => 168,
        "cashWeapon" => 170,
        "weapon" => -1,
        "subWeapon" => -2,
        "totem" => 120,
        "arcaneSymbol" => 1712,
        "authenticSymbol" => 1713,
        "grandAuthenticSymbol" => 1714,
        "hourGlass" => 17237,
        "magicQuill" => 1726,
        "jewel" => 178,
        "face_n" => 1790,
        "hair_n" => 1791,
        "head_n" => 1792,
        "hair2_n" => 1793,
        "petEquip" => 180,
        "equipBag" => 184,
        "taming" => 190,
        "saddle" => 191,
        "taming2" => 193,
        "tamingChair" => 198,
        "taming3" => 199,
        _ => return None,
    })
}

/// `Gear.IsLeftWeapon` — `121..=139`(`katara` 제외) 또는 십 단위 121/125.
pub fn is_left_weapon(value: i32) -> bool {
    ((121..=139).contains(&value) && value != 134) || value / 10 == 121 || value / 10 == 125
}

/// `Gear.IsDoubleHandWeapon` — `140..=149`, `152..=159`, boxingCannon/chakram/gram.
pub fn is_double_hand_weapon(value: i32) -> bool {
    (140..=149).contains(&value)
        || (152..=159).contains(&value)
        || matches!(value, 1403 | 1404 | 1433)
}

/// `Gear.IsWeapon`.
pub fn is_weapon(value: i32) -> bool {
    is_left_weapon(value) || is_double_hand_weapon(value)
}

/// `Potential.GetPotentialLevel` — 요구 레벨 → 잠재능력 레벨.
pub fn potential_level(gear_req_level: i32) -> i32 {
    if gear_req_level <= 0 {
        1
    } else if gear_req_level >= 200 {
        20
    } else {
        (gear_req_level + 9) / 10
    }
}

/// `Potential.CheckOptionType` — 옵션 타입이 장비 종류에 맞는지.
pub fn check_option_type(option_type: i32, gear_type: &str) -> bool {
    let value = match gear_type_value(gear_type) {
        Some(v) => v,
        None => return option_type == 0 || option_type == 11,
    };
    match option_type {
        0 => true,
        10 => is_weapon(value) || crate::cosmetic_eulo::is_sub_weapon(value) || value == 109,
        11 => !check_option_type(10, gear_type),
        20 => {
            crate::cosmetic_eulo::is_sub_weapon(value)
                || matches!(value, 106 | 107 | 100 | 104 | 105 | 108 | 110 | 113 | 115)
        }
        40 => matches!(value, 101 | 102 | 111 | 103 | 112),
        51 => value == 100,
        52 => matches!(value, 104 | 105),
        53 => value == 106,
        54 => value == 108,
        55 => value == 107,
        _ => false,
    }
}

/// `Potential.ConvertSummary` — `stringSummary`의 `#<GearPropType>`를 값으로 치환.
///
/// 원본은 이름 길이 내림차순으로 정렬해 치환한다(접두 관계인 `incSTR`/
/// `incSTRr`에서 긴 이름이 먼저 바뀌도록). 같은 길이끼리는 접두 관계가 될 수
/// 없어 정렬이 불안정해도 결과는 같다.
pub fn convert_summary(
    summary: &str,
    props: &BTreeMap<String, i64>,
    props_f: &BTreeMap<String, f64>,
) -> Option<String> {
    if summary.is_empty() {
        return None;
    }
    let mut names: Vec<&String> = props.keys().collect();
    names.extend(props_f.keys());
    names.sort_by_key(|n| std::cmp::Reverse(n.chars().count()));
    let mut text = summary.to_string();
    for name in names {
        let replacement = match props.get(name) {
            Some(v) => v.to_string(),
            None => props_f.get(name).map(|v| v.to_string()).unwrap_or_default(),
        };
        text = text.replace(&format!("#{name}"), &replacement);
    }
    Some(text)
}

/// `Potential.IsPotentialEx` — `code / 1000 % 10 == 2`.
pub fn is_potential_ex(code: i32) -> bool {
    code / 1000 % 10 == 2
}

/// `Potential.ToString` — `d6 code + " " + summary + (" - " + weight)`.
pub fn potential_to_string(code: i32, summary: Option<&str>, weight: i32) -> String {
    let mut out = format!("{code:06}");
    if let Some(summary) = summary {
        out.push(' ');
        out.push_str(summary);
    }
    if weight > 0 {
        out.push_str(&format!(" - {weight}"));
    }
    out
}

/// `Potential.CreateFromNode`이 만드는 관찰 가능한 값.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PotentialOption {
    pub code: i32,
    pub option_type: i32,
    pub req_level: i32,
    pub weight: i32,
    /// `ConvertSummary()` 결과(치환 후 문자열).
    pub summary: Option<String>,
}

fn child<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    node.object_children()
        .iter()
        .find(|c| c.name.as_ref() == name)
}

fn node_i64(node: &Node) -> Option<i64> {
    match &node.value {
        Some(ImageValue::Int(v)) => Some(i64::from(*v)),
        Some(ImageValue::Short(v)) => Some(i64::from(*v)),
        Some(ImageValue::Long(v)) => Some(*v),
        Some(ImageValue::Uint(v)) => Some(i64::from(*v)),
        Some(ImageValue::Float(v)) => Some(f64::from(*v) as i64),
        Some(ImageValue::Double(v)) => Some(*v as i64),
        Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => s.parse().ok(),
        _ => None,
    }
}

fn node_string(node: &Node) -> Option<String> {
    match &node.value {
        Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => Some(s.to_string()),
        _ => None,
    }
}

/// `Potential.CreateFromNode(potentialNode, pLevel)`.
///
/// 노드 이름이 코드이고, `info` 아래 `optionType`/`reqLevel`/`weight`/`string`,
/// `level/<pLevel>` 아래에 `GearPropType` 값이 있다. `level/<pLevel>`이 없으면
/// 원본처럼 `None`을 돌려준다.
pub fn from_node(node: &Node, level: i32) -> Option<PotentialOption> {
    let code: i32 = node.name.as_ref().parse().ok()?;
    let mut option = PotentialOption {
        code,
        option_type: 0,
        req_level: 0,
        weight: 0,
        summary: None,
    };
    let mut props: BTreeMap<String, i64> = BTreeMap::new();
    let mut props_f: BTreeMap<String, f64> = BTreeMap::new();
    let mut summary_text: Option<String> = None;

    for sub in node.object_children() {
        match sub.name.as_ref() {
            "info" => {
                for info in sub.object_children() {
                    match info.name.as_ref() {
                        "optionType" => option.option_type = node_i64(info).unwrap_or(0) as i32,
                        "reqLevel" => option.req_level = node_i64(info).unwrap_or(0) as i32,
                        "weight" => option.weight = node_i64(info).unwrap_or(0) as i32,
                        "string" => summary_text = node_string(info),
                        _ => {}
                    }
                }
            }
            "level" => {
                let level_node = child(sub, &level.to_string())?;
                for prop in level_node.object_children() {
                    let name = prop.name.to_string();
                    if name == "face" {
                        props.insert(name, 0);
                        continue;
                    }
                    match &prop.value {
                        Some(ImageValue::Int(v)) => {
                            props.insert(name, i64::from(*v));
                        }
                        Some(ImageValue::Short(v)) => {
                            props.insert(name, i64::from(*v));
                        }
                        Some(ImageValue::Long(v)) => {
                            props.insert(name, *v);
                        }
                        Some(ImageValue::Uint(v)) => {
                            props.insert(name, i64::from(*v));
                        }
                        Some(ImageValue::Float(v)) => {
                            props_f.insert(name, f64::from(*v));
                        }
                        Some(ImageValue::Double(v)) => {
                            props_f.insert(name, *v);
                        }
                        Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => {
                            if let Ok(v) = s.parse::<i64>() {
                                props.insert(name, v);
                            } else if let Ok(v) = s.parse::<f64>() {
                                props_f.insert(name, v);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    option.summary = summary_text
        .as_deref()
        .and_then(|text| convert_summary(text, &props, &props_f));
    Some(option)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn potential_level_bounds() {
        assert_eq!(potential_level(0), 1);
        assert_eq!(potential_level(-5), 1);
        assert_eq!(potential_level(1), 1);
        assert_eq!(potential_level(10), 1);
        assert_eq!(potential_level(11), 2);
        assert_eq!(potential_level(199), 20);
        assert_eq!(potential_level(200), 20);
        assert_eq!(potential_level(250), 20);
    }

    #[test]
    fn option_types_match_gear_ranges() {
        assert!(check_option_type(0, "cap"));
        assert!(check_option_type(10, "ohSword"));
        assert!(check_option_type(10, "katara"));
        assert!(check_option_type(10, "shield"));
        assert!(!check_option_type(10, "cap"));
        assert!(check_option_type(11, "cap"));
        assert!(check_option_type(20, "pants"));
        assert!(check_option_type(20, "shoulderPad"));
        assert!(check_option_type(40, "ring"));
        assert!(check_option_type(51, "cap"));
        assert!(!check_option_type(51, "coat"));
        assert!(check_option_type(52, "longcoat"));
        assert!(!check_option_type(99, "cap"));
    }

    #[test]
    fn summary_replaces_longest_first() {
        let mut props = BTreeMap::new();
        props.insert("incSTR".to_string(), 10);
        props.insert("incSTRr".to_string(), 5);
        let got = convert_summary("#incSTRr% #incSTR", &props, &BTreeMap::new()).unwrap();
        assert_eq!(got, "5% 10");
        assert_eq!(convert_summary("", &props, &BTreeMap::new()), None);
    }

    #[test]
    fn summary_float_values() {
        let mut props_f = BTreeMap::new();
        props_f.insert("incMHP".to_string(), 2.5f64);
        let got = convert_summary("#incMHP", &BTreeMap::new(), &props_f).unwrap();
        assert_eq!(got, "2.5");
    }

    #[test]
    fn to_string_formats() {
        assert_eq!(
            potential_to_string(31001, Some("STR +10"), 0),
            "031001 STR +10"
        );
        assert_eq!(
            potential_to_string(31001, Some("STR +10"), 30),
            "031001 STR +10 - 30"
        );
        assert!(is_potential_ex(312345));
        assert!(!is_potential_ex(311345));
    }
}
