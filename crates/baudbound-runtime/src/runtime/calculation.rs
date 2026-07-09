#[derive(Debug, Clone, PartialEq)]
enum CalculationToken {
    Comma,
    Identifier(String),
    Number(f64),
    Operator(char),
    Paren(char),
}

pub(crate) fn evaluate_calculation_expression(expression: &str) -> Result<f64, String> {
    let tokens = tokenize_calculation_expression(expression)?;
    let mut parser = CalculationParser { index: 0, tokens };
    let value = parser.parse_expression()?;
    if !parser.is_complete() {
        return Err("expression contains trailing tokens".to_owned());
    }
    if value.is_finite() {
        Ok(value)
    } else {
        Err("expression result must be finite".to_owned())
    }
}

fn tokenize_calculation_expression(expression: &str) -> Result<Vec<CalculationToken>, String> {
    let chars = expression.chars().collect::<Vec<_>>();
    let mut tokens = Vec::new();
    let mut index = 0;

    while index < chars.len() {
        let character = chars[index];
        if character.is_whitespace() {
            index += 1;
            continue;
        }

        if character == '(' || character == ')' {
            tokens.push(CalculationToken::Paren(character));
            index += 1;
            continue;
        }

        if character == ',' {
            tokens.push(CalculationToken::Comma);
            index += 1;
            continue;
        }

        if matches!(character, '+' | '-' | '*' | '/' | '%' | '^') {
            tokens.push(CalculationToken::Operator(character));
            index += 1;
            continue;
        }

        if character.is_ascii_digit() || character == '.' {
            let start = index;
            index += 1;
            while index < chars.len()
                && (chars[index].is_ascii_digit()
                    || chars[index] == '.'
                    || chars[index] == 'e'
                    || chars[index] == 'E'
                    || ((chars[index] == '+' || chars[index] == '-')
                        && matches!(chars.get(index.wrapping_sub(1)), Some('e' | 'E'))))
            {
                index += 1;
            }
            let raw = chars[start..index].iter().collect::<String>();
            let value = raw
                .parse::<f64>()
                .map_err(|_| format!("invalid number \"{raw}\""))?;
            if !value.is_finite() {
                return Err(format!("invalid number \"{raw}\""));
            }
            tokens.push(CalculationToken::Number(value));
            continue;
        }

        if character.is_ascii_alphabetic() || character == '_' {
            let start = index;
            index += 1;
            while index < chars.len()
                && (chars[index].is_ascii_alphanumeric() || chars[index] == '_')
            {
                index += 1;
            }
            tokens.push(CalculationToken::Identifier(
                chars[start..index]
                    .iter()
                    .collect::<String>()
                    .to_lowercase(),
            ));
            continue;
        }

        return Err(format!("unexpected token \"{character}\""));
    }

    if tokens.is_empty() {
        Err("expression is required".to_owned())
    } else {
        Ok(tokens)
    }
}

struct CalculationParser {
    index: usize,
    tokens: Vec<CalculationToken>,
}

impl CalculationParser {
    fn is_complete(&self) -> bool {
        self.index >= self.tokens.len()
    }

    fn parse_expression(&mut self) -> Result<f64, String> {
        let mut left = self.parse_term()?;
        loop {
            if self.match_operator('+') {
                left += self.parse_term()?;
            } else if self.match_operator('-') {
                left -= self.parse_term()?;
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_term(&mut self) -> Result<f64, String> {
        let mut left = self.parse_unary()?;
        loop {
            if self.match_operator('*') {
                left *= self.parse_unary()?;
            } else if self.match_operator('/') {
                let right = self.parse_unary()?;
                if right == 0.0 {
                    return Err("division by zero is not allowed".to_owned());
                }
                left /= right;
            } else if self.match_operator('%') {
                let right = self.parse_unary()?;
                if right == 0.0 {
                    return Err("division by zero is not allowed".to_owned());
                }
                left %= right;
            } else {
                return Ok(left);
            }
        }
    }

    fn parse_unary(&mut self) -> Result<f64, String> {
        if self.match_operator('-') {
            return Ok(-self.parse_unary()?);
        }
        if self.match_operator('+') {
            return self.parse_unary();
        }
        self.parse_power()
    }

    fn parse_power(&mut self) -> Result<f64, String> {
        let left = self.parse_primary()?;
        if !self.match_operator('^') {
            return Ok(left);
        }

        let right = self.parse_unary()?;
        let value = left.powf(right);
        if value.is_finite() {
            Ok(value)
        } else {
            Err("exponent result must be finite".to_owned())
        }
    }

    fn parse_primary(&mut self) -> Result<f64, String> {
        match self.advance() {
            Some(CalculationToken::Number(value)) => Ok(value),
            Some(CalculationToken::Paren('(')) => {
                let value = self.parse_expression()?;
                if !self.match_paren(')') {
                    return Err("missing closing parenthesis".to_owned());
                }
                Ok(value)
            }
            Some(CalculationToken::Identifier(name)) => self.parse_function_call(name),
            Some(token) => Err(format!("unexpected token {token:?}")),
            None => Err("expression ended unexpectedly".to_owned()),
        }
    }

    fn parse_function_call(&mut self, name: String) -> Result<f64, String> {
        if !self.match_paren('(') {
            return Err(format!(
                "function \"{name}\" must be called with parentheses"
            ));
        }

        let mut args = Vec::new();
        if !self.match_paren(')') {
            loop {
                args.push(self.parse_expression()?);
                if !self.match_comma() {
                    break;
                }
            }

            if !self.match_paren(')') {
                return Err(format!(
                    "function \"{name}\" is missing a closing parenthesis"
                ));
            }
        }

        evaluate_calculation_function(&name, &args)
    }

    fn match_operator(&mut self, operator: char) -> bool {
        if matches!(self.peek(), Some(CalculationToken::Operator(value)) if *value == operator) {
            self.index += 1;
            return true;
        }
        false
    }

    fn match_paren(&mut self, paren: char) -> bool {
        if matches!(self.peek(), Some(CalculationToken::Paren(value)) if *value == paren) {
            self.index += 1;
            return true;
        }
        false
    }

    fn match_comma(&mut self) -> bool {
        if matches!(self.peek(), Some(CalculationToken::Comma)) {
            self.index += 1;
            return true;
        }
        false
    }

    fn advance(&mut self) -> Option<CalculationToken> {
        let token = self.peek().cloned();
        if token.is_some() {
            self.index += 1;
        }
        token
    }

    fn peek(&self) -> Option<&CalculationToken> {
        self.tokens.get(self.index)
    }
}

fn evaluate_calculation_function(name: &str, args: &[f64]) -> Result<f64, String> {
    match name {
        "round" | "floor" | "ceil" => {
            if args.len() != 1 {
                return Err(format!("{name}() expects exactly one argument"));
            }
            let value = args[0];
            Ok(match name {
                "round" => value.round(),
                "floor" => value.floor(),
                _ => value.ceil(),
            })
        }
        "min" | "max" => {
            if args.is_empty() {
                return Err(format!("{name}() expects at least one argument"));
            }
            let mut values = args.iter().copied();
            let first = values.next().expect("args is known to be non-empty");
            Ok(values.fold(first, |current, value| {
                if name == "min" {
                    current.min(value)
                } else {
                    current.max(value)
                }
            }))
        }
        "random" => {
            if args.len() > 2 {
                return Err("random() expects zero, one, or two arguments".to_owned());
            }
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.subsec_nanos() as f64 / 1_000_000_000.0)
                .unwrap_or(0.0);
            Ok(match args {
                [] => seed,
                [max] => seed * max,
                [min, max] => min + seed * (max - min),
                _ => unreachable!("args length is checked above"),
            })
        }
        _ => Err(format!("unknown function \"{name}\"")),
    }
}
