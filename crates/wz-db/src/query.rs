//! WZ 노드 질의(Query) 엔진 (`WzQueryControl.cs:761-1229` 이식).
//!
//! `Arena` 트리 위에서 노드 이름 패턴(시작/포함/끝/일치)과 하위 경로 값 규칙
//! (숫자 비교 `==, !=, <, >, <=, >=`, 문자열 비교, 벡터 `(x, y)` 비교, 제외 규칙, 자손 재귀 검색)을 평가한다.

use crate::arena::Arena;
use crate::node::{NodeId, NodeValue};

/// 기본 최대 검색 결과 개수 (`WzQueryControl.cs` `MaxResultCount`).
pub const DEFAULT_MAX_RESULTS: usize = 5000;

/// 질의 규칙 종류 (`QueryRuleKind`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryRuleKind {
    Target,
    Exclude,
}

/// 값 비교 대상 타입 (`QueryValueType`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryValueType {
    Number,
    String,
    Vector,
}

/// 문자열 비교 방식 (`StringMatcher.IsMatch`, 대소문자 무시).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StringComparisonMode {
    /// 0: 접두사 일치 (`StartsWith`)
    StartsWith = 0,
    /// 1: 부분 문자열 포함 (`Contains`)
    Contains = 1,
    /// 2: 접미사 일치 (`EndsWith`)
    EndsWith = 2,
    /// 3: 전체 일치 (`Equals`)
    Equals = 3,
}

impl StringComparisonMode {
    pub fn from_index(idx: u8) -> Self {
        match idx {
            0 => Self::StartsWith,
            1 => Self::Contains,
            2 => Self::EndsWith,
            _ => Self::Equals,
        }
    }

    pub fn is_match(self, target: &str, pattern: &str) -> bool {
        if pattern.is_empty() {
            return true;
        }
        let t = target.to_ascii_lowercase();
        let p = pattern.to_ascii_lowercase();
        match self {
            Self::StartsWith => t.starts_with(&p),
            Self::Contains => t.contains(&p),
            Self::EndsWith => t.ends_with(&p),
            Self::Equals => t == p,
        }
    }
}

/// 숫자 비교 방식 (`NumericMatcher.IsMatch`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumericComparisonMode {
    /// 0: `==`
    Equal = 0,
    /// 1: `!=`
    NotEqual = 1,
    /// 2: `<`
    LessThan = 2,
    /// 3: `>`
    GreaterThan = 3,
    /// 4: `<=`
    LessOrEqual = 4,
    /// 5: `>=`
    GreaterOrEqual = 5,
}

impl NumericComparisonMode {
    pub fn from_index(idx: u8) -> Self {
        match idx {
            0 => Self::Equal,
            1 => Self::NotEqual,
            2 => Self::LessThan,
            3 => Self::GreaterThan,
            4 => Self::LessOrEqual,
            _ => Self::GreaterOrEqual,
        }
    }

    pub fn is_match(self, actual: f64, expected: f64) -> bool {
        match self {
            Self::Equal => (actual - expected).abs() <= f64::EPSILON,
            Self::NotEqual => (actual - expected).abs() > f64::EPSILON,
            Self::LessThan => actual < expected,
            Self::GreaterThan => actual > expected,
            Self::LessOrEqual => actual <= expected,
            Self::GreaterOrEqual => actual >= expected,
        }
    }
}

/// 노드의 스칼라 값을 문자열 표현으로 변환한다 (`GetValueEx<string>` 대응).
pub fn node_value_to_string(val: Option<&NodeValue>) -> Option<String> {
    match val? {
        NodeValue::Null => None,
        NodeValue::Short(v) => Some(v.to_string()),
        NodeValue::Int(v) => Some(v.to_string()),
        NodeValue::Long(v) => Some(v.to_string()),
        NodeValue::Uint(v) => Some(v.to_string()),
        NodeValue::Float(v) => Some(v.to_string()),
        NodeValue::Double(v) => Some(v.to_string()),
        NodeValue::Bool(v) => Some(v.to_string()),
        NodeValue::Str(s) | NodeValue::Uol(s) => Some(s.to_string()),
        NodeValue::Vector(v) => Some(format!("({}, {})", v.x, v.y)),
        _ => None,
    }
}

fn node_value_to_f64(val: Option<&NodeValue>) -> Option<f64> {
    match val? {
        NodeValue::Short(v) => Some(*v as f64),
        NodeValue::Int(v) => Some(*v as f64),
        NodeValue::Long(v) => Some(*v as f64),
        NodeValue::Uint(v) => Some(*v as f64),
        NodeValue::Float(v) => Some(*v as f64),
        NodeValue::Double(v) => Some(*v),
        NodeValue::Str(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn parse_vector_pattern(pattern: &str) -> Option<(i32, i32)> {
    let trimmed = pattern.trim().trim_matches(|c| c == '(' || c == ')');
    let mut parts = trimmed.split(',');
    let x = parts.next()?.trim().parse::<i32>().ok()?;
    let y = parts.next()?.trim().parse::<i32>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((x, y))
}

/// 하위 경로의 값 조건 규칙 (`ValueRule`).
#[derive(Debug, Clone)]
pub struct ValueRule {
    pub path: String,
    pub value_type: QueryValueType,
    pub comparison: u8,
    pub pattern: String,
    pub output: bool,
    pub required: bool,
}

impl ValueRule {
    pub fn new(
        path: impl Into<String>,
        value_type: QueryValueType,
        comparison: u8,
        pattern: impl Into<String>,
    ) -> Self {
        Self {
            path: path.into(),
            value_type,
            comparison,
            pattern: pattern.into(),
            output: true,
            required: true,
        }
    }

    /// 숫자 조건 규칙 생성
    pub fn with_numeric(
        path: impl Into<String>,
        comparison: NumericComparisonMode,
        pattern: impl Into<String>,
    ) -> Self {
        Self::new(path, QueryValueType::Number, comparison as u8, pattern)
    }

    /// 문자열 조건 규칙 생성
    pub fn with_string(
        path: impl Into<String>,
        comparison: StringComparisonMode,
        pattern: impl Into<String>,
    ) -> Self {
        Self::new(path, QueryValueType::String, comparison as u8, pattern)
    }

    /// 벡터 조건 규칙 생성 (`equal`: true면 ==, false면 !=)
    pub fn with_vector(path: impl Into<String>, equal: bool, pattern: impl Into<String>) -> Self {
        Self::new(
            path,
            QueryValueType::Vector,
            if equal { 0 } else { 1 },
            pattern,
        )
    }

    fn resolve_target(&self, arena: &Arena, node: NodeId) -> Option<NodeId> {
        let mut current = arena.descend_image(node);
        if self.path.is_empty() {
            return Some(current);
        }
        for seg in self.path.split(['\\', '/']).filter(|s| !s.is_empty()) {
            current = arena.find_child(current, seg)?;
        }
        Some(current)
    }

    pub fn is_match(&self, arena: &Arena, node: NodeId) -> bool {
        let Some(value_node) = self.resolve_target(arena, node) else {
            return false;
        };
        let val = arena.value(value_node);
        if self.pattern.is_empty() {
            return node_value_to_string(val).unwrap_or_default().is_empty();
        }

        match self.value_type {
            QueryValueType::Number => {
                let Some(actual) = node_value_to_f64(val) else {
                    return false;
                };
                let Ok(expected) = self.pattern.trim().parse::<f64>() else {
                    return false;
                };
                NumericComparisonMode::from_index(self.comparison).is_match(actual, expected)
            }
            QueryValueType::Vector => {
                let Some(NodeValue::Vector(vec)) = val else {
                    return false;
                };
                let Some((x, y)) = parse_vector_pattern(&self.pattern) else {
                    return false;
                };
                let equals = vec.x == x && vec.y == y;
                if self.comparison == 0 {
                    equals
                } else {
                    !equals
                }
            }
            QueryValueType::String => {
                let text = node_value_to_string(val).unwrap_or_default();
                StringComparisonMode::from_index(self.comparison).is_match(&text, &self.pattern)
            }
        }
    }

    pub fn get_output(&self, arena: &Arena, node: NodeId) -> String {
        let Some(value_node) = self.resolve_target(arena, node) else {
            return String::new();
        };
        let name = arena.name(value_node).unwrap_or_default();
        match arena.value(value_node) {
            Some(NodeValue::Vector(v)) => format!("{name}: ({}, {})", v.x, v.y),
            other => {
                if let Some(s) = node_value_to_string(other) {
                    format!("{name}: {s}")
                } else {
                    name.to_string()
                }
            }
        }
    }
}

/// 노드 질의 규칙 (`QueryRule`).
#[derive(Debug, Clone)]
pub struct QueryRule {
    pub kind: QueryRuleKind,
    pub text_comparison: StringComparisonMode,
    pub text_pattern: String,
    pub output: bool,
    pub search_descendants: bool,
    pub values: Vec<ValueRule>,
    pub children: Vec<QueryRule>,
}

impl QueryRule {
    pub fn target(text_comparison: StringComparisonMode, text_pattern: impl Into<String>) -> Self {
        Self {
            kind: QueryRuleKind::Target,
            text_comparison,
            text_pattern: text_pattern.into(),
            output: false,
            search_descendants: false,
            values: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn exclude(text_comparison: StringComparisonMode, text_pattern: impl Into<String>) -> Self {
        Self {
            kind: QueryRuleKind::Exclude,
            text_comparison,
            text_pattern: text_pattern.into(),
            output: false,
            search_descendants: false,
            values: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn is_text_match(&self, arena: &Arena, node: NodeId) -> bool {
        let name = arena.name(node).unwrap_or_default();
        self.text_comparison.is_match(name, &self.text_pattern)
    }

    pub fn are_values_match(&self, arena: &Arena, node: NodeId) -> bool {
        let required_ok = self
            .values
            .iter()
            .filter(|v| v.required)
            .all(|v| v.is_match(arena, node));
        let mut optional_iter = self.values.iter().filter(|v| !v.required).peekable();
        let optional_ok =
            optional_iter.peek().is_none() || optional_iter.any(|v| v.is_match(arena, node));
        required_ok && optional_ok
    }
}

/// 단일 질의 매칭 결과 (`QueryResult`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryResult {
    pub node_id: NodeId,
    pub path: String,
    pub value: String,
}

/// 질의 실행 전체 결과 (`QueryExecutionResult`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryExecutionResult {
    pub results: Vec<QueryResult>,
    pub limit_reached: bool,
}

fn effective_children(arena: &Arena, node: NodeId) -> &[NodeId] {
    let content = arena.descend_image(node);
    arena.children(content)
}

fn join_path(parent_path: &str, name: &str) -> String {
    if parent_path.is_empty() {
        name.to_string()
    } else {
        format!("{parent_path}/{name}")
    }
}

/// `root` 아래에서 `rules` 질의를 실행한다 (`WzQueryControl.Execute` 대응).
pub fn execute_query(
    arena: &Arena,
    root: NodeId,
    rules: &[QueryRule],
    max_results: usize,
) -> QueryExecutionResult {
    let mut results = Vec::new();
    let root_name = arena.name(root).unwrap_or_default();
    let limit_reached = execute_children(
        arena,
        root,
        root_name,
        &[],
        rules,
        &mut results,
        max_results,
    );
    QueryExecutionResult {
        results,
        limit_reached,
    }
}

fn execute_children(
    arena: &Arena,
    parent: NodeId,
    parent_path: &str,
    parent_outputs: &[String],
    rules: &[QueryRule],
    results: &mut Vec<QueryResult>,
    max_results: usize,
) -> bool {
    let target_rules: Vec<&QueryRule> = rules
        .iter()
        .filter(|r| r.kind == QueryRuleKind::Target)
        .collect();
    let exclude_rules: Vec<&QueryRule> = rules
        .iter()
        .filter(|r| r.kind == QueryRuleKind::Exclude)
        .collect();

    for &child in effective_children(arena, parent) {
        let text_matched_excludes: Vec<&&QueryRule> = exclude_rules
            .iter()
            .filter(|r| r.is_text_match(arena, child))
            .collect();
        if text_matched_excludes.iter().any(|r| r.values.is_empty()) {
            continue;
        }

        let text_matched_targets: Vec<&&QueryRule> = target_rules
            .iter()
            .filter(|r| r.is_text_match(arena, child))
            .collect();
        if !target_rules.is_empty() && text_matched_targets.is_empty() {
            continue;
        }

        if text_matched_excludes
            .iter()
            .any(|r| r.are_values_match(arena, child))
        {
            continue;
        }

        let child_name = arena.name(child).unwrap_or_default();
        let path = join_path(parent_path, child_name);

        if target_rules.is_empty() {
            results.push(QueryResult {
                node_id: child,
                path,
                value: parent_outputs.join(", "),
            });
            if results.len() >= max_results {
                return true;
            }
            continue;
        }

        for &&target in text_matched_targets.iter().filter(|r| r.search_descendants) {
            let mut outputs = parent_outputs.to_vec();
            if target.output {
                outputs.push(child_name.to_string());
            }
            if target.are_values_match(arena, child) {
                let mut cur_outputs = outputs.clone();
                cur_outputs.extend(
                    target
                        .values
                        .iter()
                        .filter(|v| v.output && v.is_match(arena, child))
                        .map(|v| v.get_output(arena, child)),
                );
                results.push(QueryResult {
                    node_id: child,
                    path: path.clone(),
                    value: cur_outputs.join(", "),
                });
                if results.len() >= max_results {
                    return true;
                }
            }
            if execute_descendants(
                arena,
                child,
                &path,
                &outputs,
                target,
                &target.children,
                results,
                max_results,
            ) {
                return true;
            }
        }

        for &&target in text_matched_targets
            .iter()
            .filter(|r| !r.search_descendants && r.are_values_match(arena, child))
        {
            let mut outputs = parent_outputs.to_vec();
            if target.output {
                outputs.push(child_name.to_string());
            }
            outputs.extend(
                target
                    .values
                    .iter()
                    .filter(|v| v.output && v.is_match(arena, child))
                    .map(|v| v.get_output(arena, child)),
            );

            if !target.children.is_empty() {
                if execute_children(
                    arena,
                    child,
                    &path,
                    &outputs,
                    &target.children,
                    results,
                    max_results,
                ) {
                    return true;
                }
            } else {
                results.push(QueryResult {
                    node_id: child,
                    path: path.clone(),
                    value: outputs.join(", "),
                });
                if results.len() >= max_results {
                    return true;
                }
            }
        }
    }

    false
}

#[allow(clippy::too_many_arguments)]
fn execute_descendants(
    arena: &Arena,
    parent: NodeId,
    parent_path: &str,
    parent_outputs: &[String],
    target: &QueryRule,
    rules: &[QueryRule],
    results: &mut Vec<QueryResult>,
    max_results: usize,
) -> bool {
    let exclude_rules: Vec<&QueryRule> = rules
        .iter()
        .filter(|r| r.kind == QueryRuleKind::Exclude)
        .collect();

    for &child in effective_children(arena, parent) {
        let text_matched_excludes: Vec<&&QueryRule> = exclude_rules
            .iter()
            .filter(|r| r.is_text_match(arena, child))
            .collect();
        if text_matched_excludes.iter().any(|r| r.values.is_empty()) {
            continue;
        }
        if text_matched_excludes
            .iter()
            .any(|r| r.are_values_match(arena, child))
        {
            continue;
        }

        let child_name = arena.name(child).unwrap_or_default();
        let path = join_path(parent_path, child_name);

        if target.are_values_match(arena, child) {
            let mut outputs = parent_outputs.to_vec();
            outputs.extend(
                target
                    .values
                    .iter()
                    .filter(|v| v.output && v.is_match(arena, child))
                    .map(|v| v.get_output(arena, child)),
            );
            results.push(QueryResult {
                node_id: child,
                path: path.clone(),
                value: outputs.join(", "),
            });
            if results.len() >= max_results {
                return true;
            }
        }

        if execute_descendants(
            arena,
            child,
            &path,
            parent_outputs,
            target,
            rules,
            results,
            max_results,
        ) {
            return true;
        }
    }

    false
}
