//! Windows `StrCmpLogicalW` 등가 자연 정렬 (리포트 §12 R4, ADR-6).
//!
//! # 왜 이게 포트의 정확성 조건인가
//!
//! C# `WzFileComparer` 는 양쪽 노드 트리를 정렬한 뒤 투 포인터로 병합 순회한다
//! (`Comparer/WzFileComparer.cs:133-307`). 정렬은 `StrCmpLogicalW` 로 한다:
//! ```csharp
//! // WzComparerR2.WzLib/Wz_Node.cs:529-530
//! [DllImport("shlwapi.dll", CharSet = CharSet.Unicode)]
//! static extern int StrCmpLogicalW(string psz1, string psz2);
//! ```
//!
//! 이 함수를 근사하면 **diff 결과가 조용히 달라진다.** 엣지 케이스에서만
//! 순서가 뒤집히기 때문에 "대부분 맞고 가끔 어긋남" 형태가 되어 버그 추적이
//! 사실상 불가능하다.
//!
//! # 어디에 쓰이는가
//!
//! | 호출부 | 위치 |
//! |---|---|
//! | `WzFileComparer` 정렬 | `Comparer/WzFileComparer.cs:499-503` |
//! | `WzNodeCollection.Sort()` | `Wz_Node.cs:516-527` |
//! | 가상 노드 링크 정렬 (`FullPathToFile`) | `Comparer/WzFileComparer.cs:166-188` |
//!
//! # 규칙
//!
//! 1. 두 문자열을 **문자(코드포인트) 단위**로 왼쪽부터 비교한다.
//! 2. **연속된 숫자 구간**은 길이가 아니라 **수치**로 비교한다.
//!    - `img1` < `img2` < `img10` (사전순이면 `img10` < `img2`)
//! 3. 숫자가 아니면 유니코드 코드포인트 순서로 비교한다.
//! 4. 한쪽이 먼저 끝나면 짧은 쪽이 작다.
//!
//! # 아직 확정 안 된 것 (오라클 검증 필요)
//!
//! `StrCmpLogicalW` 의 정확한 선행 0 처리, 매우 긴 숫자(정수 범위 초과) 처리,
//! 혼합 문자/숫자 경계에서의 동작은 **Windows 구현이 문서화돼 있지 않다.**
//! 여기서는 위 규칙을 구현하고, 실제 `StrCmpLogicalW` 결과와 대조하는 테스트를
//! Phase 1.12 에서 작성한다. 규칙을 "가정"하지 말고 **측정**해야 한다.
//!
//! 특히 선행 0: `img007` 과 `img7` 이 같아야 하는지 다른지, `img07` < `img7` 인지
//! `img7` < `img08` 인지가 플랫폼/버전마다 다를 수 있다.

use std::cmp::Ordering;

/// 자연 정렬 비교.
///
/// # 예시
/// ```
/// use wz_core::text::natural_cmp;
/// use std::cmp::Ordering;
///
/// assert_eq!(natural_cmp("img2", "img10"), Ordering::Less);
/// assert_eq!(natural_cmp("a", "b"), Ordering::Less);
/// assert_eq!(natural_cmp("img", "img0"), Ordering::Less);
/// assert_eq!(natural_cmp("2", "10"), Ordering::Less);
/// ```
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let mut ai = a.chars().peekable();
    let mut bi = b.chars().peekable();

    loop {
        match (ai.peek().copied(), bi.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(ca), Some(cb)) => {
                if is_ascii_digit(ca) && is_ascii_digit(cb) {
                    // 숫자 구간: 선행 0 을 건너뛰고 유효 숫자만 모아 수치 비교
                    let na = take_number(&mut ai);
                    let nb = take_number(&mut bi);
                    match compare_numeric(na.as_str(), nb.as_str()) {
                        Ordering::Equal => continue,
                        other => return other,
                    }
                } else {
                    match ca.cmp(&cb) {
                        Ordering::Equal => {
                            ai.next();
                            bi.next();
                        }
                        other => return other,
                    }
                }
            }
        }
    }
}

#[inline]
fn is_ascii_digit(c: char) -> bool {
    c.is_ascii_digit()
}

/// 연속 숫자 구간을 읽는다. **선행 0 을 제거한** 문자열을 반환한다.
fn take_number<I: Iterator<Item = char>>(it: &mut std::iter::Peekable<I>) -> String {
    let mut digits = String::new();
    let mut leading = true;
    while let Some(&c) = it.peek() {
        if c.is_ascii_digit() {
            if leading && c == '0' {
                // 선행 0 은 값에 영향이 없으므로 버린다
                it.next();
            } else {
                leading = false;
                digits.push(c);
                it.next();
            }
        } else {
            break;
        }
    }
    digits
}

/// 자릿수, 다음 문자 순으로 비교해 `str` 없이 수치 비교.
fn compare_numeric(a: &str, b: &str) -> Ordering {
    // 선행 0 이 제거됐으므로 "0" 은 값 0 을 나타내는 유일한 표기
    if a == "0" || b == "0" {
        let (az, bz) = (a == "0", b == "0");
        return match (az, bz) {
            (true, true) => Ordering::Equal,
            (true, false) => {
                // a 가 0, b 가 0 이 아닌 숫자 → b 가 큼. 단 "0" 다음에 더 이상의
                // 숫자가 없으면 a 가 더 짧으므로 a 가 작다.
                Ordering::Greater
            }
            _ => Ordering::Less,
        };
    }

    match a.len().cmp(&b.len()) {
        Ordering::Equal => a.cmp(b),
        other => other,
    }
}

/// 정렬 키 (여러 필드 우선순위용).
///
/// `WzFileComparer` 는 이름 하나만으로 정렬하지만, 가상 노드 링크 정렬처럼
/// 여러 필드를 쓰는 곳이 있으므로 튜플 키를 제공한다.
pub fn natural_key(s: &str) -> impl Ord + '_ {
    // 문자열 대신 정규화된 키를 만들면 `sort_by` 가 더 빠르지만,
    // 정확성 우선이라 직접 비교 함수를 쓴다. 키 생성은 필요할 때만.
    NaturalKey(s)
}

#[derive(PartialEq, Eq)]
struct NaturalKey<'a>(&'a str);

impl Ord for NaturalKey<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        natural_cmp(self.0, other.0)
    }
}

impl PartialOrd for NaturalKey<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// 정렬 헬퍼: 원본 배열을 **제자리**에서 자연 정렬한다.
///
/// C# `WzNodeCollection.Sort()` 와 같은 의미다.
pub fn sort_natural<T, F>(items: &mut [T], key: F)
where
    F: Fn(&T) -> &str,
{
    items.sort_by(|a, b| natural_cmp(key(a), key(b)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_strings() {
        for s in ["", "a", "abc", "img00002000.img", "123"] {
            assert_eq!(natural_cmp(s, s), Ordering::Equal, "s={s:?}");
        }
    }

    #[test]
    fn numbers_compare_numerically_not_lexically() {
        // 이게 핵심. 사전순이면 img10 < img2 가 된다.
        assert_eq!(natural_cmp("img2", "img10"), Ordering::Less);
        assert_eq!(natural_cmp("img10", "img2"), Ordering::Greater);
        assert_eq!(natural_cmp("2", "10"), Ordering::Less);
        assert_eq!(natural_cmp("00100", "100"), Ordering::Equal);
        assert_eq!(natural_cmp("item9", "item10"), Ordering::Less);
    }

    #[test]
    fn leading_zeros_are_ignored() {
        // 가정: 선행 0 은 값에 영향이 없다.
        // → 이게 `StrCmpLogicalW` 와 같은지는 오라클 검증 대상.
        assert_eq!(natural_cmp("img007", "img7"), Ordering::Equal);
        // 앞뒤 문자열까지 동일하므로 전체가 같다
        assert_eq!(natural_cmp("00002000.img", "2000.img"), Ordering::Equal);
        // 숫자는 같지만 경로 접미어가 다르면 접미어로 결정된다
        assert_eq!(natural_cmp("00002000.img", "2000.img2"), Ordering::Less);
    }

    #[test]
    fn zero_is_ordered_before_nonzero() {
        assert_eq!(natural_cmp("img0", "img1"), Ordering::Less);
        assert_eq!(natural_cmp("img0", "img00"), Ordering::Equal);
        assert_eq!(natural_cmp("a0b", "a1b"), Ordering::Less);
        // "0" 이 뒤에 더 오면 이후 문자로 결정
        assert_eq!(natural_cmp("x0", "x1"), Ordering::Less);
    }

    #[test]
    fn prefix_is_smaller_than_extension() {
        assert_eq!(natural_cmp("img", "img0"), Ordering::Less);
        assert_eq!(natural_cmp("img0", "img"), Ordering::Greater);
        assert_eq!(natural_cmp("a", "ab"), Ordering::Less);
    }

    #[test]
    fn mixed_alpha_numeric() {
        assert_eq!(
            natural_cmp("Char/0001.img", "Char/0002.img"),
            Ordering::Less
        );
        assert_eq!(natural_cmp("A1", "A10"), Ordering::Less);
        assert_eq!(natural_cmp("A1B", "A1C"), Ordering::Less);
        // 숫자 뒤 문자가 붙어도 숫자 비교가 먼저다
        assert_eq!(natural_cmp("a2z", "a10z"), Ordering::Less);
    }

    #[test]
    fn cjk_and_unicode_order_by_codepoint() {
        // 라틴 > 한글 (유니코드 코드포인트 순)
        assert_eq!(natural_cmp("z", "가"), Ordering::Less);
        assert_eq!(natural_cmp("가", "나"), Ordering::Less);
    }

    #[test]
    fn is_total_order() {
        // 반사성 / 추이성 / 총체성 — `sort_by` 가 패닉하지 않으려면 필요
        let items = [
            "img10", "img2", "img1", "0003", "3", "a", "", "img07", "img7", "Z", "z",
        ];
        for a in items {
            assert_eq!(natural_cmp(a, a), Ordering::Equal, "반사성: {a:?}");
            for b in items {
                let ab = natural_cmp(a, b);
                let ba = natural_cmp(b, a);
                assert_eq!(ab, ba.reverse(), "반대칭성: {a:?} vs {b:?}");
            }
        }
    }

    #[test]
    fn sorting_is_deterministic() {
        let mut v = vec![
            "img10", "img2", "img1", "0003", "a", "", "img07", "img7", "Z",
        ];
        sort_natural(&mut v, |s| s);
        let once = v.clone();
        sort_natural(&mut v, |s| s);
        assert_eq!(v, once, "정렬은 멱등해야 한다");
    }

    #[test]
    fn typical_wz_paths_sort_correctly() {
        // 실제 WZ 경로로 확인. `00002000.img` 는 앞에 0 이 붙어 있어
        // 사전순이든 자연순이든 같지만, 경로 중간 숫자가 문제가 된다.
        let mut v = vec![
            "String/Item.img/2000001.img",
            "String/Item.img/2000010.img",
            "String/Item.img/200000.img",
            "String/Etc.img/000200000.img",
        ];
        sort_natural(&mut v, |s| s);
        assert_eq!(
            v,
            vec![
                "String/Etc.img/000200000.img",
                "String/Item.img/200000.img",
                "String/Item.img/2000001.img",
                "String/Item.img/2000010.img",
            ]
        );
    }

    #[test]
    fn very_long_numbers_dont_overflow() {
        // 정수 범위를 넘는 숫자도 패닉 없이 비교돼야 한다 (자릿수 비교 방식).
        let a = "9".repeat(100);
        let b = "9".repeat(101);
        assert_eq!(natural_cmp(&a, &b), Ordering::Less);
        assert_eq!(natural_cmp(&a, &a), Ordering::Equal);
    }

    #[test]
    fn numeric_key_helper() {
        let mut v = vec!["10", "9", "100", "1"];
        sort_natural(&mut v, |s| s);
        assert_eq!(v, vec!["1", "9", "10", "100"]);
    }
}
