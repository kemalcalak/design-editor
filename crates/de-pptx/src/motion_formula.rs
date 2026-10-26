//! Time-list formulas of a PowerPoint `anim` (`#ppt_x-0.25`,
//! `0-#ppt_w/2`, `#ppt_w*0.5`): numbers, the four `#ppt_x` / `#ppt_y` /
//! `#ppt_w` / `#ppt_h` variables (the shape's centre and size as slide
//! fractions), `+ - * /` and parentheses. Anything else (`sin`, `rand`,
//! other variables) is not evaluated.

#[derive(Debug, Clone, Copy)]
pub struct Shape {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

pub fn eval(formula: &str, shape: Shape) -> Option<f64> {
    let tokens = tokenize(formula)?;
    let mut parser = Parser {
        tokens: &tokens,
        at: 0,
        shape,
    };
    let value = parser.sum()?;
    (parser.at == tokens.len() && value.is_finite()).then_some(value)
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Token<'f> {
    Num(f64),
    Var(&'f str),
    Op(char),
}

fn tokenize(formula: &str) -> Option<Vec<Token<'_>>> {
    let mut tokens = Vec::new();
    let mut rest = formula.trim();
    while let Some(c) = rest.chars().next() {
        if c.is_whitespace() {
            rest = rest.trim_start();
        } else if "+-*/()".contains(c) {
            tokens.push(Token::Op(c));
            rest = &rest[1..];
        } else if c == '#' {
            let end = rest[1..]
                .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
                .map_or(rest.len(), |i| i + 1);
            tokens.push(Token::Var(&rest[1..end]));
            rest = &rest[end..];
        } else if c.is_ascii_digit() || c == '.' {
            let end = rest
                .find(|ch: char| !(ch.is_ascii_digit() || ch == '.'))
                .unwrap_or(rest.len());
            tokens.push(Token::Num(rest[..end].parse().ok()?));
            rest = &rest[end..];
        } else {
            return None;
        }
    }
    Some(tokens)
}

struct Parser<'t, 'f> {
    tokens: &'t [Token<'f>],
    at: usize,
    shape: Shape,
}

impl<'f> Parser<'_, 'f> {
    fn peek(&self) -> Option<Token<'f>> {
        self.tokens.get(self.at).copied()
    }

    fn sum(&mut self) -> Option<f64> {
        let mut value = self.product()?;
        while let Some(Token::Op(op @ ('+' | '-'))) = self.peek() {
            self.at += 1;
            let rhs = self.product()?;
            value = if op == '+' { value + rhs } else { value - rhs };
        }
        Some(value)
    }

    fn product(&mut self) -> Option<f64> {
        let mut value = self.atom()?;
        while let Some(Token::Op(op @ ('*' | '/'))) = self.peek() {
            self.at += 1;
            let rhs = self.atom()?;
            value = if op == '*' { value * rhs } else { value / rhs };
        }
        Some(value)
    }

    fn atom(&mut self) -> Option<f64> {
        let token = self.peek()?;
        self.at += 1;
        match token {
            Token::Num(n) => Some(n),
            Token::Var("ppt_x") => Some(self.shape.x),
            Token::Var("ppt_y") => Some(self.shape.y),
            Token::Var("ppt_w") => Some(self.shape.w),
            Token::Var("ppt_h") => Some(self.shape.h),
            Token::Op('-') => self.atom().map(|v| -v),
            Token::Op('(') => {
                let value = self.sum()?;
                (self.peek() == Some(Token::Op(')'))).then(|| {
                    self.at += 1;
                    value
                })
            }
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formulas_evaluate_against_the_shape() {
        let shape = Shape {
            x: 0.5,
            y: 0.0,
            w: 0.2,
            h: 0.0,
        };
        assert_eq!(eval("#ppt_x-0.25", shape), Some(0.25));
        assert_eq!(eval("0-#ppt_w/2", shape), Some(-0.1));
        assert_eq!(eval("#ppt_w*0.5", shape), Some(0.1));
        assert_eq!(eval("(1+#ppt_w)/2", shape), Some(0.6));
        assert_eq!(eval("-#ppt_x", shape), Some(-0.5));
        assert_eq!(eval("sin(#ppt_x)", shape), None);
        assert_eq!(eval("#ppt_c", shape), None);
    }
}
