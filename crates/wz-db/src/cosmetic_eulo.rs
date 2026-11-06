//! `ItemTooltipRender2.ReplaceDescTags`의 `#cosmetic_EULO#` 처리.
//!
//! 원본은 코스메틱 ID로 장비 성별(`Gear.GetGender`)을 구해
//! `#c{이름}(남|여)#` 뒤에 받침에 맞는 조사(`으로`/`로`)를 붙이고,
//! `AvatarCanvas.HairColor`의 색상 접두어를 지운다.
//!
//! 관련된 `Gear.GetGearType`/`IsSubWeapon`/`GetCosmeticGender`도 함께 옮겼다
//! (`GearType`은 숫자 코드로 표현한다).

/// `GearType.subWeapon`
pub const SUB_WEAPON: i32 = -2;
pub const KATARA: i32 = 134;
pub const DEMON_SHIELD: i32 = 1099;
pub const SOUL_SHIELD: i32 = 1098;
pub const HOUR_GLASS: i32 = 17237;
pub const MAGIC_QUILL: i32 = 1726;
pub const EMBLEM: i32 = 119;
pub const POWER_SOURCE: i32 = 119020;
pub const BIT: i32 = 168;
pub const JEWEL: i32 = 178;
pub const EQUIP_BAG: i32 = 184;
pub const HAIR: i32 = 3;
pub const HAIR2: i32 = 4;
pub const HAIR3: i32 = 6;
pub const HAIR4: i32 = 7;
pub const FACE: i32 = 2;
pub const FACE2: i32 = 5;
pub const FACE3: i32 = 8;
pub const LONGCOAT: i32 = 105;

/// `Gear.AstraSubweaponTable` — `(code%10000)/100` → GearType.
const ASTRA_SUBWEAPON: [(i32, i32); 51] = [
    (0, 11512),
    (1, 13430),
    (2, 136),
    (3, 12730),
    (4, 12740),
    (5, 138),
    (6, 145),
    (7, 146),
    (8, 152),
    (9, 133),
    (10, 13526),
    (11, 136),
    (12, 149),
    (13, 153),
    (14, 154),
    (15, 155),
    (16, 156),
    (17, 157),
    (18, 158),
    (19, 159),
    (20, 160),
    (21, 161),
    (22, 162),
    (23, 163),
    (24, 164),
    (25, 165),
    (26, 166),
    (27, 167),
    (28, 169),
    (29, 170),
    (30, 171),
    (31, 172),
    (32, 173),
    (33, 174),
    (34, 175),
    (35, 176),
    (36, 177),
    (37, 179),
    (38, 180),
    (39, 181),
    (40, 182),
    (41, 183),
    (42, 185),
    (43, 186),
    (44, 187),
    (45, 188),
    (46, 189),
    (47, 190),
    (48, 191),
    (49, 192),
    (50, 193),
];

/// `Gear.GetGearType`.
pub fn get_gear_type(code: i32) -> i32 {
    match code / 1000 {
        1098 | 1099 | 1212 | 1213 | 1214 | 1215 | 1216 | 1252 | 1253 | 1254 | 1259 | 1403
        | 1404 | 1433 | 1712 | 1713 | 1714 | 1726 => return code / 1000,
        _ => {}
    }
    if code / 10000 == 135 {
        return match code / 100 {
            13522 | 13528 | 13529 | 13540 => code / 10,
            _ => code / 100 * 10,
        };
    }
    if code / 10000 == 119 && code / 100 == 11902 {
        return code / 10;
    }
    if code / 10000 == 179 {
        return match code / 1000 {
            1790..=1793 => code / 1000,
            _ => code / 100 * 10,
        };
    }
    if code / 10000 == 172 {
        let index = (code % 10000) / 100;
        return ASTRA_SUBWEAPON
            .iter()
            .find(|(i, _)| *i == index)
            .map(|(_, t)| *t)
            .unwrap_or(SUB_WEAPON);
    }
    code / 10000
}

/// `Gear.IsSubWeapon`.
pub fn is_sub_weapon(gear_type: i32) -> bool {
    matches!(
        gear_type,
        SUB_WEAPON | KATARA | DEMON_SHIELD | SOUL_SHIELD | HOUR_GLASS | MAGIC_QUILL
    ) || gear_type / 1000 == 135
}

/// `Gear.GetCosmeticGender` — 1:남, 2:여, 3:공용.
pub fn get_cosmetic_gender(code: i32) -> i32 {
    let check = code / 1000;
    match check / 10 {
        2 | 5 | 8 => match check % 10 {
            0 | 3 | 5 | 7 => 1,
            1 | 4 | 6 | 8 => 2,
            _ => 3,
        },
        3 | 4 | 6 | 7 => match check % 10 {
            0 | 3 | 5 | 6 => 1,
            1 | 4 | 7 | 8 => 2,
            _ => 3,
        },
        _ => 3,
    }
}

/// `Gear.GetGender` — 0:남, 1:여, 2:공용.
pub fn get_gender(code: i32) -> i32 {
    let gear_type = get_gear_type(code);
    if is_sub_weapon(gear_type) {
        return 2;
    }
    match gear_type {
        EMBLEM | POWER_SOURCE | BIT | JEWEL | EQUIP_BAG => 2,
        HAIR | HAIR2 | HAIR3 | HAIR4 | FACE | FACE2 | FACE3 => get_cosmetic_gender(code) - 1,
        LONGCOAT => {
            let digit = code / 1000 % 10;
            if digit == 0 {
                0
            } else if digit == 1 || digit == 6 {
                1
            } else {
                2
            }
        }
        _ => code / 1000 % 10,
    }
}

/// `AvatarCanvas.HairColor` — 조사 계산 전에 지우는 색상 접두어.
pub const HAIR_COLORS: [&str; 8] = [
    "검은색",
    "빨간색",
    "주황색",
    "노란색",
    "초록색",
    "파란색",
    "보라색",
    "갈색",
];

/// `#cosmetic_EULO#`를 실제 이름+성별+조사로 바꾼 문자열.
///
/// `name`은 `StringLinker.StringEqp[cosmetic_id]`의 이름이고, 없으면 `(null)`.
pub fn cosmetic_eulo_text(name: Option<&str>, cosmetic_id: i32) -> String {
    let gender = get_gender(cosmetic_id);
    let name = name.filter(|s| !s.is_empty()).unwrap_or("(null)");
    let suffix = match gender {
        0 => "(남)",
        1 => "(여)",
        _ => "",
    };
    let mut text = format!("#c{name}{suffix}#");
    // 원본은 마지막 한글 음절의 종성으로 `으로`/`로`를 고른다.
    let last = text
        .chars()
        .rev()
        .find(|c| ('\u{AC00}'..='\u{D7A3}').contains(c))
        .map(|c| (c as u32 - 0xAC00) % 28)
        .unwrap_or(0);
    text.push_str(if last == 0 || last == 8 {
        "로"
    } else {
        "으로"
    });
    for color in HAIR_COLORS {
        text = text.replace(&format!("{color} "), "");
    }
    text
}

/// `ReplaceDescTags`의 `#cosmetic_EULO#` 치환.
pub fn replace_cosmetic_eulo(text: &str, name: Option<&str>, cosmetic_id: i32) -> String {
    if !text.contains("#cosmetic_EULO#") {
        return text.to_string();
    }
    text.replace("#cosmetic_EULO#", &cosmetic_eulo_text(name, cosmetic_id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gear_type_branches() {
        assert_eq!(get_gear_type(1002140), 100);
        assert_eq!(get_gear_type(1352200), 135220);
        assert_eq!(get_gear_type(1190200), 119020);
        assert_eq!(get_gear_type(1098000), 1098);
        assert_eq!(get_gear_type(1720000), 11512);
        assert_eq!(get_gear_type(1729900), SUB_WEAPON);
    }

    #[test]
    fn gender_rules() {
        // 일반 장비: code/1000%10
        assert_eq!(get_gender(1002140), 2);
        assert_eq!(get_gender(1000000), 0);
        // 얼굴(2xxxxxx): GetCosmeticGender - 1
        assert_eq!(get_gender(20000), 0);
        assert_eq!(get_gender(21000), 1);
        // 롱코트(105xxxx): 천의 자리 0→남, 1/6→여
        assert_eq!(get_gender(1050000), 0);
        assert_eq!(get_gender(1051000), 1);
        assert_eq!(get_gender(1052000), 2);
    }

    #[test]
    fn particle_uses_final_consonant() {
        // 원본은 성별 접미어까지 포함한 문자열의 마지막 한글로 조사를 고른다.
        // "(남)"의 "남"에 받침이 있어 `으로`가 붙는다.
        assert_eq!(cosmetic_eulo_text(Some("헤어"), 1000000), "#c헤어(남)#으로");
        // 성별 접미어가 없는 공용(성별 2)이면 이름 끝 글자로 판정한다.
        assert_eq!(cosmetic_eulo_text(Some("헤어"), 1002140), "#c헤어#로");
        assert_eq!(cosmetic_eulo_text(Some("장갑"), 1002140), "#c장갑#으로");
        // 이름 없음 → (null)
        assert!(cosmetic_eulo_text(None, 1002140).starts_with("#c(null)"));
    }

    #[test]
    fn hair_color_prefix_is_stripped() {
        let text = cosmetic_eulo_text(Some("검은색 헤어"), 1002140);
        assert!(!text.contains("검은색 "), "{text}");
    }
}
