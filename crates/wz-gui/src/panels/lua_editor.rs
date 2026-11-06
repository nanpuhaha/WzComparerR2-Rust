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
    ui.add(
        egui::TextEdit::multiline(&mut source)
            .desired_rows(8)
            .font(egui::TextStyle::Monospace),
    );
    ui.data_mut(|d| d.insert_temp(id, source.clone()));
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
