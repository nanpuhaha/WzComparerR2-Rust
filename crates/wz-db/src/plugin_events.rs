//! `PluginContext` 이벤트 구독과 WinForms `Invalidate/Update` 병합 모델.
//!
//! 원본은 C# 이벤트(멀티캐스트 델리게이트)와 `Control.Invalidate()`를 쓴다.
//! 여기서는 UI 없이 **관찰 가능한 규칙**만 옮긴다.
//!
//! - 구독 순서 = 호출 순서(멀티캐스트 델리게이트)
//! - `-=`는 **마지막으로 일치하는** 핸들러를 제거한다(`Delegate.Remove`)
//! - `Invalidate(rect)`는 대기 영역과 **합집합**으로 병합되고, `Update()`가
//!   한 번의 페인트로 소비한다
//! - 전체 무효화(`Invalidate()`)는 어떤 부분 영역도 흡수한다

/// `PluginContext`가 노출하는 이벤트.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PluginEvent {
    SelectedNode1Changed,
    SelectedNode2Changed,
    SelectedNode3Changed,
    WzOpened,
    WzClosing,
    MainStyleChanged,
}

/// 구독 목록(순서 보존).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventBus {
    subscriptions: Vec<(PluginEvent, u64)>,
}

impl EventBus {
    pub fn new() -> Self {
        Self::default()
    }

    /// `event += handler`.
    pub fn subscribe(&mut self, event: PluginEvent, handler: u64) {
        self.subscriptions.push((event, handler));
    }

    /// `event -= handler` — 마지막 일치 항목을 제거한다(원본 `Delegate.Remove`).
    pub fn unsubscribe(&mut self, event: PluginEvent, handler: u64) -> bool {
        if let Some(index) = self
            .subscriptions
            .iter()
            .rposition(|(e, h)| *e == event && *h == handler)
        {
            self.subscriptions.remove(index);
            true
        } else {
            false
        }
    }

    /// 이벤트 발생 시 호출되는 핸들러(구독 순서).
    pub fn emit(&self, event: PluginEvent) -> Vec<u64> {
        self.subscriptions
            .iter()
            .filter(|(e, _)| *e == event)
            .map(|(_, h)| *h)
            .collect()
    }

    pub fn handler_count(&self, event: PluginEvent) -> usize {
        self.subscriptions
            .iter()
            .filter(|(e, _)| *e == event)
            .count()
    }
}

/// 무효화 영역.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn union(self, other: Rect) -> Rect {
        let x0 = self.x.min(other.x);
        let y0 = self.y.min(other.y);
        let x1 = (self.x + self.w).max(other.x + other.w);
        let y1 = (self.y + self.h).max(other.y + other.h);
        Rect {
            x: x0,
            y: y0,
            w: x1 - x0,
            h: y1 - y0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidRegion {
    Whole,
    Rect(Rect),
}

impl InvalidRegion {
    fn merge(self, other: InvalidRegion) -> InvalidRegion {
        match (self, other) {
            (InvalidRegion::Whole, _) | (_, InvalidRegion::Whole) => InvalidRegion::Whole,
            (InvalidRegion::Rect(a), InvalidRegion::Rect(b)) => InvalidRegion::Rect(a.union(b)),
        }
    }
}

/// `Invalidate()`/`Update()` 상태 머신.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InvalidateState {
    pending: Option<InvalidRegion>,
    pub paints: Vec<InvalidRegion>,
}

impl InvalidateState {
    pub fn new() -> Self {
        Self::default()
    }

    /// `Control.Invalidate(rect)` 또는 `Invalidate()`.
    pub fn invalidate(&mut self, region: Option<Rect>) {
        let region = match region {
            Some(rect) => InvalidRegion::Rect(rect),
            None => InvalidRegion::Whole,
        };
        self.pending = Some(match self.pending {
            Some(pending) => pending.merge(region),
            None => region,
        });
    }

    /// `Control.Update()` — 대기 영역을 한 번의 페인트로 소비한다.
    pub fn update(&mut self) -> Option<InvalidRegion> {
        let region = self.pending.take()?;
        self.paints.push(region);
        Some(region)
    }

    /// `Refresh()` — 무효화 후 즉시 페인트.
    pub fn refresh(&mut self, region: Option<Rect>) -> Option<InvalidRegion> {
        self.invalidate(region);
        self.update()
    }

    pub fn is_dirty(&self) -> bool {
        self.pending.is_some()
    }

    pub fn pending(&self) -> Option<InvalidRegion> {
        self.pending
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn subscriptions_fire_in_order_and_remove_last_match() {
        let mut bus = EventBus::new();
        bus.subscribe(PluginEvent::SelectedNode1Changed, 1);
        bus.subscribe(PluginEvent::SelectedNode1Changed, 2);
        bus.subscribe(PluginEvent::SelectedNode1Changed, 1);
        assert_eq!(bus.emit(PluginEvent::SelectedNode1Changed), vec![1, 2, 1]);
        // `-=`는 마지막 1을 제거한다.
        assert!(bus.unsubscribe(PluginEvent::SelectedNode1Changed, 1));
        assert_eq!(bus.emit(PluginEvent::SelectedNode1Changed), vec![1, 2]);
        assert!(!bus.unsubscribe(PluginEvent::SelectedNode1Changed, 9));
    }

    #[test]
    fn events_are_independent() {
        let mut bus = EventBus::new();
        bus.subscribe(PluginEvent::WzOpened, 3);
        bus.subscribe(PluginEvent::WzClosing, 4);
        assert_eq!(bus.emit(PluginEvent::WzOpened), vec![3]);
        assert_eq!(bus.emit(PluginEvent::WzClosing), vec![4]);
        assert_eq!(bus.emit(PluginEvent::MainStyleChanged), Vec::<u64>::new());
    }

    #[test]
    fn invalidates_coalesce_into_one_paint() {
        let mut state = InvalidateState::new();
        state.invalidate(Some(Rect {
            x: 0,
            y: 0,
            w: 10,
            h: 10,
        }));
        state.invalidate(Some(Rect {
            x: 5,
            y: 5,
            w: 10,
            h: 10,
        }));
        assert!(state.is_dirty());
        assert_eq!(
            state.update(),
            Some(InvalidRegion::Rect(Rect {
                x: 0,
                y: 0,
                w: 15,
                h: 15
            }))
        );
        assert!(!state.is_dirty());
        assert_eq!(state.update(), None);
        assert_eq!(state.paints.len(), 1);
    }

    #[test]
    fn whole_invalidate_absorbs_partial() {
        let mut state = InvalidateState::new();
        state.invalidate(Some(Rect {
            x: 1,
            y: 2,
            w: 3,
            h: 4,
        }));
        state.invalidate(None);
        assert_eq!(state.update(), Some(InvalidRegion::Whole));
        state.invalidate(None);
        state.invalidate(Some(Rect {
            x: 1,
            y: 2,
            w: 3,
            h: 4,
        }));
        assert_eq!(state.update(), Some(InvalidRegion::Whole));
    }

    #[test]
    fn refresh_paints_immediately() {
        let mut state = InvalidateState::new();
        let painted = state.refresh(Some(Rect {
            x: 2,
            y: 3,
            w: 4,
            h: 5,
        }));
        assert_eq!(
            painted,
            Some(InvalidRegion::Rect(Rect {
                x: 2,
                y: 3,
                w: 4,
                h: 5
            }))
        );
        assert_eq!(state.paints.len(), 1);
        assert!(!state.is_dirty());
    }
}
