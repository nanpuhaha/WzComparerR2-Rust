//! 메인 윈도우의 탭/명령 수명주기 모델 (`SuperTabControl`, 리본 버튼).
//!
//! WinForms/DevComponents 컨트롤은 만들 수 없으므로 **관찰 가능한 상태 전이**만
//! 옮긴다. 원본 근거:
//! - `PluginContext.AddTab`은 탭을 **끝에 추가**하고 `SelectedTab`을 바꾸지 않는다.
//!   (첫 탭은 컨트롤이 자동 선택한다는 전제 — 아래 문서 참고)
//! - 메인 폼은 선택/로딩 상태에 따라 버튼 `Enabled`를 토글한다
//!   (`MainForm.cs:1388` 등).
//! - `PluginManager.PluginOnLoad`는 `loadedPlugins` 순서대로 `OnLoad()`를
//!   호출하고, 예외는 플러그인별로 잡아 계속 진행한다.

/// 탭 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tab {
    pub name: String,
}

/// `SuperTabControl`의 탭 목록/선택 상태.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TabHost {
    pub tabs: Vec<Tab>,
    selected: Option<usize>,
}

impl TabHost {
    pub fn new() -> Self {
        Self::default()
    }

    /// `AddTab(tabName)` — 끝에 추가. 첫 탭이면 자동 선택.
    pub fn add_tab(&mut self, name: &str) -> usize {
        self.tabs.push(Tab {
            name: name.to_string(),
        });
        let index = self.tabs.len() - 1;
        if self.selected.is_none() {
            self.selected = Some(index);
        }
        index
    }

    /// 탭 선택. 범위를 벗어나면 `false`(상태 변화 없음).
    pub fn select(&mut self, index: usize) -> bool {
        if index < self.tabs.len() {
            self.selected = Some(index);
            true
        } else {
            false
        }
    }

    pub fn selected_index(&self) -> Option<usize> {
        self.selected
    }

    pub fn selected_name(&self) -> Option<&str> {
        self.selected
            .and_then(|i| self.tabs.get(i))
            .map(|t| t.name.as_str())
    }
}

/// 리본 버튼 하나.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Command {
    pub id: String,
    pub requires_selection: bool,
    pub enabled: bool,
}

/// 명령 등록/활성 상태/디스패치.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommandRegistry {
    pub commands: Vec<Command>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 등록 순서가 곧 디스패치/표시 순서다.
    pub fn register(&mut self, id: &str, requires_selection: bool) {
        self.commands.push(Command {
            id: id.to_string(),
            requires_selection,
            enabled: !requires_selection,
        });
    }

    /// 선택 노드 유무가 바뀔 때 `Enabled` 재계산. 변경된 `(id, enabled)` 반환.
    pub fn on_selection_changed(&mut self, has_selection: bool) -> Vec<(String, bool)> {
        let mut changed = Vec::new();
        for command in &mut self.commands {
            let enabled = !command.requires_selection || has_selection;
            if command.enabled != enabled {
                command.enabled = enabled;
                changed.push((command.id.clone(), enabled));
            }
        }
        changed
    }

    pub fn is_enabled(&self, id: &str) -> Option<bool> {
        self.commands.iter().find(|c| c.id == id).map(|c| c.enabled)
    }

    /// 버튼 클릭. 비활성/미등록이면 `false`.
    pub fn dispatch(&self, id: &str) -> bool {
        self.is_enabled(id) == Some(true)
    }

    /// 활성 명령 id(등록 순서).
    pub fn enabled_ids(&self) -> Vec<&str> {
        self.commands
            .iter()
            .filter(|c| c.enabled)
            .map(|c| c.id.as_str())
            .collect()
    }
}

/// `PluginOnLoad` 결과.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OnLoadOutcome {
    /// 실제로 호출된 순서(로드 순서와 동일).
    pub ran: Vec<String>,
    /// `(plugin name, error message)` — 예외가 나도 다음 플러그인을 계속한다.
    pub failures: Vec<(String, String)>,
}

/// `loadedPlugins` 순서대로 `OnLoad()`를 호출한다.
///
/// `plugins`는 `(name, on_load 결과)`. `Err(message)`면 실패로 기록하고 계속한다.
pub fn plugin_on_load(plugins: &[(String, Result<(), String>)]) -> OnLoadOutcome {
    let mut outcome = OnLoadOutcome::default();
    for (name, result) in plugins {
        outcome.ran.push(name.clone());
        if let Err(error) = result {
            outcome.failures.push((name.clone(), error.clone()));
        }
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_tab_is_selected_and_later_adds_do_not_change_selection() {
        let mut host = TabHost::new();
        assert_eq!(host.add_tab("WzComparerR2"), 0);
        assert_eq!(host.selected_name(), Some("WzComparerR2"));
        assert_eq!(host.add_tab("Avatar"), 1);
        assert_eq!(host.selected_index(), Some(0));
        assert!(host.select(1));
        assert_eq!(host.selected_name(), Some("Avatar"));
        assert!(!host.select(9));
        assert_eq!(host.selected_index(), Some(1));
    }

    #[test]
    fn selection_gates_commands_and_dispatch_follows_registration() {
        let mut registry = CommandRegistry::new();
        registry.register("openWz", false);
        registry.register("saveNode", true);
        registry.register("closeTab", true);
        assert_eq!(registry.enabled_ids(), vec!["openWz"]);
        assert_eq!(
            registry.on_selection_changed(true),
            vec![
                ("saveNode".to_string(), true),
                ("closeTab".to_string(), true)
            ]
        );
        assert_eq!(
            registry.enabled_ids(),
            vec!["openWz", "saveNode", "closeTab"]
        );
        assert!(registry.dispatch("saveNode"));
        assert!(!registry.dispatch("missing"));
        registry.on_selection_changed(false);
        assert!(!registry.dispatch("saveNode"));
        assert!(registry.dispatch("openWz"));
    }

    #[test]
    fn on_load_continues_after_failure() {
        let plugins = vec![
            ("Alpha".to_string(), Ok(())),
            ("Beta".to_string(), Err("boom".to_string())),
            ("Gamma".to_string(), Ok(())),
        ];
        let outcome = plugin_on_load(&plugins);
        assert_eq!(outcome.ran, vec!["Alpha", "Beta", "Gamma"]);
        assert_eq!(
            outcome.failures,
            vec![("Beta".to_string(), "boom".to_string())]
        );
    }
}
