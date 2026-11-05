use serde_json::Value;

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
    res.insert("value".into(), json_number(evaluated));
    doc.insert("result".into(), serde_json::Value::Object(res));

    println!("{}", serde_json::to_string_pretty(&doc)?);
    Ok(())
}

/// `f64` 계산 결과를 C# `decimal → double` 표기와 맞춘다.
///
/// 정수값은 `i64` JSON 숫자로, 소수는 `f64` 숫자로, NaN/Infinity는 `null`로
/// 직렬화한다. 상한은 `i64::MAX as f64`(= 2^63)가 아니라 **2^63 미만**이다.
/// `i64::MAX`는 f64로 정확히 표현되지 않으므로 `<=`를 쓰면 2^63이 i64로
/// 포화되어 C#의 `9223372036854775808`과 갈린다.
fn json_number(value: f64) -> Value {
    const I64_MIN_F: f64 = -9_223_372_036_854_775_808.0;
    const I64_MAX_EXCLUSIVE_F: f64 = 9_223_372_036_854_775_808.0;

    if value.is_finite()
        && value.fract() == 0.0
        && (I64_MIN_F..I64_MAX_EXCLUSIVE_F).contains(&value)
    {
        Value::Number((value as i64).into())
    } else if let Some(num) = serde_json::Number::from_f64(value) {
        Value::Number(num)
    } else {
        Value::Null
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_is_emitted_without_fraction() {
        assert_eq!(json_number(2.0).to_string(), "2");
        assert_eq!(json_number(-7.0).to_string(), "-7");
    }

    #[test]
    fn fraction_is_preserved() {
        assert_eq!(json_number(3.5).to_string(), "3.5");
    }

    #[test]
    fn i64_min_is_emitted_exactly() {
        assert_eq!(
            json_number(-9_223_372_036_854_775_808.0).to_string(),
            "-9223372036854775808"
        );
    }

    #[test]
    fn two_to_the_63_is_not_saturated_to_i64_max() {
        // 2^63 is exactly representable as f64 but one past i64::MAX.
        // It must stay a float value, not become 9223372036854775807.
        let v = json_number(9_223_372_036_854_775_808.0);
        assert!(
            v.as_i64().is_none(),
            "2^63 must not be truncated to i64: {v}"
        );
    }

    #[test]
    fn nan_becomes_null() {
        assert_eq!(json_number(f64::NAN), Value::Null);
    }
}
