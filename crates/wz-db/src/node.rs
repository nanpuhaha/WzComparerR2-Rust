//! 노드 표현 — `NodeId` 핸들과 값 타입.
//!
//! # 왜 `wz_core::image::Node` 를 그대로 쓰지 않는가
//!
//! `wz-core` 트리는 재귀 `Vec<Node>` 라 부모 포인터가 없고, UOL 해석에 필요한
//! "여기서 부모로 올라가기" 가 불가능하다. `NodeId` 로 평탄화하면
//!
//! - 부모 이동이 O(1)
//! - 방문 집합(`HashSet<NodeId>`) 으로 **사이클 감지**가 가능
//!   (원본 `Wz_Node.cs:549-558` 의 `ResolveUol` 는 감지가 없어 무한 루프한다)
//!
//! # `Property` 를 값이 아니라 자식으로 펴는 이유
//!
//! `wz_core::image::ImageValue::Property(Arc<Node>)` 은 "중첩 Property 객체"를
//! 값으로 가진다. 원본 `Wz_Image.ExtractImg` 은 그런 객체를 **부모 노드의
//! 자식으로 펴서** 넣는다 (`ExtractValue` case 0x08 → `ExtractImg(reader, parent)`).
//! 경로 해석 관점에서는 둘이 동일하므로 arena 는 `Property` 변형을 두지 않고
//! 자식으로 펴는 쪽을 **정규형**으로 삼는다. 불변식이 하나 줄어든다.

use std::sync::Arc;

use wz_core::image::{PngRef, SoundRef, Vec2, VideoRef};

/// arena 안의 노드 핸들.
///
/// 인덱스만 쓰면arena 재사용 시 이전 핸들이 새 노드를 가리켜 버린다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeId {
    pub(crate) index: u32,
    /// 세대 카운터. 슬롯을 재사용할 때마다 증가한다.
    pub(crate) generation: u32,
}

impl NodeId {
    pub(crate) fn new(index: u32, generation: u32) -> Self {
        Self { index, generation }
    }

    pub fn index(self) -> u32 {
        self.index
    }

    pub fn generation(self) -> u32 {
        self.generation
    }
}

/// 노드 값.
///
/// `wz_core::image::ImageValue` 와 1:1 대응하되 `Property` 변형이 없다
/// (중첩 Property 는 자식으로 펴므로 — 모듈 주석 참고).
#[derive(Debug, Clone, PartialEq)]
pub enum NodeValue {
    Null,
    Short(i16),
    Int(i32),
    Long(i64),
    Uint(u32),
    Float(f32),
    Double(f64),
    Bool(bool),
    Str(Arc<str>),
    Vector(Vec2),
    /// **미해석** UOL 경로. `resolve::resolve_uol` 이 풀다.
    Uol(Arc<str>),
    Canvas(PngRef),
    RawData(RawDataRef),
    Video(VideoRef),
    Sound(Arc<SoundRef>),
    Convex(Arc<[Vec2]>),
}

/// `RawData`/`Video` 참조. 픽셀/사운드 데이터는 아직 로드하지 않는다.
#[derive(Debug, Clone, PartialEq)]
pub struct RawDataRef {
    pub offset: u64,
    pub len: u32,
}

/// arena 에 들어 있는 노드 하나.
#[derive(Debug, Clone)]
pub struct NodeEntry {
    pub name: Arc<str>,
    pub value: Option<NodeValue>,
    pub parent: Option<NodeId>,
    /// 순서 보존. **중복 이름이 존재할 수 있다** (리포트 §3.2 S-9).
    pub children: Vec<NodeId>,
    /// 이 슬롯이 몇 번 재사용되었는지. `NodeId::generation` 과 대조한다.
    pub generation: u32,
    /// **이미지 루트**인가 (파일 트리의 `.img` 엔트리 아래에 붙은 최상위 노드).
    ///
    /// # 왜 필요한가
    ///
    /// 원본에서 `Wz_Image.Node` 는 `ParentNode` 가 `null` 인 **별도 트리**다.
    /// 그래서 `..` 가 루트에 닿으면 "이미지 안이 끝났다" 는 신호로 쓰인다
    /// (`Wz_Uol.HandleUol:39-51` — 이미지의 소유 파일로 빠져나간다).
    ///
    /// arena 는 이미지를 디렉터리 엔트리의 자식으로 붙이므로 이 구분이
    /// 사라진다. `..` 가 이미지 경계를 정확히 넘으려면 이 플래그가 필수다.
    pub is_image_root: bool,
}
