//! `WzComparerR2.Text.Parser.Parse` — 아이템/스킬 설명의 서식 문자열 파서.
//!
//! 툴팁 문자열은 `#c`(주황), `#fcAARRGGBB#`(색), `#$r`(커스텀 색),
//! `#$^f`/`#$$`(폰트 스택), `#k`(색 복원), `#@id/w/h@`(이미지),
//! `\n`/`\r`(개행/무시) 토큰을 쓴다. 이 모듈은 GDI와 무관하게 **run 경계와
//! 토큰 상태만** 계산한다.
//!
//! 원본의 특이점을 그대로 유지한다.
//! - `\` 뒤에 오는 인식 불가 문자는 `\0`이 되어 **버려진다**.
//! - 문자열 끝의 단독 `\`는 `#`로 바뀌어 색 토글로 처리된다.
//! - `\r`(원문 또는 `\r` 이스케이프)은 개행이 아니라 **무시**된다.
//! - `#fc` 뒤 `0x`는 건너뛰고, 8자리+`#`가 아니면 토큰 전체를 무시한다.
//! - `#@` 이미지는 `id/width/height` 중 파싱되는 만큼만 쓰고 실패하면 버린다.
//! - `#` 뒤에 `c`가 없으면 색 스택을 pop하고, 스택에 색이 하나뿐이면 push한다.

/// 파서가 만드는 요소.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DocElement {
    Span(Span),
    LineBreak,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Span {
    pub text: String,
    pub color_id: String,
    pub font_id: String,
    pub image_id: Option<String>,
    pub image_width: i32,
    pub image_height: i32,
}

impl Span {
    pub fn is_image(&self) -> bool {
        self.image_id.as_deref().is_some_and(|s| !s.is_empty())
    }
}

struct State {
    elements: Vec<DocElement>,
    sb: Vec<char>,
    color_stack: Vec<String>,
    font_stack: Vec<String>,
    offset: usize,
}

impl State {
    fn flush(&mut self) {
        if self.offset < self.sb.len() {
            let text: String = self.sb[self.offset..].iter().collect();
            self.elements.push(DocElement::Span(Span {
                text,
                color_id: self.color_stack.last().cloned().unwrap_or_default(),
                font_id: self.font_stack.last().cloned().unwrap_or_default(),
                ..Default::default()
            }));
            self.offset = self.sb.len();
        }
    }

    fn append_image(&mut self, spec: &str) {
        let split: Vec<&str> = spec.split('/').collect();
        let parsed: Option<(i32, i32, i32)> = match split.len() {
            1 => split[0].parse().ok().map(|id| (id, 0, 0)),
            2 => match (split[0].parse(), split[1].parse()) {
                (Ok(id), Ok(width)) => Some((id, width, 0)),
                _ => None,
            },
            3 => match (split[0].parse(), split[1].parse(), split[2].parse()) {
                (Ok(id), Ok(width), Ok(height)) => Some((id, width, height)),
                _ => None,
            },
            _ => None,
        };
        if let Some((id, width, height)) = parsed {
            self.elements.push(DocElement::Span(Span {
                text: String::new(),
                color_id: self.color_stack.last().cloned().unwrap_or_default(),
                font_id: self.font_stack.last().cloned().unwrap_or_default(),
                image_id: Some(id.to_string()),
                image_width: width,
                image_height: height,
            }));
        }
    }
}

/// 서식 문자열을 run 목록으로 분해한다.
pub fn parse(format: &str) -> Vec<DocElement> {
    let chars: Vec<char> = format.chars().collect();
    let mut st = State {
        elements: Vec::new(),
        sb: Vec::new(),
        color_stack: vec![String::new()],
        font_stack: vec![String::new()],
        offset: 0,
    };
    let len = chars.len();
    let mut pos = 0usize;

    while pos < len {
        let mut cur = chars[pos];
        pos += 1;
        if cur == '\\' {
            if pos < len {
                let escaped = chars[pos];
                pos += 1;
                cur = match escaped {
                    'r' => '\r',
                    'n' => '\n',
                    _ => '\0',
                };
            } else {
                cur = '#';
            }
        }

        match cur {
            '#' => {
                if pos < len && chars[pos] == 'c' {
                    st.flush();
                    if st.color_stack.last().map(String::as_str) != Some("c") {
                        st.color_stack.push("c".to_string());
                    }
                    pos += 1;
                } else if pos + 1 < len && chars[pos] == 'f' && chars[pos + 1] == 'c' {
                    st.flush();
                    if pos + 3 < len && chars[pos + 2] == '0' && chars[pos + 3] == 'x' {
                        pos += 2;
                    }
                    if pos + 10 < len && chars[pos + 10] == '#' {
                        let color: String = chars[pos + 2..pos + 10].iter().collect();
                        st.color_stack.push(color);
                        pos += 2 + 8 + 1;
                    }
                    // 그 외에는 토큰 전체를 무시한다(원본 `break`).
                } else if pos < len && chars[pos] == 'k' {
                    st.flush();
                    if st.color_stack.last().map(String::as_str) != Some("")
                        && st.color_stack.len() > 1
                    {
                        st.color_stack.pop();
                    }
                    pos += 1;
                } else if pos < len && chars[pos] == '$' && pos + 1 < len {
                    let font_break = if pos + 2 < len {
                        if chars[pos + 1] == '^' {
                            st.flush();
                            st.font_stack.push(chars[pos + 1..pos + 3].iter().collect());
                            pos += 3;
                            true
                        } else if chars[pos + 1] == '$' {
                            st.flush();
                            if st.font_stack.len() > 1 {
                                st.font_stack.pop();
                            }
                            pos += 2;
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    };
                    if !font_break {
                        st.flush();
                        let color: String = chars[pos..(pos + 2).min(len)].iter().collect();
                        st.color_stack.push(color);
                        pos += 2;
                    }
                } else if pos < len && chars[pos] == '@' && pos + 2 < len {
                    let mut id = String::new();
                    id.push(chars[pos + 1]);
                    pos += 2;
                    while pos < len && chars[pos] != '@' {
                        id.push(chars[pos]);
                        if pos + 1 < len {
                            pos += 1;
                        } else {
                            break;
                        }
                    }
                    if pos < len {
                        pos += 1;
                    }
                    st.flush();
                    st.append_image(&id);
                } else if st.color_stack.len() == 1 {
                    st.flush();
                    st.color_stack.push("c".to_string());
                } else {
                    st.flush();
                    if st.color_stack.len() > 1 {
                        st.color_stack.pop();
                    }
                }
            }
            '\r' => {}
            '\n' => {
                st.flush();
                st.elements.push(DocElement::LineBreak);
            }
            '\0' => {}
            other => st.sb.push(other),
        }
    }

    st.flush();
    st.elements
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spans(text: &str) -> Vec<Span> {
        parse(text)
            .into_iter()
            .map(|e| match e {
                DocElement::Span(s) => s,
                DocElement::LineBreak => Span {
                    text: "<br>".into(),
                    ..Default::default()
                },
            })
            .collect()
    }

    #[test]
    fn plain_text_is_one_span() {
        let got = spans("hello");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].text, "hello");
        assert_eq!(got[0].color_id, "");
    }

    #[test]
    fn color_tags_split_runs() {
        let got = spans("#c빨강#일반");
        assert_eq!(got.len(), 2, "{got:?}");
        assert_eq!(
            (got[0].text.as_str(), got[0].color_id.as_str()),
            ("빨강", "c")
        );
        assert_eq!(
            (got[1].text.as_str(), got[1].color_id.as_str()),
            ("일반", "")
        );
    }

    #[test]
    fn fc_token_requires_8_digits_and_hash() {
        let got = spans("#fcFF00FF00#x");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].text, "x");
        assert_eq!(got[0].color_id, "FF00FF00");
        // 8자리+`#`가 아니면 토큰 전체를 무시한다.
        // 형식이 맞지 않으면 `break`만 하고 strPos를 되돌리지 않아, 토큰 글자가
        // 그대로 본문에 남는다(원본 동작).
        let got = spans("#fcFF00#x");
        assert_eq!(got.len(), 2);
        assert_eq!(
            (got[0].text.as_str(), got[0].color_id.as_str()),
            ("fcFF00", "")
        );
        // 토큰이 무시된 뒤의 `#`는 스택이 하나라 `c`를 push한다.
        assert_eq!((got[1].text.as_str(), got[1].color_id.as_str()), ("x", "c"));
    }

    #[test]
    fn k_pops_color_and_bare_hash_toggles() {
        // `#a`는 스택이 2개라 pop하고, 그 다음 `#b`가 다시 `c`를 push한다.
        let got = spans("#c#a#b#c");
        assert_eq!(got[0].color_id, "");
        assert_eq!(got[1].color_id, "c");
        // `#a`는 색이 하나뿐이라 다시 `c`를 push한다(원본 else 분기).
        let got = spans("#c#k#a");
        assert_eq!(got.len(), 1, "{got:?}");
        assert_eq!(got[0].text, "a", "{got:?}");
        assert_eq!(got[0].color_id, "c", "{got:?}");
    }

    #[test]
    fn image_token() {
        let got = spans("#@123/45/67@x");
        assert_eq!(got[0].image_id.as_deref(), Some("123"));
        assert_eq!((got[0].image_width, got[0].image_height), (45, 67));
        assert!(got[0].text.is_empty());
        assert_eq!(got[1].text, "x");
        // 파싱 불가면 이미지를 버린다.
        let got = spans("#@abc@x");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].text, "x");
    }

    #[test]
    fn escapes_follow_original_quirks() {
        // `\n`만 개행, `\r`과 raw `\r`은 무시
        assert!(parse("a\\nb").contains(&DocElement::LineBreak));
        assert_eq!(spans("a\\rb").len(), 1);
        assert_eq!(spans("a\rb")[0].text, "ab");
        // 인식 불가 이스케이프는 버려진다.
        assert_eq!(spans("a\\zb")[0].text, "ab");
        // 단독 `\`는 `#`로 바뀌어 색 토글이 된다.
        let got = spans("a\\");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].text, "a");
        assert_eq!(got[0].color_id, "");
    }

    #[test]
    fn font_stack_round_trip() {
        let got = spans("#$^fa#$$b");
        assert_eq!(got[1].text, "b");
        assert_eq!(got[1].font_id, "");
    }
}
