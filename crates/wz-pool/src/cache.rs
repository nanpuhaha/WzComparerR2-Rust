//! 바이트 예산 기반 LRU 캐시 (리포트 ADR-1).
//!
//! # 왜 개수 LRU가 아닌가
//!
//! C# 원본 `WzFileComparer.cs:629-696`은 이미지를 **32개**까지만 캐시했다.
//! WZ 이미지는 4×4(64 B)부터 2048×2048 ARGB8888(16 MB)까지 크기가 수십만 배
//! 다르므로, 개수 기준 캐시는 메모리를 통제하지 못한다. 수십 MB만 써도 OOM이
//! 날 수 있고, 반대로 32개는 병목이 될 수도 있다.
//!
//! 여기서는 **바이트 예산**으로 관리한다. 항목 비용을 합산하고 예산을 넘으면
//! 오래 접근되지 않은 항목부터 해제한다.
//!
//! # pin 카운터
//!
//! 순회 중인 항목은 강제 제거되면 안 된다 (C# `DisposeQueue`가 "사용 중이면
//! 건너뛰기"로 처리하던 것). [`ByteBudgetLru::pin`]이 [`PinGuard`]를 반환하고,
//! Drop 시 자동으로 unpin된다.

use std::hash::Hash;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;

use dashmap::DashMap;
use lru::LruCache;

/// 기본 이미지 캐시 예산. 사용자 설정 가능.
pub const DEFAULT_BUDGET_BYTES: usize = 512 * 1024 * 1024;

struct Entry<V> {
    val: Arc<V>,
    weight: usize,
}

/// 바이트 예산 LRU.
///
/// `V`는 clonable이어야 하고, 삽입 시 [`weight`] 함수로 비용이 측정된다.
pub struct ByteBudgetLru<K, V>
where
    K: Hash + Eq + Clone,
{
    map: DashMap<K, Entry<V>>,
    order: std::sync::Mutex<LruCache<K, ()>>,
    pins: DashMap<K, AtomicUsize>,
    budget: AtomicU64,
    current: AtomicU64,
    hits: AtomicU64,
    misses: AtomicU64,
    evictions: AtomicU64,
}

impl<K, V> ByteBudgetLru<K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    pub fn with_budget(budget: usize) -> Self {
        Self {
            map: DashMap::new(),
            // 개수 상한을 두지 않는다. 예산은 바이트 단위이므로 항목 수는
            // 파서가 결정한다 (수십만 개의 작은 노드가 정상적인 사용 패턴).
            order: std::sync::Mutex::new(LruCache::unbounded()),
            pins: DashMap::new(),
            budget: AtomicU64::new(budget as u64),
            current: AtomicU64::new(0),
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
            evictions: AtomicU64::new(0),
        }
    }

    pub fn budget(&self) -> u64 {
        self.budget.load(Ordering::Relaxed)
    }

    pub fn set_budget(&self, bytes: u64) {
        self.budget.store(bytes, Ordering::Relaxed);
        self.evict_to_budget();
    }

    pub fn current_bytes(&self) -> u64 {
        self.current.load(Ordering::Relaxed)
    }

    /// 캐시된 값을 얻고 접근 순서를 갱신한다. 히트/미스를 기록한다.
    pub fn get(&self, k: &K) -> Option<Arc<V>> {
        let found = self.map.get(k).map(|e| Arc::clone(&e.val));
        match found {
            Some(v) => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                if let Ok(mut o) = self.order.lock() {
                    // 캐시가 이미 있으면 LRU 순서를 갱신한다. (없으면 넣지 않는다 —
                    // `insert` 가 주문된 키만 관리해야 한다.)
                    o.promote(k);
                }
                Some(v)
            }
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                None
            }
        }
    }

    /// `compute`로 값을 만들어 삽입한다. 이미 있으면 캐시된 값을 반환.
    ///
    /// `compute`는 캐시 락 밖에서 호출되어야 한다 (파싱 중 락 보유 금지).
    pub fn get_or_insert_with<F>(&self, k: &K, weight: fn(&V) -> usize, compute: F) -> Arc<V>
    where
        F: FnOnce() -> V,
    {
        if let Some(v) = self.get(k) {
            return v;
        }
        let v = compute();
        self.insert(k.clone(), v, weight);
        self.map
            .get(k)
            .map(|e| Arc::clone(&e.val))
            .expect("insert 직후 조회")
    }

    pub fn insert(&self, k: K, v: V, weight: fn(&V) -> usize) {
        let w = weight(&v);
        let entry = Entry {
            val: Arc::new(v),
            weight: w,
        };
        if let Some(old) = self.map.insert(k.clone(), entry) {
            self.current.fetch_sub(old.weight as u64, Ordering::Relaxed);
        }
        self.current.fetch_add(w as u64, Ordering::Relaxed);
        if let Ok(mut o) = self.order.lock() {
            o.put(k.clone(), ());
        }
        self.evict_to_budget();
    }

    /// 명시적 제거. 제거된 바이트 수를 반환.
    pub fn remove(&self, k: &K) -> u64 {
        let removed = self.map.remove(k).map(|(_, e)| e.weight as u64);
        if let Some(w) = removed {
            self.current.fetch_sub(w, Ordering::Relaxed);
        }
        if let Ok(mut o) = self.order.lock() {
            o.pop(k);
        }
        self.pins.remove(k);
        removed.unwrap_or(0)
    }

    pub fn clear(&self) {
        self.map.clear();
        self.pins.clear();
        if let Ok(mut o) = self.order.lock() {
            o.clear();
        }
        self.current.store(0, Ordering::Relaxed);
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// pin 카운터를 올린다. Drop 시 자동 해제.
    pub fn pin(&self, k: &K) -> PinGuard<'_, K, V> {
        self.pins
            .entry(k.clone())
            .or_insert_with(|| AtomicUsize::new(0))
            .fetch_add(1, Ordering::Relaxed);
        PinGuard {
            lru: self,
            key: k.clone(),
            _v: std::marker::PhantomData,
        }
    }

    fn is_pinned(&self, k: &K) -> bool {
        self.pins
            .get(k)
            .map(|v| v.load(Ordering::Relaxed) > 0)
            .unwrap_or(false)
    }

    fn unpin(&self, k: &K) {
        let should_remove = {
            let Some(entry) = self.pins.get(k) else {
                return;
            };
            let prev = entry.fetch_sub(1, Ordering::AcqRel);
            prev <= 1
        };
        if should_remove {
            self.pins.remove(k);
        }
    }

    /// 예산을 넘으면 LRU 순으로 (pin 되지 않은) 항목을 제거한다.
    fn evict_to_budget(&self) {
        // 모든 항목이 pin 된 경우 무한 루프에 빠지지 않도록 상한을 둔다.
        let max_steps = self.map.len() + 1;
        let mut steps = 0usize;

        while self.current.load(Ordering::Relaxed) > self.budget.load(Ordering::Relaxed) {
            steps += 1;
            if steps > max_steps {
                break;
            }

            // LRU 순서에서 pin 되지 않은 첫 항목을 고른다.
            //
            // 주의: `lru` 0.18 의 `iter()` 는 **MRU → LRU** 순서다.
            // `.rev()` 로 뒤집어야 진짜 LRU 가 나온다. (방향이 바뀌면 "가장 오래된
            // 항목" 이 아니라 "가장 최근 항목" 을 evict 하게 되어 pin/캐시 정책이
            // 조용히 뒤집힌다.)
            let victim = {
                let Ok(mut o) = self.order.lock() else {
                    return;
                };
                let mut candidate = None;
                for (k, _) in o.iter().rev() {
                    if !self.is_pinned(k) {
                        candidate = Some(k.clone());
                        break;
                    }
                }
                if let Some(k) = &candidate {
                    o.pop(k);
                }
                candidate
            };

            let Some(k) = victim else {
                // 전부 pin 됨 — 예산 초과를 허용한다 (순회 중 폭주 방지)
                break;
            };

            if let Some((_, e)) = self.map.remove(&k) {
                self.current.fetch_sub(e.weight as u64, Ordering::Relaxed);
                self.evictions.fetch_add(1, Ordering::Relaxed);
            }
            self.pins.remove(&k);
        }
    }

    pub fn stats(&self) -> CacheStats {
        let hits = self.hits.load(Ordering::Relaxed);
        let misses = self.misses.load(Ordering::Relaxed);
        let total = hits + misses;
        CacheStats {
            hits,
            misses,
            evictions: self.evictions.load(Ordering::Relaxed),
            entries: self.map.len(),
            bytes: self.current.load(Ordering::Relaxed),
            budget: self.budget.load(Ordering::Relaxed),
            hit_rate: if total == 0 {
                0.0
            } else {
                hits as f64 / total as f64
            },
        }
    }

    pub fn reset_stats(&self) {
        self.hits.store(0, Ordering::Relaxed);
        self.misses.store(0, Ordering::Relaxed);
        self.evictions.store(0, Ordering::Relaxed);
    }
}

/// Drop 시 자동으로 unpin하는 가드.
pub struct PinGuard<'a, K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    lru: &'a ByteBudgetLru<K, V>,
    key: K,
    _v: std::marker::PhantomData<&'a V>,
}

impl<K, V> Drop for PinGuard<'_, K, V>
where
    K: Hash + Eq + Clone,
    V: Clone,
{
    fn drop(&mut self) {
        self.lru.unpin(&self.key);
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub entries: usize,
    pub bytes: u64,
    pub budget: u64,
    pub hit_rate: f64,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone)]
    struct Blob(Vec<u8>);

    fn weight(b: &Blob) -> usize {
        b.0.len()
    }

    fn lru_with(budget: usize) -> ByteBudgetLru<u64, Blob> {
        ByteBudgetLru::with_budget(budget)
    }

    #[test]
    fn stores_and_returns() {
        let l = lru_with(1000);
        l.insert(1, Blob(vec![0; 100]), weight);
        assert_eq!(l.len(), 1);
        assert_eq!(l.current_bytes(), 100);
        assert_eq!(l.get(&1).unwrap().0.len(), 100);
    }

    #[test]
    fn miss_is_none_and_counted() {
        let l = lru_with(1000);
        assert!(l.get(&42).is_none());
        let s = l.stats();
        assert_eq!(s.misses, 1);
        assert_eq!(s.hits, 0);
    }

    #[test]
    fn evicts_lru_until_under_budget() {
        let l = lru_with(250); // 각 100 B
        l.insert(1, Blob(vec![0; 100]), weight);
        l.insert(2, Blob(vec![0; 100]), weight);
        assert_eq!(l.len(), 2);

        l.insert(3, Blob(vec![0; 100]), weight);
        assert!(l.len() <= 2, "예산 초과 금지: {}", l.len());
        assert!(l.current_bytes() <= 250);
        assert!(l.get(&1).is_none(), "가장 오래 접근되지 않은 항목이 evict");
        assert!(l.get(&3).is_some());
    }

    #[test]
    fn access_refreshes_lru_order() {
        let l = lru_with(250);
        l.insert(1, Blob(vec![0; 100]), weight);
        l.insert(2, Blob(vec![0; 100]), weight);
        assert!(l.get(&1).is_some(), "1에 재접근");
        l.insert(3, Blob(vec![0; 100]), weight);
        assert!(l.get(&1).is_some(), "접근한 항목 생존");
        assert!(l.get(&2).is_none(), "미접근 항목이 evict");
    }

    #[test]
    fn pinned_entry_survives_eviction() {
        let l = lru_with(250);
        l.insert(1, Blob(vec![0; 100]), weight);
        {
            let _guard = l.pin(&1);
            l.insert(2, Blob(vec![0; 100]), weight);
            l.insert(3, Blob(vec![0; 100]), weight);
            l.insert(4, Blob(vec![0; 100]), weight);
            assert!(l.get(&1).is_some(), "pin 항목은 보호");
        }
        // 가드 해제 후에는 evict 가능
        l.insert(5, Blob(vec![0; 100]), weight);
        l.insert(6, Blob(vec![0; 100]), weight);
        assert!(l.get(&1).is_none(), "unpin 후 evict 가능");
    }

    #[test]
    fn oversized_single_entry_terminates() {
        let l = lru_with(50);
        l.insert(1, Blob(vec![0; 1000]), weight);
        assert_eq!(l.len(), 0, "예산 초과 단독 항목은 즉시 폐기");
        assert_eq!(l.current_bytes(), 0);
    }

    #[test]
    fn all_pinned_does_not_hang() {
        let l = lru_with(10);
        l.insert(1, Blob(vec![0; 100]), weight);
        l.insert(2, Blob(vec![0; 100]), weight);
        let _a = l.pin(&1);
        let _b = l.pin(&2);
        l.insert(3, Blob(vec![0; 100]), weight);
        // 무한 루프 없이 끝나야 한다
        assert!(l.len() <= 3);
    }

    #[test]
    fn get_or_insert_with_computes_once() {
        use std::sync::atomic::AtomicU32;
        static CALLS: AtomicU32 = AtomicU32::new(0);
        let l: ByteBudgetLru<u64, Blob> = ByteBudgetLru::with_budget(10_000);
        for _ in 0..5 {
            let v = l.get_or_insert_with(&7, weight, || {
                CALLS.fetch_add(1, Ordering::Relaxed);
                Blob(vec![1; 50])
            });
            assert_eq!(v.0.len(), 50);
        }
        assert_eq!(CALLS.load(Ordering::Relaxed), 1, "1회만 계산");
    }

    #[test]
    fn reinsert_same_key_does_not_double_count() {
        let l = lru_with(10_000);
        l.insert(1, Blob(vec![0; 100]), weight);
        l.insert(1, Blob(vec![0; 100]), weight);
        assert_eq!(l.len(), 1);
        assert_eq!(l.current_bytes(), 100, "중복 키는 이중 계상되지 않음");
    }

    #[test]
    fn concurrent_access_is_safe() {
        let l = std::sync::Arc::new(lru_with(1_000_000));
        std::thread::scope(|s| {
            for t in 0..4u64 {
                let l = std::sync::Arc::clone(&l);
                s.spawn(move || {
                    for i in 0..500u64 {
                        let k = t * 1000 + i;
                        l.insert(k, Blob(vec![0; 64]), weight);
                        let _guard = l.pin(&k);
                        let _ = l.get(&k);
                    }
                });
            }
        });
        assert!(l.current_bytes() <= 1_000_000);
    }

    #[test]
    fn stats_track_hit_rate() {
        let l = lru_with(10_000);
        l.insert(1, Blob(vec![0; 10]), weight);
        l.insert(2, Blob(vec![0; 10]), weight);
        let _ = l.get(&1);
        let _ = l.get(&2);
        let _ = l.get(&3);
        let s = l.stats();
        assert_eq!(s.hits, 2);
        assert_eq!(s.misses, 1);
        assert!((s.hit_rate - 2.0 / 3.0).abs() < 1e-9);
    }
}
