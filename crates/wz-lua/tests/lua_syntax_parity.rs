//! LuaConsole 문법 강조 규칙(`Lua.xshd`)을 원본 예제 스크립트로 검증한다.
//!
//! C# 하네스가 **원본 `Resources/Lua.xshd`를 파싱해** 키워드/구두점 목록을
//! 얻고, 같은 규칙으로 예제 5개를 토큰화한 결과(`lua_syntax.json`)와 비교한다.
//! 원본 예제 파일은 이 저장소에 포함하지 않으므로, 원본 체크아웃이 있는
//! 환경에서만 실제 토큰 비교를 수행하고 없으면 규칙 집합만 검사한다.

use serde::Deserialize;
use wz_lua::highlight::{highlight, strip_bom, ACCESS_KEYWORDS, KEYWORDS, PUNCTUATION};
use wz_lua::HighlightKind;

#[derive(Deserialize)]
struct Fixture {
    keywords: Vec<String>,
    access_keywords: Vec<String>,
    punctuation: Vec<String>,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    file: String,
    token_count: usize,
    sha256: String,
}

fn kind_name(kind: HighlightKind) -> &'static str {
    match kind {
        HighlightKind::Comment => "Comment",
        HighlightKind::String => "String",
        HighlightKind::Number => "Number",
        HighlightKind::Punctuation => "Punctuation",
        HighlightKind::Keyword => "Keyword",
        HighlightKind::AccessKeyword => "AccessKeyword",
        HighlightKind::Identifier => "Identifier",
        HighlightKind::Whitespace => "Whitespace",
        HighlightKind::Other => "Other",
    }
}

fn fingerprint(source: &str) -> (usize, String) {
    use sha2::{Digest, Sha256};
    let tokens = highlight(strip_bom(source));
    let canonical = tokens
        .iter()
        .map(|t| format!("{}:{}:{}", kind_name(t.kind), t.start, t.end))
        .collect::<Vec<_>>()
        .join("\u{1e}");
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    (tokens.len(), format!("{:x}", hasher.finalize()))
}

#[test]
fn rule_sets_match_lua_xshd() {
    let fixture: Fixture = serde_json::from_str(include_str!("fixtures/lua_syntax.json")).unwrap();
    let mut keywords: Vec<String> = KEYWORDS.iter().map(|s| s.to_string()).collect();
    keywords.sort();
    assert_eq!(keywords, fixture.keywords);
    let access: Vec<String> = ACCESS_KEYWORDS.iter().map(|s| s.to_string()).collect();
    assert_eq!(access, fixture.access_keywords);
    let mut punctuation: Vec<String> = PUNCTUATION.iter().map(|c| c.to_string()).collect();
    punctuation.sort();
    punctuation.dedup();
    assert_eq!(punctuation, fixture.punctuation);
}

#[test]
fn token_stream_matches_csharp_on_original_examples() {
    let fixture: Fixture = serde_json::from_str(include_str!("fixtures/lua_syntax.json")).unwrap();
    let candidates = [
        std::env::var_os("WZ_C2_EXAMPLES").map(std::path::PathBuf::from),
        Some(std::path::PathBuf::from(
            "/Users/jwseo/GitHub/WzComparerR2-seotbeo/WzComparerR2.LuaConsole/Examples",
        )),
    ];
    let Some(dir) = candidates.into_iter().flatten().find(|p| p.is_dir()) else {
        eprintln!("skip: original LuaConsole/Examples not found");
        return;
    };

    let mut checked = 0usize;
    for case in &fixture.cases {
        let path = dir.join(&case.file);
        let Ok(source) = std::fs::read_to_string(&path) else {
            continue;
        };
        let (count, sha) = fingerprint(&source);
        assert_eq!(count, case.token_count, "token count for {}", case.file);
        assert_eq!(sha, case.sha256, "token stream for {}", case.file);
        checked += 1;
    }
    assert_eq!(checked, fixture.cases.len());
}
