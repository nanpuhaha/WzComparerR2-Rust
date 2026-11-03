//! arena/해석 테스트.
//!
//! 함정만 골라 实測한다. 정상 경로는 C# 대조에서 이미 검증되므로 여기서는
//! **원본이 틀리는 경우** 를 고정한다.

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
    a.add_node(
        info1,
        "islot",
        Some(NodeValue::Str(Arc::from("Cp"))),
    );
    a.add_node(
        info1,
        "origin",
        Some(NodeValue::Vector(Vec2 { x: 16, y: 32 })),
    );

    let item2 = a.add_node(a.root(), "01000001.img", None);
    let info2 = a.add_node(item2, "info", None);
    a.add_node(info2, "reqLevel", Some(NodeValue::Int(50)));
    a.add_node(
        info2,
        "islot",
        Some(NodeValue::Str(Arc::from("Cp"))),
    );

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

    let path = "<WZ_DATA_ROOT>/String/String_000.wz";
    if !std::path::Path::new(path).exists() {
        return;
    }
    let arch = WzArchive::open(std::path::Path::new(path)).unwrap();
    let (arena, root) = Arena::from_archive(&arch);
    let mut linker = StringLinker::new();
    assert!(linker.load_string_wz(&arena, root));
    // Should have parsed some eqp, skill, map, mob, npc strings
    assert!(!linker.string_eqp.is_empty(), "eqp should not be empty");
    assert!(!linker.string_skill.is_empty() || !linker.string_skill_str.is_empty(), "skill should not be empty");
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

    let path = "<WZ_DATA_ROOT>/String/String_000.wz";
    if !std::path::Path::new(path).exists() {
        return;
    }
    let arch = WzArchive::open(std::path::Path::new(path)).unwrap();
    let (arena, root) = Arena::from_archive(&arch);
    let mut linker = StringLinker::new();
    assert!(linker.load_string_wz(&arena, root));
    // Should have skill strings
    assert!(!linker.string_skill.is_empty() || !linker.string_skill_str.is_empty(), "skill strings should not be empty");
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
