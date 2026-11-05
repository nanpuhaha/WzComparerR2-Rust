//! arena/해석 테스트.
//!
//! 두 종류를 고정한다:
//! 1. 원본이 틀리는 경우(함정) — C# 대조에서 정상 경로가 검증되지 않는 부분.
//! 2. StringLinker 파리티(ticket #25) — 합성 String/Item/Etc 트리로 공개 경계를
//!    고정한다. 실제 WZ 가 없어도 `.img` 레이아웃/UOL/업데이트 규칙을 검증한다.

use std::sync::Arc;

use wz_core::image::Node as WzNode;

use crate::{Arena, NodeValue, ResolveError};

fn node(name: &str) -> WzNode {
    WzNode {
        name: Arc::from(name),
        value: None,
        children: Vec::new(),
    }
}

fn node_val(name: &str, v: NodeValue) -> WzNode {
    WzNode {
        name: Arc::from(name),
        value: Some(match v {
            NodeValue::Uol(s) => wz_core::image::ImageValue::Uol(s),
            NodeValue::Int(i) => wz_core::image::ImageValue::Int(i),
            NodeValue::Str(s) => wz_core::image::ImageValue::Str(s),
            other => panic!("테스트에서 지원 안 하는 값: {other:?}"),
        }),
        children: Vec::new(),
    }
}

fn with_children(mut n: WzNode, kids: Vec<WzNode>) -> WzNode {
    n.children = kids;
    n
}

/// 파일 트리와 같은 층위를 만든다.
///
/// ```text
/// (arena 루트)
///   └ Etc            ← 디렉터리 노드
///       ├ Etc.img    ← `.img` 엔트리 (원본의 Wz_Image 를 품는 노드)
///       │   └ (이미지 루트, is_image_root)
///       └ Other.img
///           └ (이미지 루트)
/// ```
///
/// # 함정: 디렉터리 단계가 빠져 있다
///
/// `.img` 엔트리를 arena 루트에 바로 붙이면 `..` 가 이미지 루트 → 엔트리 → 루트
///로 **두 단계** 올라가 버렉 트리를 통째로 벗어난다. 그래서 디렉터리 노드가
/// 반드시 하나 더 있어야 `../Other.img` 가 같은 디렉터리를 가리킨다.
fn fixture() -> (Arena, crate::NodeId) {
    let mut a = Arena::with_root("");
    let etc = a.add_node(a.root(), "Etc", None);
    let etc_entry = a.add_node(etc, "Etc.img", None);
    let img = a.ingest_image(
        etc_entry,
        &with_children(
            node("Etc.img"),
            vec![
                with_children(node("info"), vec![node_val("value", NodeValue::Int(42))]),
                node_val("alias", NodeValue::Uol(Arc::from("../Other.img"))),
                // 자기 자신을 가리키는 링크 (원본은 무한 루프).
                node_val("selfref", NodeValue::Uol(Arc::from("selfref"))),
                node_val("cycleA", NodeValue::Uol(Arc::from("cycleB"))),
                node_val("cycleB", NodeValue::Uol(Arc::from("cycleA"))),
                // 다른 이미지 **내부**로 파고드는 경로.
                //
                // 함정: 확장자를 **뺀** 세그먼트(`../Other/x`)여야 원본의
                // `.img` 재시도(`extractImage: true`)가 걸려 이미지 안으로
                // 내려간다. `../Other.img/x` 처럼 확장자가 붙은 채로 조회하면
                // 첫 조회에서 걸러져 `extractImage` 없이 끝나 실패한다.
                node_val("loose", NodeValue::Uol(Arc::from("../Other/x"))),
                // 위의 대칭 — 이쪽은 원본도 null.
                node_val("deep", NodeValue::Uol(Arc::from("../Other.img/x"))),
                // 없는 이름.
                node_val("missing", NodeValue::Uol(Arc::from("no_such_node"))),
            ],
        ),
    );
    let other_entry = a.add_node(etc, "Other.img", None);
    let other = a.ingest_image(
        other_entry,
        &with_children(node("Other.img"), vec![node("x")]),
    );
    assert!(a.valid(img));
    assert!(a.valid(other));
    (a, img)
}

#[test]
fn plain_child_lookup() {
    let (a, img) = fixture();
    let info = a.find_child(img, "info").expect("info 존재");
    let value = a.find_child(info, "value").expect("value 존재");
    assert_eq!(a.name(value), Some("value"));
    assert_eq!(a.value(value), Some(&NodeValue::Int(42)));
}

#[test]
fn find_path_walks_segments() {
    let (a, img) = fixture();
    let got = crate::find_path(&a, img, "info/value").expect("경로 해석");
    assert_eq!(a.name(got), Some("value"));
}

#[test]
fn find_path_missing_returns_none() {
    let (a, img) = fixture();
    assert!(crate::find_path(&a, img, "info/nope").is_none());
}

#[test]
fn resolve_uol_crosses_image_boundary_to_entry() {
    // `../Other.img` → 이미지를 벗어나 Etc 디렉터리로 나가 `Other.img` 엔트리.
    let (a, img) = fixture();
    let alias = a.find_child(img, "alias").expect("alias 존재");
    let got = crate::resolve_uol(&a, alias).expect("해석 성공");
    assert_eq!(a.name(got), Some("Other.img"));
    assert!(!a.is_image_root(got), "결과는 이미지 루트가 아니라 엔트리");
}

#[test]
fn resolve_uol_into_other_image_inner_node_is_null_like_csharp() {
    // 원본이 **풀지 못하는** 경우를 고정한다.
    //
    // `Wz_Image.Wz_ImageNode` (`Wz_Image.cs:1033-1041`) 는 `Nodes` 접근 시
    // `TryExtract` 을 호출하도록 **오버라이드하지 않는다**. 그래서 디렉터리에서
    // `Other.img` 엔트리를 찾아도 그 **안**(`x`)으로는 내려갈 수 없다 —
    // 추출은 `FindNodeByPath(extractImage: true)` 에서만 일어나고,
    // `HandleUol` 의 평범한 조회(`:59`)는 `extractImage` 가 false 다.
    //
    // 즉 C# 은 이 UOL 을 `null` 로 돌려준다. 우리도 같은 값을 낸다 —
    // 여기서 "더 잘" 풀어버리면 C# 과 대조가 불가능해진다.
    let (a, img) = fixture();
    let deep = a.find_child(img, "deep").expect("deep 존재");
    assert_eq!(
        crate::resolve_uol(&a, deep),
        Err(ResolveError::NotFound),
        "C# 과 동일하게 미해석이어야 함"
    );
}

#[test]
fn resolve_uol_self_reference_is_not_a_cycle() {
    // 자기 자신을 가리키는 링크. 원본 `ResolveUol` 은 `while` 조건이 계속 참이라
    // **무한 루프**한다. 여기서는 즉시 `Cycle` 로 끝난다.
    let (a, img) = fixture();
    let selfref = a.find_child(img, "selfref").expect("selfref 존재");
    assert_eq!(
        crate::resolve_uol(&a, selfref),
        Err(ResolveError::Cycle),
        "자기참조는 사이클이어야 함 (원본은 무한 루프)"
    );
}

#[test]
fn resolve_uol_mutual_cycle_detected() {
    // 원본은 `cycleA → cycleB → cycleA` 에서 무한 루프한다.
    let (a, img) = fixture();
    let a_node = a.find_child(img, "cycleA").expect("cycleA 존재");
    assert_eq!(
        crate::resolve_uol(&a, a_node),
        Err(ResolveError::Cycle),
        "상호 순환은 즉시 감지되어야 함"
    );
}

#[test]
fn resolve_uol_loose_segment_descends_into_other_image() {
    // `../Other/x` — `.img` 재시도가 `extractImage: true` 로 동작해
    // 이미지 **내부** `x` 까지 도달한다 (원본 `HandleUol:61-68`).
    let (a, img) = fixture();
    let loose = a.find_child(img, "loose").expect("loose 존재");
    let got = crate::resolve_uol(&a, loose).expect("해석 성공");
    assert_eq!(a.name(got), Some("x"));
    assert!(
        !a.is_image_root(got),
        "재시도로 이미지 **안**까지 내려갔어야 함 (extractImage=true)"
    );
}

#[test]
fn missing_uol_name_is_not_found() {
    let (a, img) = fixture();
    let m = a.find_child(img, "missing").expect("missing 존재");
    assert_eq!(crate::resolve_uol(&a, m), Err(ResolveError::NotFound));
}

#[test]
fn resolve_uol_chain_terminates_on_value() {
    // 값 노드에 닿으면 거기서 멈춘다 (UOL 가 계속 이어지지 않는다).
    let (a, img) = fixture();
    let alias = a.find_child(img, "alias").expect("alias 존재");
    let got = crate::resolve_uol(&a, alias).expect("해석 성공");
    assert!(!matches!(a.value(got), Some(NodeValue::Uol(_))));
}

#[test]
fn resolve_uol_not_found() {
    let mut a = Arena::with_root("");
    let dir = a.add_node(a.root(), "D.img", None);
    let img = a.ingest_image(
        dir,
        &with_children(
            node("D.img"),
            vec![node_val("a", NodeValue::Uol(Arc::from("nope")))],
        ),
    );
    let n = a.find_child(img, "a").expect("a 존재");
    assert_eq!(crate::resolve_uol(&a, n), Err(ResolveError::NotFound));
}

#[test]
fn duplicate_names_return_last() {
    // 원본 `WzNodeCollection.this[string]` 은 뒤에서부터 스캔하므로
    // **마지막** 매치를 준다. 앞에서 찾으면 경로가 어긋난다.
    let mut a = Arena::with_root("");
    let dir = a.add_node(a.root(), "E.img", None);
    let first = a.add_node(dir, "dup", Some(NodeValue::Int(1)));
    let last = a.add_node(dir, "dup", Some(NodeValue::Int(2)));
    assert_eq!(a.find_child(dir, "dup"), Some(last));
    assert_ne!(a.find_child(dir, "dup"), Some(first));
}

#[test]
fn generation_guard_rejects_stale_handle() {
    let mut a = Arena::with_root("");
    let n = a.add_node(a.root(), "x", None);
    assert!(a.valid(n));
    // 슬롯을 해제하면 세대가 올라가 이전 핸들이 무효가 된다.
    a.release(n);
    assert!(!a.valid(n), "해제된 핸들은 세대 검사를 통과하면 안 됨");
}

#[test]
fn ingest_flattens_nested_property() {
    // 중첩 `Property` 값은 arena 에서 자식이 된다.
    let inner = wz_core::image::Node {
        name: Arc::from("inner"),
        value: None,
        children: vec![wz_core::image::Node {
            name: Arc::from("k"),
            value: Some(wz_core::image::ImageValue::Int(9)),
            children: Vec::new(),
        }],
    };
    let src = wz_core::image::Node {
        name: Arc::from("P.img"),
        value: Some(wz_core::image::ImageValue::Property(Arc::new(inner))),
        children: Vec::new(),
    };
    let mut a = Arena::with_root("");
    let dir = a.add_node(a.root(), "P.img", None);
    let img = a.ingest_image(dir, &src);
    let k = a
        .find_child(img, "k")
        .expect("Property 안의 자식이 펴져야 함");
    assert_eq!(a.value(k), Some(&NodeValue::Int(9)));
}

#[test]
fn round_trip_to_wz_node_preserves_scalars() {
    let src = with_children(
        node("R.img"),
        vec![
            node_val("s", NodeValue::Str(Arc::from("hi"))),
            node_val("i", NodeValue::Int(-7)),
        ],
    );
    let mut a = Arena::with_root("");
    let dir = a.add_node(a.root(), "R.img", None);
    let img = a.ingest_image(dir, &src);
    let back = a.to_wz_node(img).expect("역변환");
    assert_eq!(back.name.as_ref(), "R.img");
    assert_eq!(back.children.len(), 2);
    assert_eq!(
        back.children[0].value,
        Some(wz_core::image::ImageValue::Str(Arc::from("hi")))
    );
    assert_eq!(
        back.children[1].value,
        Some(wz_core::image::ImageValue::Int(-7))
    );
}

#[test]
fn query_engine_matches_numeric_string_and_vector_rules() {
    use wz_core::image::Vec2;

    use crate::query::{
        execute_query, QueryRule, QueryValueType, StringComparisonMode, ValueRule,
        DEFAULT_MAX_RESULTS,
    };

    let mut a = Arena::with_root("");
    let item1 = a.add_node(a.root(), "01000000.img", None);
    let info1 = a.add_node(item1, "info", None);
    a.add_node(info1, "reqLevel", Some(NodeValue::Int(150)));
    a.add_node(info1, "islot", Some(NodeValue::Str(Arc::from("Cp"))));
    a.add_node(
        info1,
        "origin",
        Some(NodeValue::Vector(Vec2 { x: 16, y: 32 })),
    );

    let item2 = a.add_node(a.root(), "01000001.img", None);
    let info2 = a.add_node(item2, "info", None);
    a.add_node(info2, "reqLevel", Some(NodeValue::Int(50)));
    a.add_node(info2, "islot", Some(NodeValue::Str(Arc::from("Cp"))));

    let mut rule = QueryRule::target(StringComparisonMode::EndsWith, ".img");
    rule.values.push(ValueRule::new(
        "info/reqLevel",
        QueryValueType::Number,
        5, // >=
        "100",
    ));
    rule.values.push(ValueRule::new(
        "info/origin",
        QueryValueType::Vector,
        0, // ==
        "(16, 32)",
    ));

    let res = execute_query(&a, a.root(), &[rule], DEFAULT_MAX_RESULTS);
    assert_eq!(res.results.len(), 1);
    assert_eq!(res.results[0].path, "01000000.img");
    assert!(res.results[0].value.contains("reqLevel: 150"));
    assert!(res.results[0].value.contains("origin: (16, 32)"));
}

#[test]
fn string_linker_indexes_consume_and_eqp_with_uol() {
    use crate::linker::StringLinker;

    let mut a = Arena::with_root("");
    let string_root = a.add_node(a.root(), "String.wz", None);

    // Consume.img
    let consume_entry = a.add_node(string_root, "Consume.img", None);
    let consume_img = a.ingest_image(
        consume_entry,
        &with_children(
            node("Consume.img"),
            vec![
                with_children(
                    node("2000000"),
                    vec![
                        node_val("name", NodeValue::Str(Arc::from("빨간 포션"))),
                        node_val("desc", NodeValue::Str(Arc::from("HP를 50 회복시킨다."))),
                    ],
                ),
                node_val("2000001", NodeValue::Uol(Arc::from("2000000"))),
            ],
        ),
    );
    assert!(a.valid(consume_img));

    // Eqp.img -> Eqp -> Cap -> 1000000
    let eqp_entry = a.add_node(string_root, "Eqp.img", None);
    let _eqp_img = a.ingest_image(
        eqp_entry,
        &with_children(
            node("Eqp.img"),
            vec![with_children(
                node("Eqp"),
                vec![with_children(
                    node("Cap"),
                    vec![with_children(
                        node("1000000"),
                        vec![node_val("name", NodeValue::Str(Arc::from("가죽 모자")))],
                    )],
                )],
            )],
        ),
    );

    let mut linker = StringLinker::new();
    assert!(linker.load_string_wz(&a, string_root));
    assert_eq!(
        linker.format_tooltip(2000000).as_deref(),
        Some("빨간 포션\nHP를 50 회복시킨다.")
    );
    // UOL 해석된 2000001도 동일한 이름/설명을 가져야 함
    assert_eq!(
        linker.format_tooltip(2000001).as_deref(),
        Some("빨간 포션\nHP를 50 회복시킨다.")
    );
    assert_eq!(linker.format_tooltip(1000000).as_deref(), Some("가죽 모자"));
}

#[test]
fn string_linker_real_wz() {
    use crate::linker::StringLinker;
    use crate::Arena;
    use wz_core::WzArchive;

    let path = std::env::var_os("WZ_DATA_ROOT")
        .map(std::path::PathBuf::from)
        .map(|root| root.join("String").join("String_000.wz"));
    let Some(path) = path else { return };
    if !path.exists() {
        return;
    }
    let arch = WzArchive::open(&path).unwrap();
    let (arena, root) = Arena::from_archive(&arch);
    let mut linker = StringLinker::new();
    assert!(linker.load_string_wz(&arena, root));
    // Should have parsed some eqp, skill, map, mob, npc strings
    assert!(!linker.string_eqp.is_empty(), "eqp should not be empty");
    assert!(
        !linker.string_skill.is_empty() || !linker.string_skill_str.is_empty(),
        "skill should not be empty"
    );
    assert!(!linker.string_map.is_empty(), "map should not be empty");
    assert!(!linker.string_mob.is_empty(), "mob should not be empty");
    assert!(!linker.string_npc.is_empty(), "npc should not be empty");
    // format_tooltip should work for a known item
    let tooltip = linker.format_tooltip(1000000);
    assert!(tooltip.is_some(), "tooltip for 1000000 should exist");
}

#[test]
fn string_linker_skill_info_real_wz() {
    use crate::linker::StringLinker;
    use crate::Arena;
    use wz_core::WzArchive;

    let path = std::env::var_os("WZ_DATA_ROOT")
        .map(std::path::PathBuf::from)
        .map(|root| root.join("String").join("String_000.wz"));
    let Some(path) = path else { return };
    if !path.exists() {
        return;
    }
    let arch = WzArchive::open(&path).unwrap();
    let (arena, root) = Arena::from_archive(&arch);
    let mut linker = StringLinker::new();
    assert!(linker.load_string_wz(&arena, root));
    // Should have skill strings
    assert!(
        !linker.string_skill.is_empty() || !linker.string_skill_str.is_empty(),
        "skill strings should not be empty"
    );
    // Try to get a skill tooltip
    // Skill IDs are like 0000012 (12), 0000073 (73), etc.
    for (id, sr) in &linker.string_skill {
        if !sr.name.is_empty() {
            // Verify format_tooltip works
            let tooltip = linker.format_tooltip(*id);
            assert!(tooltip.is_some(), "tooltip for skill {} should exist", id);
            break;
        }
    }
}
// ────────────────────────────────────────────────────────────────────────
// StringLinker 파리티 (ticket #25)
//
// 아래 테스트는 전부 `StringLinker` 의 **공개** 경계
// (`load_*` → `string_*` 사전 / `get_by_id` / `format_tooltip`) 만 통과한다.
// ────────────────────────────────────────────────────────────────────────

/// `String.wz` 루트를 arena 루트에 그대로 붙인다 (CLI 가 쓰는 형태).
///
/// 원본 `StringLinker.Load(Wz_File stringWz, ...)` 는 `stringWz.Node` 를
/// 넘기는데 그 노드의 **직접 자식**이 `Consume.img` 같은 `.img` 엔트리다
/// (`StringLinker.cs:67-71`). 즉 `.img` 노드가 바로 아래에 붙는 층위가
/// String.wz 루트다.
fn string_root_direct(a: &Arena) -> crate::NodeId {
    a.root()
}

/// 이미지를 `.img` 엔트리 + 이미지 루트로 붙인다.
fn add_img(
    a: &mut Arena,
    parent: crate::NodeId,
    img_name: &str,
    children: Vec<WzNode>,
) -> crate::NodeId {
    let entry = a.add_node(parent, img_name, None);
    a.ingest_image(entry, &with_children(node(img_name), children))
}

/// 일반 컨테이너 노드를 붙인다 (이미지가 아닌 GMS 포맷 경로용).
fn add_plain_dir(
    a: &mut Arena,
    parent: crate::NodeId,
    name: &str,
    children: Vec<WzNode>,
) -> crate::NodeId {
    let dir = a.add_node(parent, name, None);
    for c in children {
        a.ingest(dir, &c);
    }
    dir
}

// ── Problem B: string_root 레이아웃 고정 ────────────────────────────────

#[test]
fn string_linker_takes_the_wz_file_root_whose_children_are_img_entries() {
    // C# `Load(stringWz.Node, ...)` — `.img` 엔트리가 **직접 자식**이어야 한다.
    let mut a = Arena::with_root("");
    let root = string_root_direct(&a);
    add_img(
        &mut a,
        root,
        "Consume.img",
        vec![with_children(
            node("2000000"),
            vec![node_val("name", NodeValue::Str(Arc::from("빨간 포션")))],
        )],
    );

    let mut linker = crate::linker::StringLinker::new();
    assert!(linker.load_string_wz(&a, root));
    assert_eq!(
        linker.string_item.get(&2000000).map(|r| r.name.as_str()),
        Some("빨간 포션")
    );
    // `.img` 엔트리가 바로 아래에 없으면 아무 것도 잡지 않는다 (오탐 없는 실패).
    let mut extra = Arena::with_root("");
    let wrapper = extra.add_node(extra.root(), "String.wz", None);
    add_img(
        &mut extra,
        wrapper,
        "Consume.img",
        vec![with_children(
            node("2000000"),
            vec![node_val("name", NodeValue::Str(Arc::from("빨간 포션")))],
        )],
    );
    let mut linker2 = crate::linker::StringLinker::new();
    assert!(!linker2.load_string_wz(&extra, extra.root()));
    assert!(linker2.string_item.is_empty());
}

// ── GMS 포맷: Wz_Image 가 아닌 노드도 그대로 순회 ──────────────────────

#[test]
fn string_linker_walks_img_nodes_that_are_not_wz_images() {
    // `StringLinker.cs:70` `Wz_Node imgNode = node;` — `Wz_Image` 가 아니면
    // `.img` 엔트리 노드 자체를 순회한다 (GMS 포맷 경로).
    let mut a = Arena::with_root("");
    let root = string_root_direct(&a);
    add_plain_dir(
        &mut a,
        root,
        "Consume.img",
        vec![with_children(
            node("2000000"),
            vec![node_val("name", NodeValue::Str(Arc::from("빨간 포션")))],
        )],
    );

    let mut linker = crate::linker::StringLinker::new();
    assert!(linker.load_string_wz(&a, root));
    assert_eq!(
        linker.string_item.get(&2000000).map(|r| r.name.as_str()),
        Some("빨간 포션")
    );
}

// ── 기존 미검증 분기 ────────────────────────────────────────────────────

#[test]
fn string_linker_etc_img_reads_two_levels_under_a_category() {
    // `StringLinker.cs:110-113` — `Etc.img/<category>/<id>` (두 단계만 내려간다).
    let mut a = Arena::with_root("");
    let root = string_root_direct(&a);
    add_img(
        &mut a,
        root,
        "Etc.img",
        vec![with_children(
            node("Item"),
            vec![with_children(
                node("4000000"),
                vec![
                    node_val("name", NodeValue::Str(Arc::from("반지 표면"))),
                    node_val("desc", NodeValue::Str(Arc::from("표면을 깎는다."))),
                ],
            )],
        )],
    );

    let mut linker = crate::linker::StringLinker::new();
    assert!(linker.load_string_wz(&a, root));
    let sr = linker.string_item.get(&4000000).expect("Etc.img 항목");
    assert_eq!(sr.name, "반지 표면");
    assert_eq!(sr.desc.as_deref(), Some("표면을 깎는다."));
    assert_eq!(sr.full_path, "Etc.img/Item/4000000");
}

#[test]
fn string_linker_mob_img_records_name_without_desc() {
    let mut a = Arena::with_root("");
    let root = string_root_direct(&a);
    add_img(
        &mut a,
        root,
        "Mob.img",
        vec![with_children(
            node("100"),
            vec![
                node_val("name", NodeValue::Str(Arc::from("슬라임"))),
                node_val("desc", NodeValue::Str(Arc::from("이건 쓰이지 않는다"))),
            ],
        )],
    );

    let mut linker = crate::linker::StringLinker::new();
    assert!(linker.load_string_wz(&a, root));
    let sr = linker.string_mob.get(&100).expect("Mob.img 항목");
    assert_eq!(sr.name, "슬라임");
    // Mob.img 은 `desc` 를 읽지 않는다 (`StringLinker.cs:181`).
    assert_eq!(sr.desc, None);
    assert_eq!(sr.full_path, "Mob.img/100");
}

#[test]
fn string_linker_npc_img_maps_func_onto_desc() {
    // `StringLinker.cs:239` — `Desc = GetDefaultString(linkNode, "func")`.
    let mut a = Arena::with_root("");
    let root = string_root_direct(&a);
    add_img(
        &mut a,
        root,
        "Npc.img",
        vec![with_children(
            node("100"),
            vec![
                node_val("name", NodeValue::Str(Arc::from("헤브"))),
                node_val("func", NodeValue::Str(Arc::from("마을 치료사"))),
            ],
        )],
    );

    let mut linker = crate::linker::StringLinker::new();
    assert!(linker.load_string_wz(&a, root));
    let sr = linker.string_npc.get(&100).expect("Npc.img 항목");
    assert_eq!(sr.name, "헤브");
    assert_eq!(sr.desc.as_deref(), Some("마을 치료사"));
}

#[test]
fn string_linker_map_img_synthesizes_street_and_map_name() {
    // `StringLinker.cs:269-274` — 이름은 항상 `"{street} : {map}"`.
    let mut a = Arena::with_root("");
    let root = string_root_direct(&a);
    add_img(
        &mut a,
        root,
        "Map.img",
        vec![with_children(
            node("Kenta"),
            vec![
                with_children(
                    node("100000000"),
                    vec![
                        node_val("streetName", NodeValue::Str(Arc::from(" murky"))),
                        node_val("mapName", NodeValue::Str(Arc::from("던전 Entrance"))),
                        node_val("mapDesc", NodeValue::Str(Arc::from("어두운 숲."))),
                    ],
                ),
                with_children(node("100000001"), vec![]),
            ],
        )],
    );

    let mut linker = crate::linker::StringLinker::new();
    assert!(linker.load_string_wz(&a, root));
    let sr = linker.string_map.get(&100000000).expect("Map.img 항목");
    assert_eq!(sr.name, " murky : 던전 Entrance");
    assert_eq!(sr.street_name.as_deref(), Some(" murky"));
    assert_eq!(sr.map_name.as_deref(), Some("던전 Entrance"));
    assert_eq!(sr.desc.as_deref(), Some("어두운 숲."));
    assert_eq!(sr.full_path, "Map.img/Kenta/100000000");

    // `streetName`/`mapName` 이 모두 없을 때도 이름은 `" : "` 가 된다
    // (`string.Format` 은 null 을 빈 문자열로 찍는다).
    let empty = linker.string_map.get(&100000001).expect("빈 Map 항목");
    assert_eq!(empty.name, " : ");
    assert_eq!(empty.street_name, None);
}

#[test]
fn string_linker_skill_img_collects_pdesc_and_h_levels() {
    // `StringLinker.cs:312-354` — `pdesc` + `h1..hn` 루프.
    let mut a = Arena::with_root("");
    let root = string_root_direct(&a);
    add_img(
        &mut a,
        root,
        "Skill.img",
        vec![with_children(
            node("Beginner"),
            vec![
                node_val("name", NodeValue::Str(Arc::from("초보자 스킬"))),
                node_val("desc", NodeValue::Str(Arc::from("기본 공격"))),
                node_val("pdesc", NodeValue::Str(Arc::from("상세 설명"))),
                node_val("h1", NodeValue::Str(Arc::from("1단계"))),
                node_val("h2", NodeValue::Str(Arc::from("2단계"))),
            ],
        )],
    );

    let mut linker = crate::linker::StringLinker::new();
    linker.load_string_wz(&a, root);
    let sr = linker
        .string_skill_str
        .get("Beginner")
        .expect("스킬 문자열");
    assert_eq!(sr.name, "초보자 스킬");
    assert_eq!(sr.pdesc.as_deref(), Some("상세 설명"));
    assert_eq!(sr.skill_h, vec!["1단계".to_string(), "2단계".to_string()]);
    assert_eq!(sr.skill_ph, Vec::<String>::new());
    assert_eq!(sr.full_path, "Skill.img/Beginner");
    // 이름만으로는 숫자 ID 사전에 들어가지 않는다.
    assert!(linker.string_skill.is_empty());
}

#[test]
fn string_linker_skill_img_prefers_h_over_numbered_levels() {
    // `StringLinker.cs:314-354` — `h` 가 있으면 `h1..hn` 루프는 돌지 않는다.
    let mut a = Arena::with_root("");
    let root = string_root_direct(&a);
    add_img(
        &mut a,
        root,
        "Skill.img",
        vec![with_children(
            node("Slash"),
            vec![
                node_val("h", NodeValue::Str(Arc::from("기본"))),
                node_val("h1", NodeValue::Str(Arc::from("무시된다"))),
            ],
        )],
    );

    let mut linker = crate::linker::StringLinker::new();
    linker.load_string_wz(&a, root);
    let sr = linker.string_skill_str.get("Slash").expect("스킬 문자열");
    assert_eq!(sr.skill_h, vec!["기본".to_string()]);
}

#[test]
fn string_linker_skill_img_stops_at_empty_h_level() {
    // `StringLinker.cs:345-346` — 빈 문자열이면 반복을 멈춘다.
    let mut a = Arena::with_root("");
    let root = string_root_direct(&a);
    add_img(
        &mut a,
        root,
        "Skill.img",
        vec![with_children(
            node("Dash"),
            vec![
                node_val("h1", NodeValue::Str(Arc::from("1"))),
                node_val("h2", NodeValue::Str(Arc::from(""))),
                node_val("h3", NodeValue::Str(Arc::from("3"))),
            ],
        )],
    );

    let mut linker = crate::linker::StringLinker::new();
    linker.load_string_wz(&a, root);
    let sr = linker.string_skill_str.get("Dash").expect("스킬 문자열");
    assert_eq!(sr.skill_h, vec!["1".to_string()]);
}

#[test]
fn string_linker_skill_img_collects_ph_hch_and_h_prefixed_levels() {
    // `StringLinker.cs:321-367` — `ph`, `hch`, `h_<level>` (KMST1196).
    let mut a = Arena::with_root("");
    let root = string_root_direct(&a);
    add_img(
        &mut a,
        root,
        "Skill.img",
        vec![with_children(
            node("Meteor"),
            vec![
                node_val("h", NodeValue::Str(Arc::from("기본"))),
                node_val("ph", NodeValue::Str(Arc::from("인파이어"))),
                node_val("hch", NodeValue::Str(Arc::from("충성스킬"))),
                node_val("h_3", NodeValue::Str(Arc::from("3성"))),
                node_val("h_1", NodeValue::Str(Arc::from("1성"))),
                node_val("h_0", NodeValue::Str(Arc::from("0성은 버린다"))),
            ],
        )],
    );

    let mut linker = crate::linker::StringLinker::new();
    linker.load_string_wz(&a, root);
    let sr = linker.string_skill_str.get("Meteor").expect("스킬 문자열");
    assert_eq!(sr.skill_ph, vec!["인파이어".to_string()]);
    assert_eq!(sr.skill_hch, vec!["충성스킬".to_string()]);
    // `level > 0` 만 남고 레벨 순으로 정렬된다.
    assert_eq!(
        sr.skill_extra_h,
        vec![(1, "1성".to_string()), (3, "3성".to_string())]
    );
}

#[test]
fn string_linker_skill_img_indexes_numeric_ids_only_when_7_digits_or_more() {
    // `StringLinker.cs:373-377` — 이름 길이 >= 7 이고 숫자로 파싱될 때만
    // `string_skill` 에 들어간다.
    let mut a = Arena::with_root("");
    let root = string_root_direct(&a);
    for name in ["1000000", "100000", "10000000"] {
        add_img(
            &mut a,
            root,
            "Skill.img",
            vec![with_children(
                node(name),
                vec![node_val("name", NodeValue::Str(Arc::from("n")))],
            )],
        );
    }

    let mut linker = crate::linker::StringLinker::new();
    assert!(linker.load_string_wz(&a, root));
    assert!(linker.string_skill.contains_key(&1000000));
    assert!(linker.string_skill.contains_key(&10000000));
    assert!(!linker.string_skill.contains_key(&100000));
    assert_eq!(linker.string_skill_str.len(), 3);
}

#[test]
fn string_linker_item_branches_read_autodesc() {
    // `StringLinker.cs:96` — `autodesc` → `AutoDesc`.
    let mut a = Arena::with_root("");
    let root = string_root_direct(&a);
    for (img, id) in [
        ("Pet.img", 5000000),
        ("Cash.img", 5000001),
        ("Ins.img", 5000002),
        ("Consume.img", 2000000),
    ] {
        add_img(
            &mut a,
            root,
            img,
            vec![with_children(
                node(id.to_string().as_str()),
                vec![
                    node_val("name", NodeValue::Str(Arc::from("이름"))),
                    node_val("autodesc", NodeValue::Str(Arc::from("자동설명"))),
                ],
            )],
        );
    }

    let mut linker = crate::linker::StringLinker::new();
    assert!(linker.load_string_wz(&a, root));
    for id in [5000000, 5000001, 5000002, 2000000] {
        assert_eq!(
            linker
                .string_item
                .get(&id)
                .and_then(|r| r.auto_desc.clone())
                .as_deref(),
            Some("자동설명"),
            "id {id}"
        );
    }
}

// ── 새로 추가한 `.img` 케이스 ───────────────────────────────────────────

#[test]
fn string_linker_familiar_img_reads_skill_subtree_only() {
    // `StringLinker.cs:134-162` — `Familiar.img/skill/<id>`.
    let mut a = Arena::with_root("");
    let root = string_root_direct(&a);
    add_img(
        &mut a,
        root,
        "Familiar.img",
        vec![
            with_children(
                node("skill"),
                vec![with_children(
                    node("1000"),
                    vec![
                        node_val("name", NodeValue::Str(Arc::from("마법사 스킬"))),
                        node_val("desc", NodeValue::Str(Arc::from("마법 공격"))),
                    ],
                )],
            ),
            // `skill` 이 아닌 자식은 무시된다.
            with_children(
                node("other"),
                vec![with_children(
                    node("2000"),
                    vec![node_val("name", NodeValue::Str(Arc::from("버려짐")))],
                )],
            ),
        ],
    );

    let mut linker = crate::linker::StringLinker::new();
    // `HasValues` 는 `stringFamiliarSkill` 을 세지 않는다 (`StringLinker.cs:736-743`).
    linker.load_string_wz(&a, root);
    let sr = linker
        .string_familiar_skill
        .get(&1000)
        .expect("Familiar 스킬");
    assert_eq!(sr.name, "마법사 스킬");
    assert_eq!(sr.desc.as_deref(), Some("마법 공격"));
    assert_eq!(sr.full_path, "Familiar.img/skill/1000");
    assert!(!linker.string_familiar_skill.contains_key(&2000));
    assert!(!linker.has_values());
}

#[test]
fn string_linker_familiar_skill_img_is_the_same_case_as_familiar() {
    let mut a = Arena::with_root("");
    let root = string_root_direct(&a);
    add_img(
        &mut a,
        root,
        "FamiliarSkill.img",
        vec![with_children(
            node("skill"),
            vec![with_children(
                node("3000"),
                vec![node_val("name", NodeValue::Str(Arc::from("공식 스킬")))],
            )],
        )],
    );

    let mut linker = crate::linker::StringLinker::new();
    linker.load_string_wz(&a, root);
    assert_eq!(
        linker
            .string_familiar_skill
            .get(&3000)
            .map(|r| r.name.as_str()),
        Some("공식 스킬")
    );
}

#[test]
fn string_linker_monster_book_img_borrows_name_from_mob_img() {
    // `StringLinker.cs:189-219` — `MonsterBook.img` 는 `Mob.img` 이 먼저
    // 로드되어 있어야 이름을 가져올 수 있다.
    let mut a = Arena::with_root("");
    let root = string_root_direct(&a);
    add_img(
        &mut a,
        root,
        "Mob.img",
        vec![with_children(
            node("100"),
            vec![node_val("name", NodeValue::Str(Arc::from("슬라임")))],
        )],
    );
    add_img(
        &mut a,
        root,
        "MonsterBook.img",
        vec![with_children(
            node("100"),
            vec![
                node_val("episode", NodeValue::Str(Arc::from("에피소드 설명"))),
                // `Mob.img` 에 없는 ID 는 무시된다.
                with_children(
                    node("999"),
                    vec![node_val("episode", NodeValue::Str(Arc::from("x")))],
                ),
            ],
        )],
    );

    let mut linker = crate::linker::StringLinker::new();
    linker.load_string_wz(&a, root);
    let sr = linker
        .string_monster_book
        .get(&100)
        .expect("MonsterBook 항목");
    assert_eq!(sr.name, "슬라임");
    assert_eq!(sr.desc.as_deref(), Some("에피소드 설명"));
    assert!(!linker.string_monster_book.contains_key(&999));
}

#[test]
fn string_linker_item_wz_special_0910_img_feeds_string_item() {
    // `StringLinker.cs:416-450` — `Item.wz/Special/0910.img/<id>`.
    let mut a = Arena::with_root("");
    let special = a.add_node(a.root(), "Special", None);
    add_img(
        &mut a,
        special,
        "0910.img",
        vec![with_children(
            node("8000000"),
            vec![
                node_val("name", NodeValue::Str(Arc::from("이벤트 아이템"))),
                node_val("desc", NodeValue::Str(Arc::from("한정 이벤트"))),
            ],
        )],
    );

    let mut linker = crate::linker::StringLinker::new();
    linker.load_item_wz(&a, a.root());
    let sr = linker.string_item.get(&8000000).expect("0910.img 항목");
    assert_eq!(sr.name, "이벤트 아이템");
    assert_eq!(sr.desc.as_deref(), Some("한정 이벤트"));
    assert_eq!(sr.full_path, "0910.img/8000000");
    // `0910.img` 은 `autodesc` 를 읽지 않는다.
    assert_eq!(sr.auto_desc, None);
}

#[test]
fn string_linker_etc_wz_set_item_info_reads_set_item_name() {
    // `StringLinker.cs:452-485` — `SetItemInfo.img` 는 `setItemName` 을 읽는다.
    let mut a = Arena::with_root("");
    let root = a.root();
    add_img(
        &mut a,
        root,
        "SetItemInfo.img",
        vec![with_children(
            node("9000000"),
            vec![
                node_val("setItemName", NodeValue::Str(Arc::from("세트 이름"))),
                node_val("name", NodeValue::Str(Arc::from("이건 안 쓴다"))),
                node_val("desc", NodeValue::Str(Arc::from("이것도 안 쓴다"))),
            ],
        )],
    );

    let mut linker = crate::linker::StringLinker::new();
    linker.load_etc_wz(&a, root);
    let sr = linker
        .string_set_item
        .get(&9000000)
        .expect("SetItemInfo 항목");
    assert_eq!(sr.name, "세트 이름");
    assert_eq!(sr.desc, None);
    assert_eq!(sr.full_path, "SetItemInfo.img/9000000");
    assert_eq!(
        sr.values.get("name").map(String::as_str),
        Some("이건 안 쓴다")
    );
}

#[test]
fn string_linker_world_archive_maps_mob_and_npc_ids_and_skips_world_name() {
    // `StringLinker.cs:517-625` — `worldArchive.img/collectionInfo/...`.
    let mut a = Arena::with_root("");
    let root = a.root();
    add_img(
        &mut a,
        root,
        "Mob.img",
        vec![with_children(
            node("100"),
            vec![node_val("name", NodeValue::Str(Arc::from("슬라임")))],
        )],
    );
    add_img(
        &mut a,
        root,
        "Npc.img",
        vec![with_children(
            node("200"),
            vec![node_val("name", NodeValue::Str(Arc::from("헤브")))],
        )],
    );
    // worldArchive 는 Etc.wz 쪽이므로 별도 루트로 읽는다.
    let mut etc = Arena::with_root("");
    let etc_root = etc.root();
    add_img(
        &mut etc,
        etc_root,
        "worldArchive.img",
        vec![with_children(
            node("collectionInfo"),
            vec![with_children(
                node("channel"),
                vec![
                    // worldName/worldDesc 는 명시적으로 건너뛴다.
                    node_val("worldName", NodeValue::Str(Arc::from("월드 이름"))),
                    with_children(
                        node("1"),
                        vec![
                            with_children(
                                node("mob"),
                                vec![with_children(
                                    node("greenSlime"),
                                    vec![
                                        with_children(
                                            node("id"),
                                            vec![
                                                node_val("0", NodeValue::Int(100)),
                                                // 0 은 건너뛴다.
                                                node_val("1", NodeValue::Int(0)),
                                            ],
                                        ),
                                        node_val("desc", NodeValue::Str(Arc::from("초록 슬라임"))),
                                    ],
                                )],
                            ),
                            with_children(
                                node("npc"),
                                vec![with_children(
                                    node("heaven"),
                                    vec![
                                        with_children(
                                            node("id"),
                                            vec![node_val("0", NodeValue::Int(200))],
                                        ),
                                        // 비어 있으면 "(null)".
                                    ],
                                )],
                            ),
                        ],
                    ),
                ],
            )],
        )],
    );

    let mut linker = crate::linker::StringLinker::new();
    linker.load_string_wz(&a, a.root());
    linker.load_etc_wz(&etc, etc_root);

    let mob = linker
        .string_world_archive_mob
        .get(&100)
        .expect("월드아카이브 몹");
    assert_eq!(mob.name, "슬라임");
    assert_eq!(mob.desc.as_deref(), Some("초록 슬라임"));
    let npc = linker
        .string_world_archive_npc
        .get(&200)
        .expect("월드아카이브 엔피씨");
    assert_eq!(npc.name, "헤브");
    assert_eq!(npc.desc.as_deref(), Some("(null)"));
    assert_eq!(
        linker
            .string_world_archive_mob_by_path
            .get("channel_1_mob_greenSlime")
            .and_then(|r| r.desc.clone())
            .as_deref(),
        Some("초록 슬라임")
    );
    assert!(!linker
        .string_world_archive_mob_by_path
        .contains_key("channel_worldName"));
}

// ── update 모드 (bb98c25) ───────────────────────────────────────────────

#[test]
fn string_linker_update_mode_merges_without_rewriting_full_path() {
    // `StringLinker.cs:97` — `if (!update) strResult.FullPath = tree.FullPath;`
    // update 모드는 기존 결과를 이어받되 `FullPath` 는 최초 로드 값을 지킨다.
    let mut a = Arena::with_root("");
    let root = string_root_direct(&a);
    add_img(
        &mut a,
        root,
        "Consume.img",
        vec![with_children(
            node("2000000"),
            vec![
                node_val("name", NodeValue::Str(Arc::from("원래 이름"))),
                node_val("desc", NodeValue::Str(Arc::from("원래 설명"))),
            ],
        )],
    );

    let mut linker = crate::linker::StringLinker::new();
    assert!(linker.load_string_wz(&a, root));
    assert_eq!(
        linker.string_item[&2000000].full_path,
        "Consume.img/2000000"
    );

    // 두 번째 패치: 이름만 바뀐다.
    let mut b = Arena::with_root("");
    let broot = b.root();
    add_img(
        &mut b,
        broot,
        "Consume.img",
        vec![with_children(
            node("2000000"),
            vec![node_val("name", NodeValue::Str(Arc::from("패치 이름")))],
        )],
    );
    linker.update_string_wz(&b, broot);
    let sr = &linker.string_item[&2000000];
    assert_eq!(sr.name, "패치 이름");
    // desc 는 패치에 없으므로 유지된다.
    assert_eq!(sr.desc.as_deref(), Some("원래 설명"));
    // FullPath 는 update 모드에서 덮어쓰지 않는다.
    assert_eq!(sr.full_path, "Consume.img/2000000");
}

#[test]
fn string_linker_update_mode_replaces_skill_levels_once() {
    // `StringLinker.cs:342-353` — update 모드에서 `h1..hn` 루프는 **첫 비어 있지
    // 않은 값에서 한 번만** 기존 목록을 비운다 (`cleared` 플래그).
    let mut a = Arena::with_root("");
    let root = string_root_direct(&a);
    add_img(
        &mut a,
        root,
        "Skill.img",
        vec![with_children(
            node("Slash"),
            vec![
                node_val("h1", NodeValue::Str(Arc::from("기존 1"))),
                node_val("h2", NodeValue::Str(Arc::from("기존 2"))),
            ],
        )],
    );
    let mut linker = crate::linker::StringLinker::new();
    linker.load_string_wz(&a, root);
    assert_eq!(
        linker.string_skill_str["Slash"].skill_h,
        vec!["기존 1".to_string(), "기존 2".to_string()]
    );

    let mut b = Arena::with_root("");
    let broot = string_root_direct(&b);
    add_img(
        &mut b,
        broot,
        "Skill.img",
        vec![with_children(
            node("Slash"),
            vec![
                node_val("h1", NodeValue::Str(Arc::from("패치 1"))),
                node_val("h2", NodeValue::Str(Arc::from("패치 2"))),
            ],
        )],
    );
    linker.update_string_wz(&b, broot);
    assert_eq!(
        linker.string_skill_str["Slash"].skill_h,
        vec!["패치 1".to_string(), "패치 2".to_string()]
    );
}

// ── 조회/형식화 ─────────────────────────────────────────────────────────

#[test]
fn string_linker_get_by_id_priority_is_item_then_eqp_then_skill() {
    let mut a = Arena::with_root("");
    let root = string_root_direct(&a);
    add_img(
        &mut a,
        root,
        "Mob.img",
        vec![with_children(
            node("777"),
            vec![node_val("name", NodeValue::Str(Arc::from("몹")))],
        )],
    );
    add_img(
        &mut a,
        root,
        "Consume.img",
        vec![with_children(
            node("777"),
            vec![node_val("name", NodeValue::Str(Arc::from("아이템")))],
        )],
    );
    add_img(
        &mut a,
        root,
        "Eqp.img",
        vec![with_children(
            node("Eqp"),
            vec![with_children(
                node("Cap"),
                vec![with_children(
                    node("777"),
                    vec![node_val("name", NodeValue::Str(Arc::from("장비")))],
                )],
            )],
        )],
    );

    let mut linker = crate::linker::StringLinker::new();
    assert!(linker.load_string_wz(&a, root));
    // item > eqp > mob
    assert_eq!(
        linker.get_by_id(777).map(|r| r.name.as_str()),
        Some("아이템")
    );

    // item 항목만 지우면 eqp 가 나온다.
    linker.string_item.remove(&777);
    assert_eq!(linker.get_by_id(777).map(|r| r.name.as_str()), Some("장비"));
    linker.string_eqp.remove(&777);
    assert_eq!(linker.get_by_id(777).map(|r| r.name.as_str()), Some("몹"));
    assert!(linker.get_by_id(1).is_none());
}

#[test]
fn string_linker_clear_empties_every_table_and_has_values_follows_csharp() {
    let mut a = Arena::with_root("");
    let root = string_root_direct(&a);
    add_img(
        &mut a,
        root,
        "Familiar.img",
        vec![with_children(
            node("skill"),
            vec![with_children(
                node("1"),
                vec![node_val("name", NodeValue::Str(Arc::from("x")))],
            )],
        )],
    );

    let mut linker = crate::linker::StringLinker::new();
    // `HasValues` 는 familiarSkill 을 세지 않는다 (`StringLinker.cs:736-743`).
    assert!(!linker.load_string_wz(&a, root));
    assert!(!linker.has_values());
    assert!(linker.string_familiar_skill.contains_key(&1));

    add_img(
        &mut a,
        root,
        "Consume.img",
        vec![with_children(
            node("2"),
            vec![node_val("name", NodeValue::Str(Arc::from("x")))],
        )],
    );
    assert!(linker.load_string_wz(&a, root));
    assert!(linker.has_values());
    linker.clear();
    assert!(!linker.has_values());
    assert!(linker.string_item.is_empty());
    assert!(linker.string_familiar_skill.is_empty());
}

#[test]
fn string_linker_format_tooltip_is_name_newline_desc() {
    let mut a = Arena::with_root("");
    let root = string_root_direct(&a);
    add_img(
        &mut a,
        root,
        "Consume.img",
        vec![
            with_children(
                node("1"),
                vec![
                    node_val("name", NodeValue::Str(Arc::from("설명 있음"))),
                    node_val("desc", NodeValue::Str(Arc::from("본문"))),
                ],
            ),
            with_children(
                node("2"),
                vec![node_val("name", NodeValue::Str(Arc::from("설명 없음")))],
            ),
            with_children(
                node("3"),
                vec![
                    node_val("name", NodeValue::Str(Arc::from("빈 설명"))),
                    node_val("desc", NodeValue::Str(Arc::from(""))),
                ],
            ),
        ],
    );

    let mut linker = crate::linker::StringLinker::new();
    assert!(linker.load_string_wz(&a, root));
    assert_eq!(linker.format_tooltip(1).as_deref(), Some("설명 있음\n본문"));
    assert_eq!(linker.format_tooltip(2).as_deref(), Some("설명 없음"));
    assert_eq!(linker.format_tooltip(3).as_deref(), Some("빈 설명"));
    // 없는 ID 는 `None` — 오라클 CLI 가 이걸로 실패해야 한다.
    assert_eq!(linker.format_tooltip(999), None);
}

#[test]
fn string_linker_resolves_duplicate_name_to_the_last_entry() {
    // 알려진 버그 #9 — 문자열 인덱스는 **마지막** 중복을 고른다.
    let mut a = Arena::with_root("");
    let root = string_root_direct(&a);
    add_img(
        &mut a,
        root,
        "Consume.img",
        vec![with_children(
            node("2000000"),
            vec![
                node_val("name", NodeValue::Str(Arc::from("첫 번째"))),
                node_val("name", NodeValue::Str(Arc::from("마지막"))),
            ],
        )],
    );

    let mut linker = crate::linker::StringLinker::new();
    assert!(linker.load_string_wz(&a, root));
    assert_eq!(linker.string_item[&2000000].name, "마지막");
}

#[test]
fn string_linker_resolves_parent_relative_uol_across_ids() {
    // 알려진 버그 #13 — UOL 경로는 UOL 노드의 **부모 기준** 상대 경로다.
    // `Eqp.img/Eqp/Body/1000001` 의 `../Cap/1000000` 은 `Body` 의 **형제**인
    // `Cap` 을 거쳐야 한다. 이미지 루트에서 바로 `..` 로 나가면 이미지 밖이다.
    let mut a = Arena::with_root("");
    let root = string_root_direct(&a);
    add_img(
        &mut a,
        root,
        "Eqp.img",
        vec![with_children(
            node("Eqp"),
            vec![
                with_children(
                    node("Cap"),
                    vec![with_children(
                        node("1000000"),
                        vec![
                            node_val("name", NodeValue::Str(Arc::from("가죽 모자"))),
                            node_val("desc", NodeValue::Str(Arc::from("머리를 가린다."))),
                        ],
                    )],
                ),
                with_children(
                    node("Body"),
                    vec![node_val(
                        "1000001",
                        NodeValue::Uol(Arc::from("../Cap/1000000")),
                    )],
                ),
            ],
        )],
    );

    let mut linker = crate::linker::StringLinker::new();
    assert!(linker.load_string_wz(&a, root));
    assert_eq!(linker.string_eqp[&1000001].name, "가죽 모자");
    assert_eq!(
        linker.string_eqp[&1000001].desc.as_deref(),
        Some("머리를 가린다.")
    );
}

#[test]
fn quest_loader_reads_questdata_name_desc_and_demand_base() {
    let mut a = Arena::with_root("");
    let data = a.add_node(a.root(), "QuestData", None);
    let entry = a.add_node(data, "0000000.img", None);
    a.ingest_image(
        entry,
        &with_children(
            node("0000000.img"),
            vec![with_children(
                node("QuestInfo"),
                vec![
                    node_val("name", NodeValue::Str(Arc::from("퀘스트"))),
                    node_val("0", NodeValue::Str(Arc::from("설명"))),
                    with_children(
                        node("demand"),
                        vec![node_val("base", NodeValue::Str(Arc::from("기본")))],
                    ),
                ],
            )],
        ),
    );

    let mut linker = crate::linker::StringLinker::new();
    assert!(linker.load_quest_wz(&a, a.root()));
    let sr = linker.string_quest.get(&0).expect("quest 0");
    assert_eq!(sr.name, "퀘스트");
    assert_eq!(sr.desc.as_deref(), Some("설명"));
    assert_eq!(sr.quest_demand_base.as_deref(), Some("기본"));
    assert!(sr.full_path.contains("QuestData"), "{}", sr.full_path);
}

#[test]
fn reactor_loader_prefers_view_name() {
    let mut a = Arena::with_root("");
    let entry = a.add_node(a.root(), "0000000.img", None);
    a.ingest_image(
        entry,
        &with_children(
            node("0000000.img"),
            vec![with_children(
                node("info"),
                vec![node_val("viewName", NodeValue::Str(Arc::from("리액터")))],
            )],
        ),
    );

    let mut linker = crate::linker::StringLinker::new();
    assert!(linker.load_reactor_wz(&a, a.root()));
    let sr = linker.string_reactor.get(&0).expect("reactor 0");
    assert_eq!(sr.name, "리액터");
    assert!(sr.full_path.contains("Reactor"), "{}", sr.full_path);
}

#[test]
fn load_roots_loads_optional_quest_root_in_one_call() {
    let mut a = Arena::with_root("");
    let string = a.add_node(a.root(), "String", None);
    let quest = a.add_node(a.root(), "Quest", None);
    let data = a.add_node(quest, "QuestData", None);
    let entry = a.add_node(data, "0000000.img", None);
    a.ingest_image(
        entry,
        &with_children(
            node("0000000.img"),
            vec![with_children(
                node("QuestInfo"),
                vec![node_val("name", NodeValue::Str(Arc::from("퀘스트")))],
            )],
        ),
    );

    let roots = crate::linker::StringRoots {
        string: crate::linker::StringRoot {
            arena: &a,
            node: string,
        },
        item: None,
        etc: None,
        quest: Some(crate::linker::StringRoot {
            arena: &a,
            node: quest,
        }),
        reactor: None,
    };
    let mut linker = crate::linker::StringLinker::new();
    assert!(linker.load_roots(&roots));
    assert_eq!(
        linker.string_quest.get(&0).map(|s| s.name.as_str()),
        Some("퀘스트")
    );
}

#[test]
fn domain_specific_lookup_disambiguates_colliding_ids() {
    let mut a = Arena::with_root("");
    let root = string_root_direct(&a);
    add_img(
        &mut a,
        root,
        "Consume.img",
        vec![with_children(
            node("100"),
            vec![node_val("name", NodeValue::Str(Arc::from("아이템")))],
        )],
    );
    add_img(
        &mut a,
        root,
        "Mob.img",
        vec![with_children(
            node("100"),
            vec![node_val("name", NodeValue::Str(Arc::from("몹")))],
        )],
    );

    let mut linker = crate::linker::StringLinker::new();
    assert!(linker.load_string_wz(&a, root));

    use crate::linker::StringDomain;
    assert_eq!(linker.get(StringDomain::Item, 100).unwrap().name, "아이템");
    assert_eq!(linker.get(StringDomain::Mob, 100).unwrap().name, "몹");
    // ADR-0006: get_by_id is a convenience; item precedes mob.
    assert_eq!(linker.get_by_id(100).unwrap().name, "아이템");
    assert_eq!(
        linker.format_tooltip_in(StringDomain::Mob, 100).as_deref(),
        Some("몹")
    );
}

#[test]
fn map_scene_particles_real_wz() {
    use crate::MapData;
    use wz_core::WzArchive;

    let path = std::env::var_os("WZ_DATA_ROOT")
        .map(std::path::PathBuf::from)
        .map(|root| {
            root.join("Map")
                .join("Map")
                .join("Map0")
                .join("Map0_000.wz")
        });
    let Some(path) = path else { return };
    if !path.exists() {
        return;
    }
    let arch = WzArchive::open(&path).unwrap();
    let tree = arch.read_tree().unwrap();
    let entry = tree
        .root
        .entries
        .iter()
        .find(|e| e.name.as_deref() == Some("000110000.img"))
        .expect("000110000.img");
    let node = arch.parse_image_at(entry.offset.unwrap()).unwrap();
    let map = MapData::parse_from_node(&node);
    assert_eq!(map.particles.len(), 13);
    let fog = map
        .particles
        .iter()
        .find(|p| p.name == "adventure2022_tree_burn_00_Fog2")
        .expect("fog particle");
    assert_eq!(fog.points.len(), 4);
    assert_eq!(fog.rx, -100);
    assert_eq!(fog.ry, -100);
    assert_eq!(fog.points[0], (-825, -167));
}

#[test]
fn map_scene_events_real_wz() {
    use crate::MapData;
    use wz_core::WzArchive;

    let path = std::env::var_os("WZ_DATA_ROOT")
        .map(std::path::PathBuf::from)
        .map(|root| {
            root.join("Map")
                .join("Map")
                .join("Map1")
                .join("Map1_000.wz")
        });
    let Some(path) = path else { return };
    if !path.exists() {
        return;
    }
    let arch = WzArchive::open(&path).unwrap();
    let tree = arch.read_tree().unwrap();
    let entry = tree
        .root
        .entries
        .iter()
        .find(|e| e.name.as_deref() == Some("100000000.img"))
        .expect("100000000.img");
    let node = arch.parse_image_at(entry.offset.unwrap()).unwrap();
    let map = MapData::parse_from_node(&node);
    assert!(!map.map_events.is_empty());
    let event = map
        .map_events
        .iter()
        .find(|e| e.index == "01")
        .expect("effect 01");
    assert_eq!(
        event.event_type.as_deref(),
        Some("SetAnimationOnceAndReturn")
    );
    assert_eq!(event.default_animation.as_deref(), Some("01_NB"));
    assert_eq!(event.changed_animation.as_deref(), Some("01_BB"));
    assert_eq!(event.tags.as_deref(), Some("Hband1"));
}

#[test]
fn particle_desc_real_wz() {
    use crate::particle::parse_particle_desc;
    use wz_core::WzArchive;

    let path = std::env::var_os("WZ_DATA_ROOT")
        .map(std::path::PathBuf::from)
        .map(|root| root.join("Effect").join("Effect_001.wz"));
    let Some(path) = path else { return };
    if !path.exists() {
        return;
    }
    let arch = WzArchive::open(&path).unwrap();
    let tree = arch.read_tree().unwrap();
    let entry = tree
        .root
        .entries
        .iter()
        .find(|e| e.name.as_deref() == Some("particle.img"))
        .expect("particle.img");
    let node = arch.parse_image_at(entry.offset.unwrap()).unwrap();
    let fog = node
        .object_children()
        .iter()
        .find(|c| c.name.as_ref() == "adventure2022_tree_burn_00_Fog1")
        .expect("Fog1 particle");

    let desc = parse_particle_desc(fog);
    assert_eq!(desc.data_type, 0);
    assert_eq!(desc.total_particle, 60);
    assert_eq!(desc.angle, 90.0);
    assert_eq!(desc.start_size, 300.0);
    assert_eq!(desc.end_size, 500.0);
    assert_eq!(desc.life, 7.0);
    assert_eq!(desc.start_color, 0x007F7F7F);
    assert_eq!(desc.blend_func_src, 5);
    assert_eq!(desc.blend_func_dst, 6);
    assert_eq!(desc.gravity.as_ref().map(|g| g.speed), Some(45.0));
    assert_eq!(
        desc.texture.outlink.as_deref(),
        Some("Effect/_Canvas/particle.img/adventure2022_tree_burn_00_Fog1/texture")
    );
    assert_eq!(desc.texture.origin, Some((350, 350)));
}

#[test]
fn map_display_names_real_wz() {
    use crate::linker::StringLinker;
    use crate::Arena;
    use wz_core::WzArchive;

    let path = std::env::var_os("WZ_DATA_ROOT")
        .map(std::path::PathBuf::from)
        .map(|root| root.join("String").join("String_000.wz"));
    let Some(path) = path else { return };
    if !path.exists() {
        return;
    }
    let arch = WzArchive::open(&path).unwrap();
    let (arena, root) = Arena::from_archive(&arch);
    let mut linker = StringLinker::new();
    assert!(linker.load_string_wz(&arena, root));
    let mut named = 0;
    for id in [100000000, 200000000, 10000] {
        if let Some((street, name)) = linker.map_display_names(id) {
            if !street.is_empty() || !name.is_empty() {
                named += 1;
            }
        }
    }
    assert!(named > 0, "at least one map should have street/name");
}
