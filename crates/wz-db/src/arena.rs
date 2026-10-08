//! 노드 arena — bump 할당 + 세대 카운터 (ADR-2).
//!
//! # 왜 arena 인가
//!
//! 이미지 하나가 수만~수십만 노드가 될 수 있다. 노드마다 `Box` 를 쓰면
//! 할당 비용과 포인터 추적이 치고, `Vec<Node>` 재귀 구조는 부모 이동을
//! 못 한다. 평탄 `Vec` + 인덱스 핸들은 두 문제를 한 번에 없앤다.
//!
//! # 세대 카운터가 필요한 이유
//!
//! 슬롯을 재사용하면 이전 `NodeId` 가 **새 노드** 를 가리키게 된다.
//! UOL 해석은 이미 해제된 링크를 따라갈 수 있어서, 세대가 다르면 즉시
//! 거부해야 한다. 세대 검사가 없으면 엉뚱한 노드를 조용히 반환한다.

use std::sync::Arc;

use wz_core::image::{ImageValue, Node as WzNode};

use crate::node::{NodeEntry, NodeId, NodeValue, RawDataRef};

/// 평탄 노드 저장소.
#[derive(Debug, Default)]
pub struct Arena {
    entries: Vec<NodeEntry>,
    /// 재사용 가능한 빈 슬롯들.
    free: Vec<u32>,
}

impl Arena {
    pub fn new() -> Self {
        Self::default()
    }

    /// 비어 있는 루트를 만든다. `NodeId(0, 0)` 이 항상 이 루트다.
    pub fn with_root(name: impl Into<Arc<str>>) -> Self {
        let mut a = Self::default();
        a.alloc(name.into(), None, None);
        a
    }

    pub fn root(&self) -> NodeId {
        // `with_root` 가 항상 0번 슬롯을 쓰므로 첫 노드가 루트다.
        NodeId::new(0, 0)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn alloc(
        &mut self,
        name: Arc<str>,
        value: Option<NodeValue>,
        parent: Option<NodeId>,
    ) -> NodeId {
        self.alloc_flagged(name, value, parent, false)
    }

    /// `is_image_root` 까지 지정해 노드를 만든다.
    pub(crate) fn alloc_flagged(
        &mut self,
        name: Arc<str>,
        value: Option<NodeValue>,
        parent: Option<NodeId>,
        is_image_root: bool,
    ) -> NodeId {
        let entry = NodeEntry {
            name,
            value,
            parent,
            children: Vec::new(),
            generation: 0,
            is_image_root,
        };
        if let Some(idx) = self.free.pop() {
            // 세대를 올려야 이전 핸들이 이 슬롯을 다시 가리키지 못한다.
            let generation = self.entries[idx as usize].generation.wrapping_add(1);
            self.entries[idx as usize] = NodeEntry {
                generation,
                ..entry
            };
            NodeId::new(idx, generation)
        } else {
            let index = self.entries.len() as u32;
            self.entries.push(entry);
            NodeId::new(index, 0)
        }
    }

    /// 핸들이 현재 arena 와 유효한지 (세대 확인).
    pub fn valid(&self, id: NodeId) -> bool {
        self.entries
            .get(id.index as usize)
            .is_some_and(|e| e.generation == id.generation)
    }

    pub fn entry(&self, id: NodeId) -> Option<&NodeEntry> {
        let e = self.entries.get(id.index as usize)?;
        (e.generation == id.generation).then_some(e)
    }

    pub fn name(&self, id: NodeId) -> Option<&str> {
        self.entry(id).map(|e| &*e.name)
    }

    pub fn value(&self, id: NodeId) -> Option<&NodeValue> {
        self.entry(id).and_then(|e| e.value.as_ref())
    }

    pub fn parent(&self, id: NodeId) -> Option<NodeId> {
        self.entry(id).and_then(|e| e.parent)
    }

    pub fn children(&self, id: NodeId) -> &[NodeId] {
        self.entry(id).map(|e| e.children.as_slice()).unwrap_or(&[])
    }

    pub(crate) fn push_child(&mut self, parent: NodeId, child: NodeId) {
        if let Some(p) = self.entries.get_mut(parent.index as usize) {
            p.children.push(child);
        }
    }

    /// 슬롯을 해제해 나중에 재사용한다.
    ///
    /// 재사용 시 세대가 올라가므로 해제 전 핸들은 무효가 된다
    /// ([`Arena::valid`] 참조). 자식 슬롯은 재귀적으로 해제하지 않는다 —
    /// 부분 해제는 부모/자식 불변식을 깨뜨리므로 호출자가 트리 단위로 다룬다.
    pub fn release(&mut self, id: NodeId) {
        if !self.valid(id) {
            return;
        }
        if let Some(e) = self.entries.get_mut(id.index as usize) {
            e.children.clear();
            e.parent = None;
            // 세대를 **지금** 올려야 해제한 즉시 핸들이 무효가 된다.
            // 재사용 시점에 올리면 그전까지 `valid()` 가 참이라 깨진 링크가
            // 조용히 통과한다.
            e.generation = e.generation.wrapping_add(1);
        }
        self.free.push(id.index);
    }

    /// 이름으로 자식을 찾는다.
    ///
    /// # 함정: 중복 이름이면 **마지막** 것을 반환한다
    ///
    /// 원본 `Wz_Node.WzNodeCollection.this[string]` (`:419-440`) 은
    /// 사전에 없으면 뒤에서부터 선형 스캔한다. 즉 마지막 매치가 정답이다.
    /// 앞에서부터 찾으면 다른 노드를 잡아 경로 해석이 어긋난다.
    pub fn find_child(&self, parent: NodeId, name: &str) -> Option<NodeId> {
        let kids = self.children(parent);
        kids.iter()
            .rev()
            .copied()
            .find(|&c| self.name(c) == Some(name))
    }

    /// 새 노드를 만들어 붙인다.
    pub fn add_node(
        &mut self,
        parent: NodeId,
        name: impl Into<Arc<str>>,
        value: Option<NodeValue>,
    ) -> NodeId {
        let id = self.alloc(name.into(), value, Some(parent));
        self.push_child(parent, id);
        id
    }

    /// `wz-core` 노드 트리 전체를 arena 로 옮긴다. 반환값은 옮긴 루트 핸들.
    ///
    /// 중첩 `Property` 는 값이 아니라 **자식**으로 펴진다 (원본
    /// `ExtractImg` 과 동일). `ImageValue::Property` 에 들어 있던
    /// mini Property 와 평범한 자식 컨테이너가 arena 에서는 구분이 없다.
    pub fn ingest(&mut self, parent: NodeId, node: &WzNode) -> NodeId {
        let value = node.value.as_ref().and_then(convert_value);
        let id = self.alloc(node.name.clone(), value, Some(parent));
        self.push_child(parent, id);

        // 중첩 Property 가 있으면 먼저 펴고, 그 아래 실제 자식들을 붙인다.
        if let Some(wz_core::image::ImageValue::Property(inner)) = &node.value {
            for ch in &inner.children {
                self.ingest(id, ch);
            }
        }
        for ch in &node.children {
            self.ingest(id, ch);
        }
        id
    }

    /// 이미지 루트를 arena 로 옮긴다.
    ///
    /// 함정: 루트에 `is_image_root` 가 켜져야 한다. 이것이 없으면 `..` 가
    /// 이미지 경계를 인식 못 하고 디렉터리 트리 안으로 잘못 올라간다
    /// (`Wz_Uol.HandleUol` 의 이미지 이탈 동작이 깨진다).
    pub fn ingest_image(&mut self, parent: NodeId, node: &WzNode) -> NodeId {
        let value = node.value.as_ref().and_then(convert_value);
        let id = self.alloc_flagged(node.name.clone(), value, Some(parent), true);
        self.push_child(parent, id);
        if let Some(wz_core::image::ImageValue::Property(inner)) = &node.value {
            for ch in &inner.children {
                self.ingest(id, ch);
            }
        }
        for ch in &node.children {
            self.ingest(id, ch);
        }
        id
    }

    /// 이미지 루트인가.
    pub fn is_image_root(&self, id: NodeId) -> bool {
        self.entry(id).is_some_and(|e| e.is_image_root)
    }

    /// `.img` 엔트리에서 이미지 루트로 내려간다.
    ///
    /// 원본 `FindNodeByPath(extractImage: true)` 가 `Wz_Image` 값을 만나면
    /// `img.Node` 로 내려가는 동작에 해당한다. `.img` 엔트리가 아니라면
    /// 자기 자신을 그대로 반환한다.
    pub fn descend_image(&self, id: NodeId) -> NodeId {
        self.children(id)
            .iter()
            .copied()
            .find(|&c| self.is_image_root(c))
            .unwrap_or(id)
    }

    /// arena 전체를 `wz-core` 트리로 되돌린다 (직렬화/round-trip 용).
    pub fn to_wz_node(&self, id: NodeId) -> Option<WzNode> {
        let e = self.entry(id)?;
        let children: Vec<WzNode> = e
            .children
            .iter()
            .filter_map(|&c| self.to_wz_node(c))
            .collect();
        Some(WzNode {
            name: e.name.clone(),
            value: e.value.as_ref().map(convert_back),
            children,
        })
    }
}

/// `ImageValue` → `NodeValue`. `Property` 는 호출자가 펴므로 `None`.
fn convert_value(v: &ImageValue) -> Option<NodeValue> {
    Some(match v {
        ImageValue::Property(_) => return None,
        ImageValue::Null => NodeValue::Null,
        ImageValue::Short(x) => NodeValue::Short(*x),
        ImageValue::Int(x) => NodeValue::Int(*x),
        ImageValue::Long(x) => NodeValue::Long(*x),
        ImageValue::Uint(x) => NodeValue::Uint(*x),
        ImageValue::Float(x) => NodeValue::Float(*x),
        ImageValue::Double(x) => NodeValue::Double(*x),
        ImageValue::Bool(x) => NodeValue::Bool(*x),
        ImageValue::Str(x) => NodeValue::Str(x.clone()),
        ImageValue::Vector(v) => NodeValue::Vector(*v),
        ImageValue::Uol(x) => NodeValue::Uol(x.clone()),
        ImageValue::Png(p) => NodeValue::Canvas(p.clone()),
        ImageValue::RawData(r) => NodeValue::RawData(RawDataRef {
            offset: r.offset,
            len: r.len,
        }),
        ImageValue::Video(r) => NodeValue::Video(r.clone()),
        ImageValue::Sound(s) => NodeValue::Sound(s.clone()),
        ImageValue::Convex(c) => NodeValue::Convex(c.clone()),
    })
}

fn convert_back(v: &NodeValue) -> ImageValue {
    match v {
        NodeValue::Null => ImageValue::Null,
        NodeValue::Short(x) => ImageValue::Short(*x),
        NodeValue::Int(x) => ImageValue::Int(*x),
        NodeValue::Long(x) => ImageValue::Long(*x),
        NodeValue::Uint(x) => ImageValue::Uint(*x),
        NodeValue::Float(x) => ImageValue::Float(*x),
        NodeValue::Double(x) => ImageValue::Double(*x),
        NodeValue::Bool(x) => ImageValue::Bool(*x),
        NodeValue::Str(x) => ImageValue::Str(x.clone()),
        NodeValue::Vector(v) => ImageValue::Vector(*v),
        NodeValue::Uol(x) => ImageValue::Uol(x.clone()),
        NodeValue::Canvas(p) => ImageValue::Png(p.clone()),
        // arena 는 mini Property 를 이미 펴므로 `sub_property` 는 잃는다.
        // 픽셀 데이터는 미로딩이라 오프셋/길이만 보존하면 충분하다.
        NodeValue::RawData(r) => ImageValue::RawData(wz_core::image::RawDataRef {
            offset: r.offset,
            len: r.len,
            sub_property: None,
        }),
        NodeValue::Video(r) => ImageValue::Video(r.clone()),
        NodeValue::Sound(s) => ImageValue::Sound(s.clone()),
        NodeValue::Convex(c) => ImageValue::Convex(c.clone()),
    }
}
