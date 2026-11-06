//! ItemTooltip 속성 태그 (`ItemTooltipRender2.GetItemAttributeString`).
//!
//! 원본은 `Item.CreateFromNode`의 `Props`를 읽어 거래/소지 제한 태그를
//! 고정 순서로 만든다. 여기서는 WZ `info` 노드에서 같은 값을 직접 읽는다.

use crate::linker::StringLinker;
use serde::{Deserialize, Serialize};
use wz_core::image::{ImageValue, Node};

fn child<'a>(node: &'a Node, name: &str) -> Option<&'a Node> {
    node.object_children()
        .iter()
        .find(|c| c.name.as_ref() == name)
}

fn value(node: &Node, key: &str) -> Option<i64> {
    child(node, key).and_then(|c| match &c.value {
        Some(ImageValue::Int(v)) => Some(i64::from(*v)),
        Some(ImageValue::Short(v)) => Some(i64::from(*v)),
        Some(ImageValue::Long(v)) => Some(*v),
        Some(ImageValue::Uint(v)) => Some(i64::from(*v)),
        Some(ImageValue::Str(s)) | Some(ImageValue::Uol(s)) => s.parse().ok(),
        _ => None,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ItemTooltipView {
    pub id: String,
    pub name: String,
    pub desc: String,
    pub auto_desc: String,
    pub attributes: Vec<String>,
}

/// ItemTooltip의 이름/설명/속성 태그를 만든다.
pub fn build_item_tooltip(
    item: &Node,
    item_id: &str,
    linker: &StringLinker,
) -> Option<ItemTooltipView> {
    let id: i32 = item_id.parse().ok()?;
    let sr = linker.string_item.get(&id);
    Some(ItemTooltipView {
        id: item_id.to_string(),
        name: sr.map(|sr| sr.name.clone()).unwrap_or_default(),
        desc: sr.and_then(|sr| sr.desc.clone()).unwrap_or_default(),
        auto_desc: sr.and_then(|sr| sr.auto_desc.clone()).unwrap_or_default(),
        attributes: item_attribute_lines(item, id),
    })
}

/// `ItemStringHelper.GetItemCoreSpecString` — 코어 아이템 설명.
///
/// `kind`는 `ItemCoreSpecType` 이름(`Ctrl_mobLv` 등)이다.
pub fn item_core_spec_string(kind: &str, value: i32, desc: Option<&str>) -> Option<String> {
    let has_coda = desc
        .filter(|d| !d.is_empty())
        .and_then(|d| d.chars().last())
        .map(|c| {
            let code = c as u32;
            (0xAC00..=0xD7A3).contains(&code) && !(code - 0xAC00).is_multiple_of(28)
        })
        .unwrap_or(false);
    let with_coda = |text: &str| {
        if has_coda {
            format!("{text}으로")
        } else {
            format!("{text}로")
        }
    };
    Some(match kind {
        "Ctrl_mobLv" if value != 0 => format!("몬스터 레벨 {value} 증가"),
        "Ctrl_mobHPRate" if value != 0 => format!("몬스터 HP {value}% 증가"),
        "Ctrl_mobRate" if value != 0 => format!("몬스터 개체 수 {value}% 증가"),
        "Ctrl_mobRateSpecial" if value != 0 => format!("몬스터 개체 수 {value}% 추가 증가"),
        "Ctrl_change_Mob" => format!("{} 몬스터 이미지 변경", with_coda(desc?)),
        "Ctrl_change_BGM" => format!("{} 배경 음악 변경", with_coda(desc?)),
        "Ctrl_change_BackGrnd" => format!("{} 배경 이미지 변경", with_coda(desc?)),
        "Ctrl_partyExp" if value != 0 => format!("파티 경험치 {value}% 증가"),
        "Ctrl_partyExpSpecial" if value != 0 => format!("파티 경험치 {value}% 추가 증가"),
        "Ctrl_addMob" if value != 0 => format!("{}, 링크{value} 지역에 추가", desc?),
        "Ctrl_dropRate" if value != 0 => format!("드롭률 {value}% 증가"),
        "Ctrl_dropRateSpecial" if value != 0 => format!("드롭률 {value}% 추가 증가"),
        "Ctrl_dropRate_Herb" if value != 0 => format!("약초 드롭률 {value}% 증가"),
        "Ctrl_dropRate_Mineral" if value != 0 => format!("광물 드롭률 {value}% 증가"),
        "Ctrl_dropRareEquip" if value != 0 => "장비 아이템이 미확인 상태로 드롭".to_string(),
        "Ctrl_reward" | "Ctrl_addMission" => desc?.to_string(),
        _ => return None,
    })
}

/// `ItemStringHelper.GetSkillReqAmount` — 스킬 요구량 라벨.
///
/// 스킬 ID의 만 단위(`skillID / 10000`)로 동물 계열을 구분한다. 원본은
/// 11200/11210/11211/11212 외에는 `？？`로 표시한다.
pub fn skill_req_amount(skill_id: i32, req_amount: i32) -> String {
    let animal = match skill_id / 10000 {
        11200 => "巨熊",
        11210 => "雪豹",
        11211 => "猛禽",
        11212 => "猫咪",
        _ => "？？",
    };
    format!("[需要{animal}技能点: {req_amount}]")
}

/// `GetItemAttributeString`과 같은 순서/문구의 태그 목록.
pub fn item_attribute_lines(item: &Node, item_id: i32) -> Vec<String> {
    let info = child(item, "info").unwrap_or(item);
    let mut tags = Vec::new();
    if value(info, "only").unwrap_or(0) != 0 {
        tags.push("중복 소지 불가".to_string());
    }
    if value(info, "quest").unwrap_or(0) != 0 {
        tags.push("퀘스트 아이템".to_string());
    }
    if value(info, "pquest").unwrap_or(0) != 0 {
        tags.push("파티 퀘스트 아이템".to_string());
    }
    if value(info, "tradeBlock").unwrap_or(0) != 0 {
        tags.push("교환 불가".to_string());
    }
    if value(info, "useTradeBlock").unwrap_or(0) != 0 {
        tags.push("사용시 교환 불가".to_string());
    } else if matches!(item_id / 10000, 501 | 502 | 516) {
        tags.push("교환 불가".to_string());
    }
    if value(info, "accountSharable").unwrap_or(0) != 0 {
        if value(info, "exp_minLev").unwrap_or(0) != 0 {
            tags.push("사용시 교환 불가".to_string());
        }
        if value(info, "sharableOnce").unwrap_or(0) != 0 {
            tags.push("월드 내 나의 캐릭터 간 1회 이동 가능\n(이동 후 교환불가)".to_string());
        } else {
            tags.push("월드 내 나의 캐릭터 간 이동만 가능".to_string());
        }
    }
    if value(info, "exchangeableOnce").unwrap_or(0) != 0 {
        tags.push("1회 교환가능 (사용 또는 거래 후 교환불가)".to_string());
    }
    if let Some(multi_pet) = value(info, "multiPet") {
        tags.push(if multi_pet == 0 {
            "일반펫 (다른 일반펫과 중복 사용불가)".to_string()
        } else {
            "멀티펫 (다른 펫과 최대 3개 중복 사용가능)".to_string()
        });
    } else if item_id / 10000 == 500 {
        tags.push("일반펫 (다른 일반펫과 중복 사용불가)".to_string());
    }
    if value(info, "accountSharableAfterExchange").unwrap_or(0) != 0 {
        tags.push("1회 교환 가능\n(거래 후 월드 내 나의 캐릭터 간 이동만 가능)".to_string());
    }
    if value(info, "mintable").unwrap_or(0) != 0 {
        tags.push("민팅 가능".to_string());
    }
    tags
}

/// `ItemStringHelper.GetItemCategoryName` — 아이템 분류 한글명.
///
/// 원본 `Item.ItemType` 이름을 그대로 받는다(`Consume`, `Install`, `Etc`,
/// `Cash`, `Pet`). 그 외는 `None`.
pub fn item_category_name(kind: &str) -> Option<&'static str> {
    Some(match kind {
        "Consume" => "소비",
        "Install" => "설치",
        "Etc" => "기타",
        "Cash" => "캐시",
        "Pet" => "펫",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item_with(pairs: &[(&str, i32)]) -> Node {
        let mut item = Node::new("02000012");
        let mut info = Node::new("info");
        for (key, value) in pairs {
            let mut c = Node::new(*key);
            c.value = Some(ImageValue::Int(*value));
            info.children.push(c);
        }
        item.children.push(info);
        item
    }

    #[test]
    fn skill_req_amount_uses_ten_thousands() {
        assert_eq!(skill_req_amount(112_000_000, 7), "[需要巨熊技能点: 7]");
        assert_eq!(skill_req_amount(112_100_000, 1), "[需要雪豹技能点: 1]");
        assert_eq!(skill_req_amount(112_130_000, 2), "[需要？？技能点: 2]");
        // 11200/10000 == 1이라 소형 ID는 매칭되지 않는다(원본과 동일).
        assert_eq!(skill_req_amount(11_200, 3), "[需要？？技能点: 3]");
    }

    #[test]
    fn trade_block_tag() {
        let item = item_with(&[("tradeBlock", 1)]);
        assert_eq!(item_attribute_lines(&item, 2_000_012), vec!["교환 불가"]);
    }

    #[test]
    fn account_share_tags() {
        let item = item_with(&[("accountSharable", 1), ("sharableOnce", 1)]);
        assert_eq!(
            item_attribute_lines(&item, 2_000_012),
            vec!["월드 내 나의 캐릭터 간 1회 이동 가능\n(이동 후 교환불가)"]
        );
    }
}

/// `ItemStringHelper.ToCJKNumberExpr` — 가격/수치를 한글 단위로 표기.
///
/// - `detailed = false`: `경/조/억/만` 블록을 공백으로 구분 (`1억 2345만 6789`)
/// - `detailed = true`: 자리마다 `십/백/천` + 블록 단위, 0은 1~3개 연속일 때만
///   하나로 압축 (`일억이천삼백사십오만육천칠백팔십구`)
pub fn to_cjk_number_expr(value: i64, detailed: bool) -> String {
    let mut out = String::new();
    let mut v = value as i128;
    if v < 0 {
        out.push('-');
        v = -v;
    }
    if detailed {
        const SMALL: [&str; 4] = ["", "십", "백", "천"];
        const BIG: [&str; 5] = ["", "만", "억", "조", "경"];
        let digits: Vec<u8> = v.to_string().bytes().collect();
        let len = digits.len();
        let mut block_has_value = false;
        let mut zero_count = 0usize;
        for (i, digit) in digits.iter().enumerate() {
            let pos_from_right = len - i - 1;
            let small = pos_from_right % 4;
            let big = pos_from_right / 4;
            if *digit == b'0' {
                zero_count += 1;
            } else {
                if (1..=3).contains(&zero_count) {
                    out.push('0');
                }
                zero_count = 0;
                out.push(*digit as char);
                if small > 0 {
                    out.push_str(SMALL[small]);
                }
                block_has_value = true;
            }
            if small == 0 {
                if block_has_value && big > 0 && big < BIG.len() {
                    out.push_str(BIG[big]);
                }
                block_has_value = false;
                zero_count = 0;
            }
        }
    } else {
        const UNITS: [(i128, &str); 4] = [
            (1_0000_0000_0000_0000, "경"),
            (1_0000_0000_0000, "조"),
            (1_0000_0000, "억"),
            (1_0000, "만"),
        ];
        let mut first = true;
        for (unit, suffix) in UNITS {
            if v >= unit {
                let part = v / unit;
                if !first {
                    out.push(' ');
                }
                out.push_str(&format!("{part}{suffix}"));
                v -= part * unit;
                first = false;
            }
        }
        if v > 0 {
            if !first {
                out.push(' ');
            }
            out.push_str(&v.to_string());
        }
    }
    if out.is_empty() {
        "0".to_string()
    } else {
        out
    }
}

#[cfg(test)]
mod cjk_number_tests {
    use super::to_cjk_number_expr;

    #[test]
    fn non_detailed_blocks() {
        assert_eq!(to_cjk_number_expr(0, false), "0");
        assert_eq!(to_cjk_number_expr(1234, false), "1234");
        assert_eq!(to_cjk_number_expr(12345, false), "1만 2345");
        assert_eq!(to_cjk_number_expr(123456789, false), "1억 2345만 6789");
        assert_eq!(to_cjk_number_expr(-10000, false), "-1만");
        assert_eq!(to_cjk_number_expr(1_0000_0000_0000_0000, false), "1경");
    }

    #[test]
    fn detailed_uses_small_and_big_units() {
        assert_eq!(to_cjk_number_expr(0, true), "0");
        assert_eq!(to_cjk_number_expr(1234, true), "1천2백3십4");
        assert_eq!(to_cjk_number_expr(12345, true), "1만2천3백4십5");
        assert_eq!(to_cjk_number_expr(10000, true), "1만");
        assert_eq!(to_cjk_number_expr(10001, true), "1만01");
        assert_eq!(to_cjk_number_expr(-45, true), "-4십5");
    }
}

/// `ItemStringHelper.ToThousandsNumberExpr` — 가격 축약 표기.
///
/// - `is_msea = false`: 1e9/1e6/1e3 이상을 **소수 1자리**로 반올림해
///   `B`/`M`/`K` 접미(`Math.Round(double, 1)` = banker's rounding)
/// - `is_msea = true`: `Q/T/B/M/K` 블록을 공백으로 구분(내림 나눗셈)
/// - 0은 `"0"`, 음수는 `-` 접두
pub fn to_thousands_number_expr(value: i64, is_msea: bool) -> String {
    let mut out = String::new();
    let mut v = value;
    if v < 0 {
        out.push('-');
        v = v.wrapping_neg();
    }
    if is_msea {
        const UNITS: [(i64, &str); 5] = [
            (1_0000_0000_0000_0000, "Q"),
            (1_000_000_000_000, "T"),
            (1_000_000_000, "B"),
            (1_000_000, "M"),
            (1_000, "K"),
        ];
        let mut first = true;
        for (unit, suffix) in UNITS {
            if v >= unit {
                let part = v / unit;
                if !first {
                    out.push(' ');
                }
                out.push_str(&format!("{part}{suffix}"));
                v -= part * unit;
                first = false;
            }
        }
        if v > 0 {
            if !first {
                out.push(' ');
            }
            out.push_str(&v.to_string());
        }
    } else if v >= 1_000_000_000 {
        out.push_str(&format_dotnet_one_decimal(v as f64 / 1_000_000_000.0));
        out.push('B');
    } else if v >= 1_000_000 {
        out.push_str(&format_dotnet_one_decimal(v as f64 / 1_000_000.0));
        out.push('M');
    } else if v >= 1_000 {
        out.push_str(&format_dotnet_one_decimal(v as f64 / 1_000.0));
        out.push('K');
    } else if v > 0 {
        out.push_str(&v.to_string());
    }
    if out.is_empty() {
        "0".to_string()
    } else {
        out
    }
}

/// `Math.Round(x, 1)` 후 C# `double.ToString()`과 같은 최단 표기.
fn format_dotnet_one_decimal(x: f64) -> String {
    let rounded = (x * 10.0).round_ties_even() / 10.0;
    format!("{rounded}")
}

/// `ItemStringHelper.GetMobSkillName` — 몹 스킬 ID의 한글명.
pub fn get_mob_skill_name(id: i32) -> Option<&'static str> {
    Some(match id {
        100 => "물리 공격력 증가",
        101 => "마법 공격력 증가",
        102 => "물리 방어력 증가",
        103 => "마법 방어력 증가",
        105 => "HP 흡수",
        110 => "주변 물리 공격력 증가",
        111 => "주변 마법 공격력 증가",
        112 => "주변 물리 방어력 증가",
        113 => "주변 마법 방어력 증가",
        114 => "HP 회복",
        115 => "이동속도 증가",
        120 => "봉인",
        121 => "빗나감",
        122 => "허약",
        123 => "기절",
        124 => "저주",
        125 => "중독",
        126 => "슬로우",
        127 => "버프 해제",
        128 => "유혹",
        129 => "추방",
        131 => "범위 지속 데미지",
        132 => "혼란",
        133 => "언데드",
        134 => "물약 봉인",
        135 => "멈추지 않는",
        136 => "암흑",
        137 => "프로즌",
        138 => "잠재능력 무효",
        140 => "물리 공격 무시",
        141 => "마법 공격 무시",
        142 => "하드 스킨",
        143 => "물리 공격 반사",
        144 => "마법 공격 반사",
        145 => "공격 반사",
        146 => "무적",
        150 => "물리 공격력 증가",
        151 => "마법 공격력 증가",
        152 => "물리 방어력 증가",
        153 => "마법 방어력 증가",
        154 => "명중률 증가",
        155 => "회피율 증가",
        156 => "이동속도 증가",
        170 => "텔레포트",
        171 => "폭발",
        172 => "변이",
        173 => "에어본",
        174 => "석화",
        200 => "소환",
        201 => "소환",
        _ => return None,
    })
}

#[cfg(test)]
mod thousands_mobskill_tests {
    use super::*;

    #[test]
    fn thousands_short_forms() {
        assert_eq!(to_thousands_number_expr(0, false), "0");
        assert_eq!(to_thousands_number_expr(999, false), "999");
        assert_eq!(to_thousands_number_expr(1000, false), "1K");
        assert_eq!(to_thousands_number_expr(1500, false), "1.5K");
        assert_eq!(to_thousands_number_expr(1_500_000_000, false), "1.5B");
        // i64::MIN은 C# unchecked 부정으로 부호만 남는다.
        assert_eq!(to_thousands_number_expr(i64::MIN, false), "-");
    }

    #[test]
    fn thousands_msea_blocks() {
        assert_eq!(to_thousands_number_expr(1500, true), "1K 500");
        assert_eq!(
            to_thousands_number_expr(1_234_567_890, true),
            "1B 234M 567K 890"
        );
        assert_eq!(to_thousands_number_expr(-1000, true), "-1K");
    }

    #[test]
    fn mob_skill_names() {
        assert_eq!(get_mob_skill_name(100), Some("물리 공격력 증가"));
        assert_eq!(get_mob_skill_name(201), Some("소환"));
        assert_eq!(get_mob_skill_name(104), None);
        assert_eq!(get_mob_skill_name(999), None);
    }
}
