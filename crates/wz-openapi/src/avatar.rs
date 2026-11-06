//! `AvatarCodeStructure` + `Utils.Unpack` — 아바타 코드 비트팩 해제.
//!
//! 구조 정의(26~44)는 C# `AvatarCodeStructure.Structure`를 그대로 덤프해
//! `avatar_structure.json`에 넣었다. `Unpack`은 최상위 필드를 순서대로
//! 읽고, 값이 0이 아니고 하위 항목이 있으면 `ExpandOffset` 위치에 삽입한다.

use serde::Deserialize;
use std::sync::OnceLock;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct DataInfo {
    pub name: String,
    pub bits: i32,
    #[serde(default = "default_expand")]
    pub expand_offset: i32,
    #[serde(default)]
    pub subitems: Vec<DataInfo>,
    #[serde(skip)]
    pub value: i32,
}

fn default_expand() -> i32 {
    1
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnpackedField {
    pub name: String,
    pub bits: i32,
    pub value: i32,
}

static RAW: &str = include_str!("avatar_structure.json");

fn structures() -> &'static std::collections::BTreeMap<i32, Vec<DataInfo>> {
    static CACHE: OnceLock<std::collections::BTreeMap<i32, Vec<DataInfo>>> = OnceLock::new();
    CACHE.get_or_init(|| {
        let raw: std::collections::BTreeMap<String, Vec<DataInfo>> =
            serde_json::from_str(RAW).expect("avatar_structure.json");
        raw.into_iter()
            .map(|(k, v)| (k.parse().expect("version key"), v))
            .collect()
    })
}

/// 지원 버전. 원본 딕셔너리 키와 같다.
pub fn versions() -> Vec<i32> {
    structures().keys().copied().collect()
}

/// `UnpackedAvatarData` 생성자처럼 버전을 보정한다.
pub fn effective_version(version: i32) -> (i32, bool) {
    let map = structures();
    if map.contains_key(&version) {
        return (version, false);
    }
    let min = *map.keys().next().unwrap();
    let max = *map.keys().next_back().unwrap();
    (if version < min { min } else { max }, true)
}

/// `Utils.Unpack` — 사용한 비트 수와 필드 목록.
pub fn unpack(version: i32, pack: &[u8]) -> (i32, bool, Vec<UnpackedField>) {
    let (version, unknown) = effective_version(version);
    let mut items = structures()[&version].clone();
    let mut offset = 0usize;
    let mut index = 0usize;
    while index < items.len() {
        let bits = items[index].bits.max(0) as usize;
        let Some(value) = read_bits(pack, offset, bits) else {
            break;
        };
        items[index].value = value;
        offset += bits;
        if value != 0 && !items[index].subitems.is_empty() {
            let at = index + items[index].expand_offset.max(0) as usize;
            let subs = items[index].subitems.clone();
            let at = at.min(items.len());
            items.splice(at..at, subs);
        }
        index += 1;
    }
    let fields = items
        .iter()
        .map(|i| UnpackedField {
            name: i.name.clone(),
            bits: i.bits,
            value: i.value,
        })
        .collect();
    (version, unknown, fields)
}

fn read_bits(pack: &[u8], offset: usize, bits: usize) -> Option<i32> {
    let mut value: i32 = 0;
    for i in 0..bits {
        let bit_index = offset + i;
        let byte = *pack.get(bit_index / 8)?;
        if byte & (1 << (bit_index % 8)) != 0 {
            value |= 1 << i;
        }
    }
    Some(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_unknown_versions() {
        assert_eq!(effective_version(26), (26, false));
        assert_eq!(effective_version(25), (26, true));
        assert_eq!(effective_version(37), (44, true));
    }

    #[test]
    fn unpack_zero_pack_keeps_top_level_only() {
        let pack = vec![0u8; 64];
        let (version, unknown, fields) = unpack(26, &pack);
        assert_eq!(version, 26);
        assert!(!unknown);
        assert!(fields.iter().all(|f| f.value == 0));
        assert_eq!(fields.len(), structures()[&26].len());
    }
}
