//! 메이플스토리 스킬 수식 계산기 엔진 (`Calculator.cs` 포트).
//!
//! 원본 `WzComparerR2.Common/Calculator.cs` 대응.

use std::collections::HashMap;

/// 수식 평가 오류.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalcError {
    EmptyExpression,
    InvalidToken(String),
    MissingBracket,
    UndefinedIdentifier(String),
    EvaluationError(String),
}

impl std::fmt::Display for CalcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyExpression => write!(f, "수식이 비어 있습니다"),
            Self::InvalidToken(tok) => write!(f, "유효하지 않은 토큰: {tok}"),
            Self::MissingBracket => write!(f, "괄호 쌍이 맞지 않습니다"),
            Self::UndefinedIdentifier(id) => write!(f, "정의되지 않은 식별자: {id}"),
            Self::EvaluationError(msg) => write!(f, "수식 평가 오류: {msg}"),
        }
    }
}

impl std::error::Error for CalcError {}

/// 수학 표현식을 평가한다 (`Calculator.Parse` 대응).
pub fn evaluate_expression(expr: &str, args: &[f64]) -> Result<f64, CalcError> {
    if expr.trim().is_empty() {
        return Ok(0.0);
    }

    let mut vars = HashMap::new();
    if !args.is_empty() {
        vars.insert("x".into(), args[0]);
    }
    if args.len() > 1 {
        vars.insert("y".into(), args[1]);
    }
    if args.len() > 2 {
        vars.insert("z".into(), args[2]);
    }
    if args.len() > 3 {
        vars.insert("w".into(), args[3]);
    }

    evaluate_recursive(expr, &vars)
}

/// 내장 함수 및 연산자를 지원하는 수식 파서
fn evaluate_recursive(expr: &str, vars: &HashMap<String, f64>) -> Result<f64, CalcError> {
    let mut p = Parser::new(expr, vars);
    let val = p.parse_expression()?;
    p.skip_whitespace();
    if p.pos < p.chars.len() {
        return Err(CalcError::InvalidToken(p.chars[p.pos].to_string()));
    }
    Ok(val)
}

struct Parser<'a> {
    chars: Vec<char>,
    pos: usize,
    vars: &'a HashMap<String, f64>,
}

impl<'a> Parser<'a> {
    fn new(expr: &'a str, vars: &'a HashMap<String, f64>) -> Self {
        Self {
            chars: expr.chars().collect(),
            pos: 0,
            vars,
        }
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.chars.len() && (self.chars[self.pos].is_whitespace() || self.chars[self.pos] == '%') {
            self.pos += 1;
        }
    }

    fn parse_expression(&mut self) -> Result<f64, CalcError> {
        let mut left = self.parse_term()?;
        loop {
            self.skip_whitespace();
            if self.pos >= self.chars.len() {
                break;
            }
            match self.chars[self.pos] {
                '+' => {
                    self.pos += 1;
                    left += self.parse_term()?;
                }
                '-' => {
                    self.pos += 1;
                    left -= self.parse_term()?;
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_term(&mut self) -> Result<f64, CalcError> {
        let mut left = self.parse_factor()?;
        loop {
            self.skip_whitespace();
            if self.pos >= self.chars.len() {
                break;
            }
            match self.chars[self.pos] {
                '*' => {
                    self.pos += 1;
                    left *= self.parse_factor()?;
                }
                '/' => {
                    self.pos += 1;
                    let right = self.parse_factor()?;
                    left = if right == 0.0 { 0.0 } else { left / right };
                }
                _ => break,
            }
        }
        Ok(left)
    }

    fn parse_factor(&mut self) -> Result<f64, CalcError> {
        self.skip_whitespace();
        if self.pos >= self.chars.len() {
            return Err(CalcError::EmptyExpression);
        }

        // 단항 연산자
        if self.chars[self.pos] == '+' {
            self.pos += 1;
            return self.parse_factor();
        }
        if self.chars[self.pos] == '-' {
            self.pos += 1;
            return Ok(-self.parse_factor()?);
        }

        // 괄호
        if self.chars[self.pos] == '(' {
            self.pos += 1;
            let val = self.parse_expression()?;
            self.skip_whitespace();
            if self.pos < self.chars.len() && self.chars[self.pos] == ')' {
                self.pos += 1;
                return Ok(val);
            }
            return Err(CalcError::MissingBracket);
        }

        // 숫자
        if self.chars[self.pos].is_ascii_digit() || self.chars[self.pos] == '.' {
            let start = self.pos;
            let mut has_dot = self.chars[self.pos] == '.';
            self.pos += 1;
            while self.pos < self.chars.len() {
                let c = self.chars[self.pos];
                if c.is_ascii_digit() {
                    self.pos += 1;
                } else if c == '.' && !has_dot {
                    has_dot = true;
                    self.pos += 1;
                } else {
                    break;
                }
            }
            let s: String = self.chars[start..self.pos].iter().collect();
            return s
                .parse::<f64>()
                .map_err(|_| CalcError::InvalidToken(s));
        }

        // 식별자 또는 함수 호출
        if self.chars[self.pos].is_alphabetic() || self.chars[self.pos] == '_' {
            let start = self.pos;
            while self.pos < self.chars.len()
                && (self.chars[self.pos].is_alphanumeric() || self.chars[self.pos] == '_')
            {
                self.pos += 1;
            }
            let name: String = self.chars[start..self.pos].iter().collect();
            self.skip_whitespace();

            // 함수 호출 여부 확인 `(`
            if self.pos < self.chars.len() && self.chars[self.pos] == '(' {
                self.pos += 1;
                let mut args = Vec::new();
                self.skip_whitespace();
                if self.pos < self.chars.len() && self.chars[self.pos] != ')' {
                    args.push(self.parse_expression()?);
                    self.skip_whitespace();
                    while self.pos < self.chars.len() && self.chars[self.pos] == ',' {
                        self.pos += 1;
                        args.push(self.parse_expression()?);
                        self.skip_whitespace();
                    }
                }
                if self.pos < self.chars.len() && self.chars[self.pos] == ')' {
                    self.pos += 1;
                } else {
                    return Err(CalcError::MissingBracket);
                }

                return self.call_function(&name, &args);
            }

            // 변수 조회
            let lower = name.to_ascii_lowercase();
            if let Some(&val) = self.vars.get(&lower).or_else(|| self.vars.get(&name)) {
                return Ok(val);
            }
            return Err(CalcError::UndefinedIdentifier(name));
        }

        Err(CalcError::InvalidToken(self.chars[self.pos].to_string()))
    }

    fn call_function(&self, name: &str, args: &[f64]) -> Result<f64, CalcError> {
        let lower = name.to_ascii_lowercase();
        match lower.as_str() {
            "u" => {
                let x = *args.first().unwrap_or(&0.0);
                Ok(x.ceil())
            }
            "d" => {
                let x = *args.first().unwrap_or(&0.0);
                Ok(x.floor())
            }
            "min" => {
                if args.is_empty() {
                    Ok(0.0)
                } else {
                    let mut min_val = args[0];
                    for &a in &args[1..] {
                        if a < min_val {
                            min_val = a;
                        }
                    }
                    Ok(min_val)
                }
            }
            "max" => {
                if args.is_empty() {
                    Ok(0.0)
                } else {
                    let mut max_val = args[0];
                    for &a in &args[1..] {
                        if a > max_val {
                            max_val = a;
                        }
                    }
                    Ok(max_val)
                }
            }
            _ if lower.starts_with("log") => {
                let base_str = &lower[3..];
                let base: f64 = base_str.parse().unwrap_or(10.0);
                let x = *args.first().unwrap_or(&0.0);
                if x <= 0.0 || base <= 1.0 {
                    Ok(0.0)
                } else {
                    Ok((x.ln() / base.ln()).floor())
                }
            }
            _ => Err(CalcError::UndefinedIdentifier(name.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculator_arithmetic_and_precedence() {
        assert_eq!(evaluate_expression("1 + 2 * 3", &[]).unwrap(), 7.0);
        assert_eq!(evaluate_expression("(1 + 2) * 3", &[]).unwrap(), 9.0);
        assert_eq!(evaluate_expression("-5 + 10", &[]).unwrap(), 5.0);
    }

    #[test]
    fn test_calculator_variables_and_builtins() {
        // x = 20
        assert_eq!(evaluate_expression("x * 2 + 10", &[20.0]).unwrap(), 50.0);
        // u(x/3) -> ceil(20/3) = 7
        assert_eq!(evaluate_expression("u(x / 3)", &[20.0]).unwrap(), 7.0);
        // d(x/3) -> floor(20/3) = 6
        assert_eq!(evaluate_expression("d(x / 3)", &[20.0]).unwrap(), 6.0);
        // min(x, 15) -> min(20, 15) = 15
        assert_eq!(evaluate_expression("min(x, 15)", &[20.0]).unwrap(), 15.0);
        // log10(100) -> 2
        assert_eq!(evaluate_expression("log10(100)", &[]).unwrap(), 2.0);
    }
}
