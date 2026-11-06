//! LuaConsole 콘솔 패널 — 검증된 `wz_lua::Sandbox`로 스크립트를 실행한다.
//!
//! 원본 `FrmConsole`은 WinForms 콘솔 창이다. 여기서는 출력 이력/입력/Run 버튼만
//! egui로 재현하고, 실행은 실제 검증된 샌드박스에 위임한다.

use std::cell::RefCell;
use std::path::Path;
use wz_lua::Sandbox;

thread_local! {
    static SANDBOX: RefCell<Option<Sandbox>> = const { RefCell::new(None) };
}

/// 실행 결과 모델.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecutionOutcome {
    /// 청크가 돌려준 값들(문자열화).
    Values(Vec<String>),
    /// 문법/런타임 오류.
    Error(String),
}

/// 샌드박스를 만들고 `env`/`each`/`host_call`을 준비한다.
pub fn new_sandbox() -> Option<Sandbox> {
    let mut sandbox = Sandbox::new().ok()?;
    sandbox
        .init(None, None, None, None, Path::new("."))
        .ok()?;
    Some(sandbox)
}

/// 청크를 실행해 결과/오류를 돌려준다.
pub fn execute(sandbox: &mut Sandbox, source: &str) -> ExecutionOutcome {
    match sandbox.do_string(source) {
        Ok(values) => ExecutionOutcome::Values(values),
        Err(error) => ExecutionOutcome::Error(error.to_string()),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Command,
    Output,
    Error,
    System,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConsoleEntry {
    pub kind: EntryKind,
    pub text: String,
}

/// 콘솔 이력/입력 상태.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ConsoleModel {
    pub history: Vec<ConsoleEntry>,
    pub input: String,
    /// 실행한 명령(최근 순 아님, 입력 순서).
    pub commands: Vec<String>,
    /// ↑/↓ 탐색 위치.
    pub recall: Option<usize>,
}

impl ConsoleModel {
    pub fn new() -> Self {
        Self::default()
    }

    /// 입력을 실행하고 이력에 명령/결과를 남긴다. 빈 입력은 무시.
    pub fn submit(&mut self, sandbox: &mut Sandbox, source: &str) -> Option<ExecutionOutcome> {
        let source = source.trim();
        if source.is_empty() {
            return None;
        }
        self.history.push(ConsoleEntry {
            kind: EntryKind::Command,
            text: source.to_string(),
        });
        self.commands.push(source.to_string());
        self.recall = None;
        let outcome = execute(sandbox, source);
        // `env:Write/WriteLine` 출력을 먼저 이력에 남긴다.
        let env_output = sandbox.drain_output();
        if !env_output.is_empty() {
            let mut lines: Vec<&str> = env_output.split('\n').collect();
            if lines.last() == Some(&"") {
                lines.pop();
            }
            for line in lines {
                self.history.push(ConsoleEntry {
                    kind: EntryKind::Output,
                    text: line.to_string(),
                });
            }
        }
        match &outcome {
            ExecutionOutcome::Values(values) => {
                for value in values {
                    self.history.push(ConsoleEntry {
                        kind: EntryKind::Output,
                        text: value.clone(),
                    });
                }
                if values.is_empty() {
                    self.history.push(ConsoleEntry {
                        kind: EntryKind::System,
                        text: "(반환값 없음)".to_string(),
                    });
                }
            }
            ExecutionOutcome::Error(error) => {
                self.history.push(ConsoleEntry {
                    kind: EntryKind::Error,
                    text: error.clone(),
                });
            }
        }
        self.input.clear();
        Some(outcome)
    }

    /// ↑ — 직전 명령을 입력으로 가져온다.
    pub fn recall_prev(&mut self) -> Option<&str> {
        if self.commands.is_empty() {
            return None;
        }
        let index = match self.recall {
            None => self.commands.len() - 1,
            Some(0) => 0,
            Some(i) => i - 1,
        };
        self.recall = Some(index);
        self.input = self.commands[index].clone();
        Some(self.input.as_str())
    }

    /// ↓ — 다음 명령으로 이동. 마지막이면 입력을 비운다.
    pub fn recall_next(&mut self) -> Option<&str> {
        match self.recall {
            None => None,
            Some(i) if i + 1 < self.commands.len() => {
                self.recall = Some(i + 1);
                self.input = self.commands[i + 1].clone();
                Some(self.input.as_str())
            }
            Some(_) => {
                self.recall = None;
                self.input.clear();
                None
            }
        }
    }
}

/// 패널 렌더. `Run`/Enter로 청크를 실행한다.
pub fn show(ui: &mut egui::Ui) {
    let model_id = egui::Id::unique("lua_console_model");
    let mut model = ui
        .data_mut(|d| d.get_temp::<ConsoleModel>(model_id))
        .unwrap_or_else(|| {
            let mut model = ConsoleModel::new();
            model.history.push(ConsoleEntry {
                kind: EntryKind::System,
                text: "LuaConsole — 검증된 wz_lua 샌드박스에서 실행합니다.".to_string(),
            });
            model
        });

    // ↑/↓로 명령 이력을 탐색한다.
    if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
        model.recall_prev();
    }
    if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
        model.recall_next();
    }

    ui.horizontal(|ui| {
        ui.label("Lua 실행");
        let run = ui.button("Run").clicked();
        let enter = ui.input(|i| i.key_pressed(egui::Key::Enter));
        let response = ui.add(
            egui::TextEdit::singleline(&mut model.input)
                .hint_text("return 1+2")
                .desired_width(360.0),
        );
        if response.lost_focus() && enter {
            // TextEdit가 Enter를 소비한 경우.
        }
        if run || (enter && response.has_focus()) {
            let outcome = SANDBOX.with(|slot| {
                let mut slot = slot.borrow_mut();
                if slot.is_none() {
                    *slot = new_sandbox();
                }
                match slot.as_mut() {
                    Some(sandbox) => model.submit(sandbox, &model.input.clone()),
                    None => {
                        model.history.push(ConsoleEntry {
                            kind: EntryKind::Error,
                            text: "Lua 샌드박스를 초기화하지 못했습니다".to_string(),
                        });
                        None
                    }
                }
            });
            let _ = outcome;
        }
    });
    ui.separator();
    egui::ScrollArea::vertical()
        .max_height(180.0)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            for entry in &model.history {
                let text = match entry.kind {
                    EntryKind::Command => format!("> {}", entry.text),
                    EntryKind::Output => format!("  {}", entry.text),
                    EntryKind::Error => format!("! {}", entry.text),
                    EntryKind::System => entry.text.clone(),
                };
                let rich = match entry.kind {
                    EntryKind::Command => egui::RichText::new(text).monospace().strong(),
                    EntryKind::Output => egui::RichText::new(text).monospace(),
                    EntryKind::Error => egui::RichText::new(text)
                        .monospace()
                        .color(egui::Color32::from_rgb(220, 80, 80)),
                    EntryKind::System => egui::RichText::new(text)
                        .monospace()
                        .color(egui::Color32::from_gray(150)),
                };
                ui.label(rich);
            }
        });

    ui.data_mut(|d| d.insert_temp(model_id, model));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sandbox() -> Sandbox {
        new_sandbox().expect("sandbox")
    }

    #[test]
    fn executes_and_returns_values() {
        let mut sb = sandbox();
        assert_eq!(
            execute(&mut sb, "return 1+2, 'lua'"),
            ExecutionOutcome::Values(vec!["3".into(), "lua".into()])
        );
    }

    #[test]
    fn runtime_error_is_reported() {
        let mut sb = sandbox();
        match execute(&mut sb, "error('boom')") {
            ExecutionOutcome::Error(message) => assert!(message.contains("boom"), "{message}"),
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn history_records_command_and_output() {
        let mut sb = sandbox();
        let mut model = ConsoleModel::new();
        model.input = "return 'hello'".into();
        let outcome = model.submit(&mut sb, "return 'hello'");
        assert_eq!(outcome, Some(ExecutionOutcome::Values(vec!["hello".into()])));
        assert_eq!(model.history.len(), 2);
        assert_eq!(model.history[0].kind, EntryKind::Command);
        assert_eq!(model.history[0].text, "return 'hello'");
        assert_eq!(model.history[1].kind, EntryKind::Output);
        assert_eq!(model.history[1].text, "hello");
        assert!(model.input.is_empty());
    }

    #[test]
    fn empty_input_is_ignored_and_errors_are_recorded() {
        let mut sb = sandbox();
        let mut model = ConsoleModel::new();
        assert_eq!(model.submit(&mut sb, "   "), None);
        assert!(model.history.is_empty());
        model.submit(&mut sb, "error('nope')");
        assert_eq!(model.history.len(), 2);
        assert_eq!(model.history[1].kind, EntryKind::Error);
        assert!(model.history[1].text.contains("nope"));
    }

    #[test]
    fn command_history_recalls_up_and_down() {
        let mut sb = sandbox();
        let mut model = ConsoleModel::new();
        model.submit(&mut sb, "return 1");
        model.submit(&mut sb, "return 2");
        assert_eq!(model.commands, vec!["return 1", "return 2"]);
        assert_eq!(model.recall_prev(), Some("return 2"));
        assert_eq!(model.recall_prev(), Some("return 1"));
        assert_eq!(model.recall_prev(), Some("return 1"));
        assert_eq!(model.recall_next(), Some("return 2"));
        assert_eq!(model.recall_next(), None);
        assert!(model.input.is_empty());
    }

    #[test]
    fn env_writeline_output_is_recorded_before_return_values() {
        let mut sb = sandbox();
        let mut model = ConsoleModel::new();
        model.submit(&mut sb, "env:WriteLine('hi {0}', 'lua') return 7");
        let kinds: Vec<EntryKind> = model.history.iter().map(|e| e.kind).collect();
        assert_eq!(
            kinds,
            vec![EntryKind::Command, EntryKind::Output, EntryKind::Output]
        );
        assert_eq!(model.history[1].text, "hi lua");
        assert_eq!(model.history[2].text, "7");
    }

    #[test]
    fn each_and_host_call_are_available() {
        let mut sb = sandbox();
        let outcome = execute(
            &mut sb,
            "local list = host_call(function(a,b) return a+b end, 2, 5)\nreturn list",
        );
        assert_eq!(outcome, ExecutionOutcome::Values(vec!["7".into()]));
    }
}
