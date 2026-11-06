//! LuaConsole 에디터 패널 — 검증된 `wz_lua::highlight` 토큰을 egui 색상으로 그린다.
//!
//! 원본 `FrmLuaEditor`는 ICSharpCode.TextEditor(WinForms)를 쓴다. 여기서는
//! 같은 `Lua.xshd` 색상 매핑(`wz_lua::color_rgb`)을 egui `LayoutJob`으로
//! 옮겨, 토큰 경계와 색이 그대로 보이게 한다.

/// 토큰 종류를 모두 포함하는 합성 스크립트(원본 예제 파일 대신 사용).
pub const DEFAULT_SCRIPT: &str = r#"-- 주석: TODO 항목
--[[ 여러 줄
     주석 ]]
local self = 42
local text = "문자열" .. 'c'
local multi = [[긴 문자열]]
function greet(name)
    if name ~= nil and name ~= "" then
        return "hello, " .. name
    end
    return nil
end
for i = 1, 10 do
    print(greet("lua") .. tostring(i))
end
local hex = 0xFF
local frac = 3.14
"#;

/// `Lua.xshd` 색을 적용한 egui 텍스트 레이아웃.
pub fn highlighted_job(source: &str, font: egui::FontId) -> egui::text::LayoutJob {
    let chars: Vec<char> = source.chars().collect();
    let mut job = egui::text::LayoutJob::default();
    for token in wz_lua::highlight(source) {
        let text: String = chars[token.start..token.end].iter().collect();
        let mut format = egui::TextFormat {
            font_id: font.clone(),
            ..Default::default()
        };
        if let Some((r, g, b)) = wz_lua::color_rgb(token.kind) {
            format.color = egui::Color32::from_rgb(r, g, b);
        }
        job.append(&text, 0.0, format);
    }
    job
}

/// 스모크/테스트용 요약: `(토큰 수, 색이 지정된 구간 수, 서로 다른 색 수)`.
pub fn summary(source: &str) -> (usize, usize, usize) {
    let tokens = wz_lua::highlight(source);
    let mut colors: Vec<(u8, u8, u8)> = tokens
        .iter()
        .filter_map(|t| wz_lua::color_rgb(t.kind))
        .collect();
    colors.sort_unstable();
    colors.dedup();
    let colored = tokens
        .iter()
        .filter(|t| wz_lua::color_rgb(t.kind).is_some())
        .count();
    (tokens.len(), colored, colors.len())
}

/// 줄 번호 거터 텍스트(`1`부터). 줄 수는 `\n` 개수 + 1.
pub fn gutter_text(source: &str) -> String {
    let lines = source.split('\n').count().max(1);
    (1..=lines)
        .map(|n| n.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

/// 문자 오프셋이 속한 토큰 종류. 경계(끝)는 다음 토큰 시작으로 본다.
pub fn token_at_char(source: &str, char_index: usize) -> Option<wz_lua::HighlightKind> {
    let tokens = wz_lua::highlight(source);
    tokens
        .iter()
        .find(|t| char_index >= t.start && char_index < t.end)
        .or_else(|| tokens.last().filter(|t| char_index == t.end))
        .map(|t| t.kind)
}

/// `줄 L, 열 C · 토큰 Kind` (줄/열은 1부터, 열은 문자 기준).
pub fn cursor_status(source: &str, char_index: usize) -> String {
    let chars: Vec<char> = source.chars().collect();
    let index = char_index.min(chars.len());
    let line_start = chars[..index]
        .iter()
        .rposition(|c| *c == '\n')
        .map(|p| p + 1)
        .unwrap_or(0);
    let line = chars[..index].iter().filter(|c| **c == '\n').count() + 1;
    let column = index - line_start + 1;
    let kind = token_at_char(source, index)
        .map(|k| format!("{k:?}"))
        .unwrap_or_else(|| "-".to_string());
    format!("줄 {line}, 열 {column} · 토큰 {kind}")
}

/// 선택 구간에 포함된 토큰 종류(중복 제거).
pub fn selection_kinds(source: &str, from: usize, to: usize) -> Vec<wz_lua::HighlightKind> {
    let (lo, hi) = (from.min(to), from.max(to));
    if lo == hi {
        return Vec::new();
    }
    let chars: Vec<char> = source.chars().collect();
    let hi = hi.min(chars.len());
    let mut kinds: Vec<wz_lua::HighlightKind> = wz_lua::highlight(source)
        .into_iter()
        .filter(|t| t.start < hi && t.end > lo)
        .map(|t| t.kind)
        .collect();
    kinds.sort_by_key(|k| format!("{k:?}"));
    kinds.dedup();
    kinds
}

pub fn show(ui: &mut egui::Ui) {
    let id = egui::Id::unique("lua_console_source");
    let mut source = ui
        .data_mut(|d| d.get_temp::<String>(id))
        .unwrap_or_else(|| DEFAULT_SCRIPT.to_string());

    ui.horizontal(|ui| {
        ui.label("소스");
        if ui.button("예제로 되돌리기").clicked() {
            source = DEFAULT_SCRIPT.to_string();
        }
        let (tokens, colored, colors) = summary(&source);
        ui.label(format!(
            "토큰 {tokens} · 색 구간 {colored} · 색 {colors}종"
        ));
    });
    // 소스가 바뀔 때만 재토큰화한다(캐시).
    let cache_id = egui::Id::unique("lua_console_tokens");
    let tokens: Vec<wz_lua::Token> = ui.data_mut(|d| {
        let cached: Option<(String, Vec<wz_lua::Token>)> = d.get_temp(cache_id);
        match cached {
            Some((cached_source, tokens)) if cached_source == source => tokens,
            _ => {
                let tokens = wz_lua::highlight(&source);
                d.insert_temp(cache_id, (source.clone(), tokens.clone()));
                tokens
            }
        }
    });

    let gutter = gutter_text(&source);
    let mut cursor_status_line = String::from("줄 1, 열 1");
    ui.horizontal_top(|ui| {
        ui.add(
            egui::Label::new(
                egui::RichText::new(gutter)
                    .monospace()
                    .color(egui::Color32::from_gray(120)),
            )
            .selectable(false),
        );
        let output = egui::TextEdit::multiline(&mut source)
            .desired_rows(8)
            .code_editor()
            .show(ui);
        if let Some(range) = output.cursor_range {
            let primary: usize = range.primary.index.into();
            let secondary: usize = range.secondary.index.into();
            cursor_status_line = cursor_status(&source, primary);
            if primary != secondary {
                let kinds = selection_kinds(&source, primary, secondary);
                let names: Vec<String> = kinds.iter().map(|k| format!("{k:?}")).collect();
                cursor_status_line.push_str(&format!(" · 선택 {}", names.join("/")));
            }
        }
    });
    ui.data_mut(|d| {
        d.insert_temp(id, source.clone());
        d.insert_temp(cache_id, (source.clone(), tokens));
    });
    ui.label(egui::RichText::new(cursor_status_line).weak());
    ui.separator();
    ui.label("강조 미리보기");
    egui::ScrollArea::vertical()
        .max_height(220.0)
        .show(ui, |ui| {
            ui.label(highlighted_job(&source, egui::FontId::monospace(13.0)));
        });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_script_covers_all_colored_kinds() {
        let (tokens, colored, colors) = summary(DEFAULT_SCRIPT);
        assert!(tokens > 40, "tokens {tokens}");
        assert!(colored > 20, "colored {colored}");
        // 주석/문자열/숫자/구두점/키워드(+self) = 5가지 색.
        assert_eq!(colors, 5);
    }

    #[test]
    fn gutter_counts_lines() {
        assert_eq!(gutter_text("a"), "1");
        assert_eq!(gutter_text("a\nb\nc"), "1\n2\n3");
        assert_eq!(gutter_text("a\n"), "1\n2");
        assert_eq!(gutter_text(""), "1");
    }

    #[test]
    fn cursor_maps_to_line_column_and_token() {
        let src = "local x\n-- hi\nreturn x";
        assert_eq!(token_at_char(src, 0), Some(wz_lua::HighlightKind::Keyword));
        assert_eq!(cursor_status(src, 0), "줄 1, 열 1 · 토큰 Keyword");
        // 두 번째 줄 주석
        assert_eq!(
            token_at_char(src, 9),
            Some(wz_lua::HighlightKind::Comment)
        );
        assert_eq!(cursor_status(src, 9), "줄 2, 열 2 · 토큰 Comment");
    }

    #[test]
    fn selection_reports_distinct_kinds() {
        let src = "local x = \"s\"";
        let kinds = selection_kinds(src, 0, src.chars().count());
        assert!(kinds.contains(&wz_lua::HighlightKind::Keyword));
        assert!(kinds.contains(&wz_lua::HighlightKind::String));
        assert_eq!(selection_kinds(src, 3, 3).len(), 0);
    }

    #[test]
    fn job_text_round_trips() {
        let job = highlighted_job(DEFAULT_SCRIPT, egui::FontId::monospace(13.0));
        let chars: Vec<char> = DEFAULT_SCRIPT.chars().collect();
        let tokens = wz_lua::highlight(DEFAULT_SCRIPT);
        let joined: String = tokens
            .iter()
            .map(|t| chars[t.start..t.end].iter().collect::<String>())
            .collect();
        assert_eq!(joined, DEFAULT_SCRIPT);
        // egui는 같은 포맷의 인접 구간을 병합하므로 section 수는 토큰 수 이하다.
        assert!(!job.sections.is_empty());
        assert!(job.sections.len() <= tokens.len());
    }
}
