//! 경로 해석과 UOL 해석.
//!
//! 원본에 해석기가 **두 벌** 있다 (`Db2Host.GetNode` `Db2Host.cs:71-144`,
//! `DB2NodeExtension.GetNode` `:170-225`). 하나는 재귀형, 하나는 반복형이고
//! 둘 다 `ResolveUol()` 을 직접 쓰지 않는다. 이 모듈이 정식 해석기가 된다.
//!
//! # 원본의 결함 두 가지 (여기서 고친다)
//!
//! 1. **무한 루프** — `Wz_Node.ResolveUol` (`Wz_Node.cs:549-558`) 은
//!    `while (node?.Value is Wz_Uol)` 이고 방문 집합이 없다. 데이터에
//!    순환 UOL 이 하나만 있어도 프로그램이 멈춘다.
//! 2. **해석 결과 검증 없음** — 해석된 경로가 실제 이미지 바깥을 가리켜도
//!    그대로 반환한다.
//!
//! 둘 다 `resolve_uol` 의 방문 집합으로 해결한다. 실패는 **오류**로 낸다 —
//! 조용히 `None` 을返하면 "링크가 없다" 와 "링크가 순환한다" 를 구분할 수 없다.

use std::collections::HashSet;

use crate::arena::Arena;
use crate::node::{NodeId, NodeValue};

/// 해석 실패 사유.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolveError {
    /// 경로가 어떤 노드도 가리키지 않는다 (원본도 `null` 반환).
    NotFound,
    /// 링크가 자기 자신을 가리키거나 순환한다. 원본은 **무한 루프**한다.
    Cycle,
    /// 안전 상한 초과. 원본엔 없던 방어다.
    TooDeep,
}

impl std::fmt::Display for ResolveError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound => write!(f, "경로를 찾지 못함"),
            Self::Cycle => write!(f, "UOL 순환 (원본은 무한 루프)"),
            Self::TooDeep => write!(f, "해석 깊이 상한 초과"),
        }
    }
}

impl std::error::Error for ResolveError {}

/// 안전 상한. 원본엔 없지만 순환 버그를 방어한다.
const MAX_DEPTH: usize = 256;

/// `/` 구분 경로를 따라간다.
///
/// # 함정: `extractImage` 는 이미 흡수돼 있다
///
/// 원본 `FindNodeByPath(extractImage: true)` 는 `Wz_Image` 값을 만나면
/// `img.Node` 로 내려간다. arena 는 이미지를 **파싱된 자식**으로 갖고
/// (`ingest` 참고) 이 단계가 이미 끝난 상태라 별도 처리가 필요 없다.
pub fn find_path(a: &Arena, from: NodeId, path: &str) -> Option<NodeId> {
    let mut cur = Some(from);
    for seg in path.split('/') {
        // 빈 세그먼트은 건너뛴다. 원본 `string.Split('/')` 도 빈 문자열을
        // 만들지만 `nodes[""]` 는 대개 null 이라 결과가 같다. 명시적으로
        // 처리해 "a//b" 가 "a/b" 처럼 동작하게 한다.
        if seg.is_empty() {
            continue;
        }
        cur = cur.and_then(|p| a.find_child(p, seg));
        cur?;
    }
    cur
}

/// UOL 하나를 해석한다 (`Wz_Uol.HandleUol` 대응).
///
/// `node` 는 UOL 값을 가진 노드여야 한다.
///
/// # 함정: 시작점은 UOL 노드의 **부모**다
///
/// 원본이 `currentNode = currentNode.ParentNode` 로 시작하므로
/// `"../a"` 같은 경로가 자기 자신을 가리킬 수 있다. 그게 의도된 동작이다.
pub fn handle_uol(a: &Arena, node: NodeId) -> Option<NodeId> {
    let NodeValue::Uol(path) = a.value(node)? else {
        return None;
    };
    if path.is_empty() {
        return None;
    }
    let mut cur = a.parent(node)?;

    // 이미지를 벗어났는지. 원본 `outImg`.
    let mut out_img = false;

    for seg in path.split('/') {
        match seg {
            // 원본: `..` 가 이미지 루트에 닿으면 파일(디렉터리) 수준으로
            // 빠져나가고 `outImg` 를 켠다. 이미지 **내부**에서는 그냥 부모로.
            //
            // 함정: arena 에서 이미지 루트도 `.img` 엔트리를 부모로 갖는다.
            // `is_image_root` 없이는 디렉터리 트리 안으로 잘못 올라가서
            // `..` 한 번이 두 단계씩 이동한다.
            ".." => {
                if a.is_image_root(cur) {
                    // 이미지 루트 → `.img` 엔트리 → 디렉터리. 두 단계 올라간다.
                    cur = a.parent(cur).and_then(|img_entry| a.parent(img_entry))?;
                    out_img = true;
                } else {
                    cur = a.parent(cur)?;
                }
            }
            name if !name.is_empty() => {
                match a.find_child(cur, name) {
                    Some(n) => cur = n,
                    None if out_img => {
                        // 원본 `HandleUol:61-68` 의 `.img` 재시도.
                        //
                        // 함정: 이 재시도만 `extractImage: true` 다.
                        // 그래서 `../Other/x`(확장자 없는 세그먼트)는
                        // **이미지 안으로 내려가서** 성공하지만,
                        // `../Other.img/x` 는 앞선 조회에서 `Other.img`
                        // 가 그대로 걸려 내려가지 못하고 실패한다.
                        // 이 비대칭이 원본의 실제 동작이다.
                        let with_ext = format!("{name}.img");
                        cur = a.descend_image(a.find_child(cur, &with_ext)?);
                        out_img = false;
                    }
                    None => return None,
                }
            }
            _ => {}
        }
    }
    Some(cur)
}

/// UOL 체인을 끝까지 푼다 (`Wz_Node.ResolveUol` 대응).
///
/// # 원본과의 차이
///
/// 원본은 `while` 루프에 방문 집합이 없어 순환하면 멈추지 않는다.
/// 여기서는 방문한 `NodeId` 를 기억해 순환을 [`ResolveError::Cycle`] 로
/// 보고한다. 링크가 해석되지 않는 경우도 조용히 넘어가지 않고 오류로 낸다.
pub fn resolve_uol(a: &Arena, node: NodeId) -> Result<NodeId, ResolveError> {
    let mut cur = node;
    let mut seen: HashSet<NodeId> = HashSet::new();

    for _ in 0..MAX_DEPTH {
        // 값이 UOL 이 아니면 그 노드가 최종 답이다.
        if !matches!(a.value(cur), Some(NodeValue::Uol(_))) {
            return Ok(cur);
        }
        // 순환 검사: 같은 UOL 노드를 두 번 만나면 그 지점부터 끝없이 반복된다.
        if !seen.insert(cur) {
            return Err(ResolveError::Cycle);
        }
        cur = handle_uol(a, cur).ok_or(ResolveError::NotFound)?;
    }
    Err(ResolveError::TooDeep)
}

/// 해석하되 실패하면 `None`. 호출부가 에러를 신경 쓰지 않을 때 쓴다.
pub fn resolve_uol_lossy(a: &Arena, node: NodeId) -> Option<NodeId> {
    resolve_uol(a, node).ok()
}
