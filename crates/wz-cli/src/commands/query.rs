


pub fn infer_value_type(pat: &str) -> wz_db::QueryValueType {
    if pat.parse::<f64>().is_ok() {
        wz_db::QueryValueType::Number
    } else if pat.starts_with('(') && pat.ends_with(')') {
        wz_db::QueryValueType::Vector
    } else {
        wz_db::QueryValueType::String
    }
}
pub fn parse_prop_filter(spec: &str) -> Option<wz_db::ValueRule> {
    use wz_db::{NumericComparisonMode, QueryValueType, StringComparisonMode, ValueRule};

    let ops: [(&str, NumericComparisonMode); 6] = [
        ("<=", NumericComparisonMode::LessOrEqual),
        (">=", NumericComparisonMode::GreaterOrEqual),
        ("!=", NumericComparisonMode::NotEqual),
        ("==", NumericComparisonMode::Equal),
        ("<", NumericComparisonMode::LessThan),
        (">", NumericComparisonMode::GreaterThan),
    ];
    for (op, num_cmp) in ops {
        if let Some((path, pat)) = spec.split_once(op) {
            let pat = pat.trim();
            let path = path.trim();
            let value_type = infer_value_type(pat);
            return match value_type {
                QueryValueType::Number => Some(ValueRule::with_numeric(path, num_cmp, pat)),
                QueryValueType::String => {
                    let str_cmp = if num_cmp == NumericComparisonMode::Equal {
                        StringComparisonMode::Equals
                    } else {
                        StringComparisonMode::Contains
                    };
                    Some(ValueRule::with_string(path, str_cmp, pat))
                }
                QueryValueType::Vector => {
                    Some(ValueRule::with_vector(path, num_cmp == NumericComparisonMode::Equal, pat))
                }
            };
        }
    }
    if let Some((path, pat)) = spec.split_once('=') {
        let pat = pat.trim();
        let path = path.trim();
        let value_type = infer_value_type(pat);
        return match value_type {
            QueryValueType::Number => {
                Some(ValueRule::with_numeric(path, NumericComparisonMode::Equal, pat))
            }
            QueryValueType::String => {
                Some(ValueRule::with_string(path, StringComparisonMode::Equals, pat))
            }
            QueryValueType::Vector => Some(ValueRule::with_vector(path, true, pat)),
        };
    }
    None
}
pub fn cmd_query(
    wz_path: &std::path::Path,
    name_pattern: &str,
    props: &[String],
    json: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    use wz_core::WzArchive;
    use wz_db::{execute_query, QueryRule, StringComparisonMode, DEFAULT_MAX_RESULTS};

    let archive = WzArchive::open(wz_path)?;
    let (arena, _) = wz_db::Arena::from_archive(&archive);

    let mut rule = QueryRule::target(StringComparisonMode::Contains, name_pattern);
    rule.search_descendants = true;
    for p in props {
        if let Some(vr) = parse_prop_filter(p) {
            rule.values.push(vr);
        }
    }

    let exec = execute_query(&arena, arena.root(), &[rule], DEFAULT_MAX_RESULTS);
    if json {
        let items: Vec<serde_json::Value> = exec
            .results
            .iter()
            .map(|r| {
                serde_json::json!({
                    "path": r.path,
                    "value": r.value,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&items)?);
    } else {
        println!("=== 질의 검색 결과 ({}건) ===", exec.results.len());
        for r in &exec.results {
            if r.value.is_empty() {
                println!("  {}", r.path);
            } else {
                println!("  {} -> {}", r.path, r.value);
            }
        }
    }
    Ok(())
}
