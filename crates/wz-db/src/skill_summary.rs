//! Skill 설명 요약 파서 (`SummaryParser.GetSkillSummary`).
//!
//! SkillTooltip은 `String/Skill.img/<id>/desc`를 `SummaryParser`로 변환해
//! 색상 태그/속성 토큰/이스케이프를 처리한다. 이 클라이언트에는 스킬
//! `common`/`level` 수치가 없어 속성 resolver는 항상 실패하는 경로만
//! 구현한다. 즉 C#과 동일하게 미해결 `#prop#` 토큰은 제거되고, `#c`/`#$g`/
//! `#$x`/`#k`/`#fc`/`\n`/`\r`/`\c` 처리는 그대로 수행한다.

const C_START: &str = "#c";
const C_END: &str = "#";
const G_START: &str = "#$g";
const G_END: &str = "#";
const X_START: &str = "#$x";
const X_END: &str = "#";

fn is_property_start(c: char) -> bool {
    c == '_' || c.is_ascii_lowercase() || c.is_ascii_uppercase()
}

fn read_property_length(chars: &[char], start: usize) -> usize {
    if start >= chars.len() || !is_property_start(chars[start]) {
        return 0;
    }
    let mut end = start + 1;
    while end < chars.len() && (is_property_start(chars[end]) || chars[end].is_ascii_digit()) {
        end += 1;
    }
    end - start
}

fn try_read_skill_reference(chars: &[char], start: usize) -> Option<(usize, usize, String)> {
    if start + 1 >= chars.len() || chars[start + 1] != '[' {
        return None;
    }
    let id_start = start + 2;
    let mut cursor = id_start;
    while cursor < chars.len() && chars[cursor].is_ascii_digit() {
        cursor += 1;
    }
    if cursor == id_start || cursor >= chars.len() || chars[cursor] != ']' {
        return None;
    }
    let skill_id: usize = chars[id_start..cursor]
        .iter()
        .collect::<String>()
        .parse()
        .ok()?;
    let property_start = cursor + 1;
    let property_length = read_property_length(chars, property_start);
    if property_length == 0 {
        return None;
    }
    let property: String = chars[property_start..property_start + property_length]
        .iter()
        .collect();
    Some((property_start + property_length, skill_id, property))
}

fn strip_trailing_line_tokens(mut text: String) -> String {
    loop {
        if text.ends_with("\\r") || text.ends_with("\\n") {
            text.truncate(text.len() - 2);
        } else {
            break;
        }
    }
    text
}

/// `SummaryParser.GetSkillSummary`의 resolver 실패 경로를 옮긴다.
pub fn parse_skill_summary(text: &str, _level: i32) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::new();
    let mut idx = 0usize;
    let mut begin_c = false;
    let mut begin_g = false;
    let mut begin_x = false;

    while idx < chars.len() {
        if chars[idx] == '#' {
            if let Some((end, _skill_id, _property)) = try_read_skill_reference(&chars, idx) {
                // resolver가 없으므로 원본과 같이 참조 토큰을 그대로 남긴다.
                out.extend(&chars[idx..end]);
                idx = end;
                continue;
            }
            let len = read_property_length(&chars, idx + 1);
            if idx + 1 < chars.len() && chars[idx + 1] == 'c' {
                begin_c = true;
                out.push_str(C_START);
                idx += 2;
            } else if idx + 1 < chars.len() && chars[idx + 1] == '$' {
                if idx + 2 < chars.len() && chars[idx + 2] == 'g' {
                    begin_g = true;
                    out.push_str(G_START);
                    idx += 3;
                } else if idx + 2 < chars.len() && chars[idx + 2] == 'x' {
                    begin_x = true;
                    out.push_str(X_START);
                    idx += 3;
                }
            } else if idx + 2 < chars.len() && chars[idx + 1] == 'f' && chars[idx + 2] == 'c' {
                if idx + 11 < chars.len() && chars[idx + 11] == '#' {
                    out.extend(&chars[idx..idx + 12]);
                    idx += 12;
                } else if idx + 13 < chars.len() && chars[idx + 13] == '#' {
                    out.extend(&chars[idx..idx + 14]);
                    idx += 14;
                } else {
                    idx += 1;
                }
            } else if idx + 1 < chars.len() && chars[idx + 1] == 'k' {
                out.push_str("#k");
                idx += 2;
            } else if begin_x {
                begin_x = false;
                out.push_str(X_END);
                idx += 1;
            } else if begin_g {
                begin_g = false;
                out.push_str(G_END);
                idx += 1;
            } else if begin_c {
                begin_c = false;
                out.push_str(C_END);
                idx += 1;
            } else if idx + 1 < chars.len() && len == 0 {
                begin_c = true;
                out.push_str(C_START);
                idx += 1;
            } else if len > 0 {
                let key: String = chars[idx + 1..idx + 1 + len].iter().collect();
                if key.chars().all(|c| c.is_ascii_digit()) {
                    out.push_str(&key);
                }
                idx += len + 1;
            } else {
                idx += 1;
            }
        } else if chars[idx] == '\\' {
            if idx + 1 < chars.len() {
                match chars[idx + 1] {
                    'c' => {}
                    'r' => out.push_str("\\r"),
                    'n' => {
                        // 기본 옵션은 EndColorOnNewLine=false.
                        out.push_str("\\n");
                    }
                    '\\' => out.push('\\'),
                    other => out.push(other),
                }
                idx += 2;
            } else {
                idx += 1;
            }
        } else {
            out.push(chars[idx]);
            idx += 1;
        }
    }
    strip_trailing_line_tokens(out.replace('\t', ""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn removes_unresolved_properties_and_handles_colors() {
        assert_eq!(
            parse_skill_summary("MP #mpCon 소비, #damage% 데미지", 1),
            "MP  소비, % 데미지"
        );
        assert_eq!(parse_skill_summary("#c강조#됩니다", 1), "#c강조#됩니다");
    }

    #[test]
    fn handles_escapes_and_trailing_newlines() {
        assert_eq!(parse_skill_summary("줄1\\n줄2\\r\\n", 1), "줄1\\n줄2");
        assert_eq!(parse_skill_summary("a\\cb\\\\c", 1), "ab\\c");
    }
}
