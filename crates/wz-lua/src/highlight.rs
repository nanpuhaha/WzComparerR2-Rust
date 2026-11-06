//! LuaConsole 에디터의 문법 강조 규칙 (`Resources/Lua.xshd`).
//!
//! 원본은 ICSharpCode.TextEditor(WinForms)의 `ISyntaxModeFileProvider`로
//! `Lua.xshd`를 읽어 강조한다. 여기서는 그 설정이 정의하는 **토큰 경계**만
//! 재현한다(색/굵기는 UI 몫).
//!
//! `Lua.xshd`의 규칙과 우선순위:
//! 1. `--[[ … ]]` 블록 주석, `--` 줄 주석
//! 2. `"…"` 문자열, `'…'` 문자열(줄 끝에서 멈춤), `[[ … ]]` 여러 줄 문자열
//! 3. `LanguageKeywords`(and/break/…/while), `AccessKeywords`(self)
//! 4. `Digits`(DarkBlue)
//! 5. `Punctuation` 26종
//!
//! 숫자 정규식은 xshd에 명시되지 않아(구형 `<Digits>` 기본값) `\d+(\.\d+)?`로
//! 고정했다. 이 선택은 문서에 명시한다.

/// 강조 종류.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HighlightKind {
    Comment,
    String,
    Number,
    Punctuation,
    Keyword,
    AccessKeyword,
    Identifier,
    Whitespace,
    Other,
}

/// 강조 토큰. `start`/`end`는 **문자(char) 오프셋**이다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Token {
    pub kind: HighlightKind,
    pub start: usize,
    pub end: usize,
}

/// `Lua.xshd`의 `LanguageKeywords`.
pub const KEYWORDS: [&str; 21] = [
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "if", "in", "local",
    "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
];

/// `Lua.xshd`의 `AccessKeywords`.
pub const ACCESS_KEYWORDS: [&str; 1] = ["self"];

/// `Lua.xshd`의 `Punctuation` 26종.
pub const PUNCTUATION: [char; 26] = [
    '?', ',', '.', ';', '(', ')', '[', ']', '{', '}', '+', '-', '/', '%', '*', '<', '>', '^', '=',
    '~', '!', '|', '&', '@', '$', '/',
];

/// 파일 선두 BOM을 제거한다. 원본은 텍스트를 BOM 없이 읽으므로, 비교/강조
/// 전에 호출한다(토큰 오프셋이 파일 내용과 일치하도록).
pub fn strip_bom(source: &str) -> &str {
    source.strip_prefix('\u{feff}').unwrap_or(source)
}

/// `Lua.xshd`가 지정한 색(이름 → RGB). 색이 없는 종류는 `None`.
///
/// - `Comment` Green `#008000`
/// - `String`/`Char`/`MultiLineString` Red `#FF0000`
/// - `Digits` DarkBlue `#00008B`
/// - `Punctuation` DarkGreen `#006400`
/// - `LanguageKeywords`/`AccessKeywords` Blue `#0000FF`
pub fn color_rgb(kind: HighlightKind) -> Option<(u8, u8, u8)> {
    Some(match kind {
        HighlightKind::Comment => (0x00, 0x80, 0x00),
        HighlightKind::String => (0xFF, 0x00, 0x00),
        HighlightKind::Number => (0x00, 0x00, 0x8B),
        HighlightKind::Punctuation => (0x00, 0x64, 0x00),
        HighlightKind::Keyword | HighlightKind::AccessKeyword => (0x00, 0x00, 0xFF),
        HighlightKind::Identifier | HighlightKind::Whitespace | HighlightKind::Other => {
            return None
        }
    })
}

fn is_ident_start(c: char) -> bool {
    c == '_' || c.is_ascii_alphabetic()
}

fn is_ident_continue(c: char) -> bool {
    c == '_' || c.is_ascii_alphanumeric()
}

/// 소스를 토큰으로 나눈다. 토큰은 소스를 **빈틈없이** 덮는다.
pub fn highlight(source: &str) -> Vec<Token> {
    let chars: Vec<char> = source.chars().collect();
    let len = chars.len();
    let mut tokens = Vec::new();
    let mut i = 0usize;

    while i < len {
        let c = chars[i];
        let start = i;

        // 주석
        if c == '-' && i + 1 < len && chars[i + 1] == '-' {
            if i + 3 < len && chars[i + 2] == '[' && chars[i + 3] == '[' {
                i += 4;
                while i + 1 < len && !(chars[i] == ']' && chars[i + 1] == ']') {
                    i += 1;
                }
                i = (i + 2).min(len);
            } else {
                i += 2;
                while i < len && chars[i] != '\n' {
                    i += 1;
                }
            }
            tokens.push(Token {
                kind: HighlightKind::Comment,
                start,
                end: i,
            });
            continue;
        }

        // 문자열
        if c == '"' {
            i += 1;
            while i < len && chars[i] != '"' {
                i += 1;
            }
            i = (i + 1).min(len);
            tokens.push(Token {
                kind: HighlightKind::String,
                start,
                end: i,
            });
            continue;
        }
        if c == '\'' {
            i += 1;
            while i < len && chars[i] != '\'' && chars[i] != '\n' {
                i += 1;
            }
            if i < len && chars[i] == '\'' {
                i += 1;
            }
            tokens.push(Token {
                kind: HighlightKind::String,
                start,
                end: i,
            });
            continue;
        }
        if c == '[' && i + 1 < len && chars[i + 1] == '[' {
            i += 2;
            while i + 1 < len && !(chars[i] == ']' && chars[i + 1] == ']') {
                i += 1;
            }
            i = (i + 2).min(len);
            tokens.push(Token {
                kind: HighlightKind::String,
                start,
                end: i,
            });
            continue;
        }

        // 숫자
        if c.is_ascii_digit() {
            i += 1;
            while i < len && chars[i].is_ascii_digit() {
                i += 1;
            }
            if i + 1 < len && chars[i] == '.' && chars[i + 1].is_ascii_digit() {
                i += 1;
                while i < len && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            tokens.push(Token {
                kind: HighlightKind::Number,
                start,
                end: i,
            });
            continue;
        }

        // 식별자/키워드
        if is_ident_start(c) {
            i += 1;
            while i < len && is_ident_continue(chars[i]) {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let kind = if ACCESS_KEYWORDS.contains(&word.as_str()) {
                HighlightKind::AccessKeyword
            } else if KEYWORDS.contains(&word.as_str()) {
                HighlightKind::Keyword
            } else {
                HighlightKind::Identifier
            };
            tokens.push(Token {
                kind,
                start,
                end: i,
            });
            continue;
        }

        // 구두점 (중복 '/'는 집합이므로 무해)
        if PUNCTUATION.contains(&c) {
            i += 1;
            tokens.push(Token {
                kind: HighlightKind::Punctuation,
                start,
                end: i,
            });
            continue;
        }

        // 공백
        if c.is_whitespace() {
            i += 1;
            while i < len && chars[i].is_whitespace() {
                i += 1;
            }
            tokens.push(Token {
                kind: HighlightKind::Whitespace,
                start,
                end: i,
            });
            continue;
        }

        i += 1;
        tokens.push(Token {
            kind: HighlightKind::Other,
            start,
            end: i,
        });
    }

    tokens
}

/// 테스트/비교용: `(kind, text)` 목록.
pub fn token_texts(source: &str) -> Vec<(HighlightKind, String)> {
    let chars: Vec<char> = source.chars().collect();
    highlight(source)
        .into_iter()
        .map(|t| (t.kind, chars[t.start..t.end].iter().collect::<String>()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bom_is_stripped_explicitly() {
        assert_eq!(strip_bom("\u{feff}local x"), "local x");
        assert_eq!(strip_bom("local x"), "local x");
    }

    #[test]
    fn tokens_tile_the_source() {
        let src = "local x = 1 -- hi\nreturn x\n";
        let chars: Vec<char> = src.chars().collect();
        let tokens = highlight(src);
        assert_eq!(tokens.first().map(|t| t.start), Some(0));
        assert_eq!(tokens.last().map(|t| t.end), Some(chars.len()));
        for pair in tokens.windows(2) {
            assert_eq!(pair[0].end, pair[1].start);
        }
    }

    #[test]
    fn keywords_and_access() {
        let kinds = token_texts("local self = nil");
        assert_eq!(kinds[0].0, HighlightKind::Keyword);
        assert_eq!(kinds[2].0, HighlightKind::AccessKeyword);
        assert_eq!(kinds[4].0, HighlightKind::Punctuation);
        assert_eq!(kinds[6].0, HighlightKind::Keyword);
    }

    #[test]
    fn numbers_and_punctuation() {
        let got = token_texts("a = 3.14 + 42");
        let kinds: Vec<_> = got.iter().map(|(k, _)| *k).collect();
        assert!(kinds.contains(&HighlightKind::Number));
        let numbers: Vec<_> = got
            .iter()
            .filter(|(k, _)| *k == HighlightKind::Number)
            .map(|(_, t)| t.as_str())
            .collect();
        assert_eq!(numbers, vec!["3.14", "42"]);
        assert!(kinds.contains(&HighlightKind::Punctuation));
    }

    #[test]
    fn comments_win_over_punctuation() {
        let got = token_texts("x = 1 -- note\n--[[ block\nstill ]] y");
        let comments: Vec<_> = got
            .iter()
            .filter(|(k, _)| *k == HighlightKind::Comment)
            .map(|(_, t)| t.as_str())
            .collect();
        assert_eq!(comments[0], "-- note");
        assert_eq!(comments[1], "--[[ block\nstill ]]");
    }

    #[test]
    fn strings_stop_correctly() {
        let got = token_texts("a = \"hi\" .. 'yo'");
        let strings: Vec<_> = got
            .iter()
            .filter(|(k, _)| *k == HighlightKind::String)
            .map(|(_, t)| t.as_str())
            .collect();
        assert_eq!(strings, vec!["\"hi\"", "'yo'"]);
        // 작은따옴표 문자열은 줄 끝에서 멈춘다.
        let got = token_texts("x = 'abc\nnext");
        let strings: Vec<_> = got
            .iter()
            .filter(|(k, _)| *k == HighlightKind::String)
            .map(|(_, t)| t.as_str())
            .collect();
        assert_eq!(strings, vec!["'abc"]);
    }

    #[test]
    fn long_brackets_are_strings() {
        let got = token_texts("s = [[line1\nline2]]");
        let strings: Vec<_> = got
            .iter()
            .filter(|(k, _)| *k == HighlightKind::String)
            .map(|(_, t)| t.as_str())
            .collect();
        assert_eq!(strings, vec!["[[line1\nline2]]"]);
    }
}
