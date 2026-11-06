//! `PluginContext.AddRibbonBar`의 결정적 배치 모델.
//!
//! 원본은 DevComponents RibbonBar(WinForms)를 만들고 패널에 넣는다. 컨트롤을
//! 만들 수 없으므로 **좌표/순서 규칙**만 옮긴다.
//!
//! 원본 동작(`PluginContext.AddRibbonBar`):
//! 1. 탭 이름을 `OrdinalIgnoreCase`로 찾아 `RibbonPanel`을 얻는다
//! 2. `ribbonPanel.Controls[0]`을 기준(lastBar)으로
//!    `Location = (last.X + last.Width, last.Y)` (인덱스 0 = 최근 추가된 바)
//! 3. `Size = (max(1, bar.Width), last.Height)`
//! 4. `Controls.Add` 후 `SetChildIndex(bar, 0)` → 새 바가 인덱스 0
//!
//! 따라서 두 번째 추가부터는 직전 바의 오른쪽에 이어 붙는다.

/// 패널에 놓인 리본 바. `x/y/width/height`는 WinForms 좌표.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RibbonBar {
    pub text: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RibbonPanel {
    pub tab_name: String,
    /// 인덱스 0이 `Controls[0]`(가장 최근에 추가된 바).
    pub bars: Vec<RibbonBar>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RibbonError {
    /// 원본 `throw new Exception("无法找到RibbonPanel。")`.
    PanelNotFound(String),
    /// 기준 바가 없는 패널(원본은 `Controls[0]`에서 예외).
    EmptyPanel(String),
}

impl std::fmt::Display for RibbonError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RibbonError::PanelNotFound(tab) => write!(f, "无法找到RibbonPanel。({tab})"),
            RibbonError::EmptyPanel(tab) => write!(f, "empty panel ({tab})"),
        }
    }
}

impl std::error::Error for RibbonError {}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RibbonModel {
    pub panels: Vec<RibbonPanel>,
}

impl RibbonModel {
    pub fn new(panels: Vec<RibbonPanel>) -> Self {
        Self { panels }
    }

    /// `PluginContext.AddRibbonBar(tabName, barText)` — 성공 시 추가된 바.
    pub fn add_bar(
        &mut self,
        tab_name: &str,
        text: &str,
        width: i32,
    ) -> Result<RibbonBar, RibbonError> {
        let panel = self
            .panels
            .iter_mut()
            .find(|p| p.tab_name.eq_ignore_ascii_case(tab_name))
            .ok_or_else(|| RibbonError::PanelNotFound(tab_name.to_string()))?;
        let last = panel
            .bars
            .first()
            .ok_or_else(|| RibbonError::EmptyPanel(tab_name.to_string()))?;
        let bar = RibbonBar {
            text: text.to_string(),
            x: last.x + last.width,
            y: last.y,
            width: width.max(1),
            height: last.height,
        };
        panel.bars.insert(0, bar.clone());
        Ok(bar)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn model() -> RibbonModel {
        RibbonModel::new(vec![RibbonPanel {
            tab_name: "Plugin".into(),
            bars: vec![RibbonBar {
                text: "Base".into(),
                x: 0,
                y: 5,
                width: 120,
                height: 24,
            }],
        }])
    }

    #[test]
    fn bars_append_to_the_right_and_insert_at_front() {
        let mut model = model();
        let a = model.add_bar("Plugin", "A", 90).unwrap();
        assert_eq!((a.x, a.y, a.width, a.height), (120, 5, 90, 24));
        let b = model.add_bar("Plugin", "B", 30).unwrap();
        // 기준은 Controls[0] = A.
        assert_eq!((b.x, b.y, b.width), (210, 5, 30));
        let panel = &model.panels[0];
        assert_eq!(panel.bars.len(), 3);
        assert_eq!(panel.bars[0].text, "B");
        assert_eq!(panel.bars[1].text, "A");
    }

    #[test]
    fn tab_lookup_is_case_insensitive() {
        let mut model = model();
        assert!(model.add_bar("plugin", "A", 10).is_ok());
        assert!(model.add_bar("PLUGIN", "B", 10).is_ok());
    }

    #[test]
    fn width_is_clamped_to_one() {
        let mut model = model();
        let bar = model.add_bar("Plugin", "zero", 0).unwrap();
        assert_eq!(bar.width, 1);
    }

    #[test]
    fn missing_tab_errors() {
        let mut model = model();
        assert_eq!(
            model.add_bar("Missing", "X", 10),
            Err(RibbonError::PanelNotFound("Missing".into()))
        );
        let mut empty = RibbonModel::new(vec![RibbonPanel {
            tab_name: "Plugin".into(),
            bars: Vec::new(),
        }]);
        assert!(matches!(
            empty.add_bar("Plugin", "X", 10),
            Err(RibbonError::EmptyPanel(_))
        ));
    }
}
