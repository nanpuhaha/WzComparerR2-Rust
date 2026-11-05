//! WorldArchiveTooltipRender의 텍스트 모델.
//!
//! 원본은 `StringLinker.StringWorldArchiveMob/Npc`의 `Desc`와
//! `StringMonsterBook`의 `Desc`를 받아 섹션으로 나눠 그린다. 여기서는
//! GDI 레이아웃을 제외한 라인 구성만 옮긴다. NPC 대사(`NpcQuote`)는
//! 원본이 `FindNodeFromSource`로 대화/버블 노드를 직접 찾으므로 별도
//! 데이터 경로가 필요해 이번 범위에서 제외한다.

/// 원본 `SplitLine`과 같은 구분자 집합.
pub fn split_lines(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                out.push(std::mem::take(&mut current));
            }
            '\n' => out.push(std::mem::take(&mut current)),
            '\\' => match chars.peek() {
                Some('r') => {
                    chars.next();
                    if chars.peek() == Some(&'\\') {
                        // `\r\n` 리터럴
                        let mut clone = chars.clone();
                        clone.next();
                        if clone.peek() == Some(&'n') {
                            chars.next();
                            chars.next();
                        }
                    } else if chars.peek() == Some(&'n') {
                        chars.next();
                    }
                    out.push(std::mem::take(&mut current));
                }
                Some('n') => {
                    chars.next();
                    out.push(std::mem::take(&mut current));
                }
                _ => current.push(c),
            },
            _ => current.push(c),
        }
    }
    out.push(current);
    out
}

/// 몹 툴팁: 월드 아카이브 설명 + (있으면) 몬스터북 섹션.
pub fn mob_lines(world_archive: Option<&str>, monster_book: Option<&str>) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(desc) = world_archive.filter(|d| !d.is_empty()) {
        lines.extend(split_lines(desc));
    }
    if let Some(desc) = monster_book.filter(|d| !d.is_empty()) {
        lines.push("[몬스터북]".to_string());
        lines.extend(split_lines(desc));
    }
    lines
}

/// NPC 툴팁의 월드 아카이브 설명. 대사 섹션이 있으면 설명이 없을 때
/// 원본은 `"(정보 없음)"`을 쓴다.
pub fn npc_lines(world_archive: Option<&str>, has_quote: bool) -> Vec<String> {
    match world_archive.filter(|d| !d.is_empty()) {
        Some(desc) => split_lines(desc),
        None if has_quote => vec!["(정보 없음)".to_string()],
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_literal_and_real_newlines() {
        assert_eq!(split_lines("a\\r\\nb\r\nc\\nd"), vec!["a", "b", "c", "d"]);
        assert_eq!(split_lines("x\\ry"), vec!["x", "y"]);
    }

    #[test]
    fn mob_sections() {
        assert_eq!(
            mob_lines(Some("설명"), Some("도감")),
            vec!["설명", "[몬스터북]", "도감"]
        );
        assert_eq!(mob_lines(None, Some("도감")), vec!["[몬스터북]", "도감"]);
    }

    #[test]
    fn npc_fallback() {
        assert_eq!(npc_lines(None, true), vec!["(정보 없음)"]);
        assert!(npc_lines(None, false).is_empty());
    }
}
