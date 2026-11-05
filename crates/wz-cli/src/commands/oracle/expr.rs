pub fn cmd_oracle_expr(expr: &str, args_str: &str) -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<f64> = if args_str.trim().is_empty() {
        Vec::new()
    } else {
        args_str
            .split(',')
            .filter_map(|s| s.trim().parse::<f64>().ok())
            .collect()
    };

    let evaluated = wz_db::evaluate_expression(expr, &args)?;

    let mut doc = serde_json::Map::new();
    doc.insert("schema_version".into(), serde_json::Value::Number(1.into()));
    doc.insert("op".into(), serde_json::Value::String("expr".into()));
    let mut res = serde_json::Map::new();
    res.insert("expr".into(), serde_json::Value::String(expr.to_string()));
    if let Some(num) = serde_json::Number::from_f64(evaluated) {
        res.insert("value".into(), serde_json::Value::Number(num));
    } else {
        res.insert("value".into(), serde_json::Value::Null);
    }
    doc.insert("result".into(), serde_json::Value::Object(res));

    println!("{}", serde_json::to_string_pretty(&doc)?);
    Ok(())
}
