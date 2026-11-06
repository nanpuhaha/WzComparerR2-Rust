//! `NpcQuote.CreateFromNode` — NPC 대사 수집.
//!
//! `String/Npc.img/<id>/n0..n99, f*, w*, d*` 값과 `dialogue`/`dialog`/
//! `bubble` 자식을 모아 `#p123#` 같은 태그를 이름으로 치환한다.

use crate::arena::Arena;
use crate::linker::StringLinker;
use crate::node::NodeId;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NpcQuote {
    pub n: BTreeMap<i32, String>,
    pub f: BTreeMap<i32, String>,
    pub w: BTreeMap<i32, String>,
    pub d: BTreeMap<i32, String>,
    pub special: BTreeMap<i32, String>,
}

fn child(arena: &Arena, node: NodeId, name: &str) -> Option<NodeId> {
    arena
        .children(node)
        .iter()
        .copied()
        .find(|&c| arena.name(c) == Some(name))
}

fn scalar(arena: &Arena, node: NodeId) -> Option<String> {
    match arena.value(node) {
        Some(crate::node::NodeValue::Str(s)) | Some(crate::node::NodeValue::Uol(s)) => {
            Some(s.to_string())
        }
        _ => None,
    }
}

/// 원본 `NpcQuote.CreateFromNode`.
///
/// `npc_node`는 `String/Npc.img/<id>` 노드다(호출자가 링커에서 찾아 넘긴다).
pub fn parse(arena: &Arena, npc_node: NodeId, linker: &StringLinker) -> NpcQuote {
    let mut quote = NpcQuote::default();
    let mut n_index = -1i32;
    for kind in ["n", "f", "w", "d"] {
        for i in 0..100 {
            let Some(node) = child(arena, npc_node, &format!("{kind}{i}")) else {
                continue;
            };
            let Some(value) = scalar(arena, node) else {
                continue;
            };
            let text = string_parse(&value, linker);
            match kind {
                "n" => {
                    quote.n.insert(i, text);
                    n_index = i;
                }
                "f" => {
                    quote.f.insert(i, text);
                }
                "w" => {
                    quote.w.insert(i, text);
                }
                _ => {
                    quote.d.insert(i, text);
                }
            }
        }
    }

    for name in ["dialogue", "dialog"] {
        if let Some(node) = child(arena, npc_node, name) {
            for &inner in arena.children(node) {
                if let Some(name) = arena.name(inner).and_then(|s| s.parse::<i32>().ok()) {
                    if let Some(text) = scalar(arena, inner) {
                        quote.special.insert(name, string_parse(&text, linker));
                    }
                }
            }
        }
    }
    if let Some(bubble) = child(arena, npc_node, "bubble") {
        for &quest in arena.children(bubble) {
            for &state in arena.children(quest) {
                for &inner in arena.children(state) {
                    if let Some(text) = scalar(arena, inner) {
                        n_index += 1;
                        quote.n.insert(n_index, string_parse(&text, linker));
                    }
                }
            }
        }
    }
    quote
}

/// `GetNpcQuote` — 모든 대사를 중복 제거 후 `\r\n`으로 연결.
pub fn message(quote: &NpcQuote) -> String {
    let mut seen = std::collections::HashSet::new();
    let mut list = Vec::new();
    for map in [&quote.n, &quote.f, &quote.w, &quote.d, &quote.special] {
        for value in map.values() {
            if seen.insert(value.clone()) {
                list.push(value.clone());
            }
        }
    }
    list.join("\r\n")
}

/// 원본 `NpcQuote.stringParse`의 태그 치환/제거.
pub fn string_parse(text: &str, linker: &StringLinker) -> String {
    let mut out = String::new();
    let mut rest = text;
    // `#(p|o|m|t|q|i|v|y|illu)\s*(\d{1,9}).*?#`
    while let Some(hash) = rest.find('#') {
        out.push_str(&rest[..hash]);
        let after = &rest[hash + 1..];
        let tag_len = if after.starts_with("illu") {
            4
        } else if after.starts_with(['p', 'o', 'm', 't', 'q', 'i', 'v', 'y']) {
            1
        } else {
            0
        };
        if tag_len == 0 {
            out.push('#');
            rest = after;
            continue;
        }
        let tag = after[..tag_len].to_string();
        let mut cursor = tag_len;
        while after[cursor..].starts_with(char::is_whitespace) {
            cursor += 1;
        }
        let digits_start = cursor;
        while after[cursor..]
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_digit())
        {
            cursor += 1;
        }
        let digits = &after[digits_start..cursor];
        if digits.is_empty() || digits.len() > 9 {
            out.push('#');
            rest = after;
            continue;
        }
        let Some(end) = after[cursor..].find('#') else {
            out.push('#');
            rest = after;
            continue;
        };
        let id: i32 = digits.parse().unwrap_or(-1);
        out.push_str(&resolve(&tag, id, linker));
        rest = &after[cursor + end + 1..];
    }
    out.push_str(rest);

    out = out.replace("#b", "");
    out = out.replace("#k", "");
    out = out.replace("#kk", "");
    out = out.replace("#K", "");
    out = out.replace("#r", "");
    out = out.replace("#g", "");
    out = out.replace("#l", "");
    out = out.replace("#eqp#", "");
    out = out.replace("#es", "#ＥＳ");
    out = out.replace("#e", "");
    out = out.replace("ＥＳ", "es");
    out = out.replace("#E", "");
    out = out.replace("#n", " ");
    out
}

fn resolve(tag: &str, id: i32, linker: &StringLinker) -> String {
    let name = |sr: Option<&crate::linker::StringResult>| {
        sr.map(|sr| sr.name.clone())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| id.to_string())
    };
    match tag {
        "p" => name(linker.string_npc.get(&id)),
        "o" if id >= 100_000_000 => linker
            .string_map
            .get(&id)
            .and_then(|sr| sr.map_name.clone())
            .unwrap_or_else(|| id.to_string()),
        "o" => name(linker.string_mob.get(&id)),
        "m" => linker
            .string_map
            .get(&id)
            .and_then(|sr| sr.map_name.clone())
            .unwrap_or_else(|| id.to_string()),
        "t" | "i" | "v" => name(
            linker
                .string_item
                .get(&id)
                .or_else(|| linker.string_eqp.get(&id)),
        ),
        "q" => name(linker.string_skill.get(&id)),
        "y" => name(linker.string_quest.get(&id)),
        _ => id.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_npc_and_mob_tags() {
        let mut linker = StringLinker::new();
        linker.string_npc.insert(
            1000,
            crate::linker::StringResult {
                name: "리트".into(),
                ..Default::default()
            },
        );
        linker.string_mob.insert(
            100100,
            crate::linker::StringResult {
                name: "달팽이".into(),
                ..Default::default()
            },
        );
        assert_eq!(
            string_parse("#p1000#가 #o100100#를 봤다#n", &linker),
            "리트가 달팽이를 봤다 "
        );
    }

    #[test]
    fn message_dedups_in_order() {
        let mut quote = NpcQuote::default();
        quote.n.insert(0, "a".into());
        quote.f.insert(0, "a".into());
        quote.w.insert(0, "b".into());
        assert_eq!(message(&quote), "a\r\nb");
    }
}
