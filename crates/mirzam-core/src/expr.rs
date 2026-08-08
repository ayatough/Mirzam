//! `{{ ... }}` 用のミニ式評価器。
//! 対応: 数値/文字列変数、四則演算、括弧、単項マイナス、関数 round/ceil/floor。

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Num(f64),
    Str(String),
}

impl Value {
    pub fn to_display(&self) -> String {
        match self {
            Value::Str(s) => s.clone(),
            Value::Num(n) => {
                if n.fract() == 0.0 && n.abs() < 1e15 {
                    format!("{}", *n as i64)
                } else {
                    // 小数は有効桁を抑えて表示
                    let s = format!("{n:.4}");
                    s.trim_end_matches('0').trim_end_matches('.').to_string()
                }
            }
        }
    }
}

pub fn eval_expr(src: &str, vars: &BTreeMap<String, Value>) -> Result<Value, String> {
    let tokens = tokenize(src)?;
    let mut p = Parser {
        tokens,
        pos: 0,
        vars,
    };
    let v = p.expr()?;
    if p.pos != p.tokens.len() {
        return Err(format!("式の末尾に余分なトークン: {:?}", p.tokens[p.pos]));
    }
    Ok(v)
}

#[derive(Debug, Clone, PartialEq)]
enum Tok {
    Num(f64),
    Ident(String),
    Op(char),
    LParen,
    RParen,
    Comma,
}

fn tokenize(src: &str) -> Result<Vec<Tok>, String> {
    let mut toks = Vec::new();
    let chars: Vec<char> = src.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            ' ' | '\t' | '\n' => i += 1,
            '+' | '-' | '*' | '/' => {
                toks.push(Tok::Op(c));
                i += 1;
            }
            '(' => {
                toks.push(Tok::LParen);
                i += 1;
            }
            ')' => {
                toks.push(Tok::RParen);
                i += 1;
            }
            ',' => {
                toks.push(Tok::Comma);
                i += 1;
            }
            '0'..='9' | '.' => {
                let start = i;
                while i < chars.len()
                    && (chars[i].is_ascii_digit() || chars[i] == '.' || chars[i] == '_')
                {
                    i += 1;
                }
                let s: String = chars[start..i].iter().filter(|&&c| c != '_').collect();
                toks.push(Tok::Num(s.parse().map_err(|e| format!("数値エラー: {e}"))?));
            }
            c if c.is_alphabetic() || c == '_' => {
                let start = i;
                while i < chars.len() && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                toks.push(Tok::Ident(chars[start..i].iter().collect()));
            }
            other => return Err(format!("未対応の文字: {other}")),
        }
    }
    Ok(toks)
}

struct Parser<'a> {
    tokens: Vec<Tok>,
    pos: usize,
    vars: &'a BTreeMap<String, Value>,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&Tok> {
        self.tokens.get(self.pos)
    }

    fn next(&mut self) -> Option<Tok> {
        let t = self.tokens.get(self.pos).cloned();
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn expr(&mut self) -> Result<Value, String> {
        let mut left = self.term()?;
        while let Some(Tok::Op(op @ ('+' | '-'))) = self.peek().cloned() {
            self.pos += 1;
            let right = self.term()?;
            left = binop(op, left, right)?;
        }
        Ok(left)
    }

    fn term(&mut self) -> Result<Value, String> {
        let mut left = self.factor()?;
        while let Some(Tok::Op(op @ ('*' | '/'))) = self.peek().cloned() {
            self.pos += 1;
            let right = self.factor()?;
            left = binop(op, left, right)?;
        }
        Ok(left)
    }

    fn factor(&mut self) -> Result<Value, String> {
        match self.next() {
            Some(Tok::Num(n)) => Ok(Value::Num(n)),
            Some(Tok::Op('-')) => match self.factor()? {
                Value::Num(n) => Ok(Value::Num(-n)),
                Value::Str(_) => Err("文字列に単項マイナスは使えません".into()),
            },
            Some(Tok::LParen) => {
                let v = self.expr()?;
                match self.next() {
                    Some(Tok::RParen) => Ok(v),
                    _ => Err("閉じ括弧がありません".into()),
                }
            }
            Some(Tok::Ident(name)) => {
                if self.peek() == Some(&Tok::LParen) {
                    self.pos += 1;
                    let mut args = Vec::new();
                    if self.peek() != Some(&Tok::RParen) {
                        loop {
                            args.push(self.expr()?);
                            match self.next() {
                                Some(Tok::Comma) => continue,
                                Some(Tok::RParen) => break,
                                _ => return Err("関数呼び出しが閉じていません".into()),
                            }
                        }
                    } else {
                        self.pos += 1;
                    }
                    call(&name, &args)
                } else {
                    self.vars
                        .get(&name)
                        .cloned()
                        .ok_or_else(|| format!("未定義の変数: {name}"))
                }
            }
            other => Err(format!("式のパースに失敗: {other:?}")),
        }
    }
}

fn as_num(v: &Value) -> Result<f64, String> {
    match v {
        Value::Num(n) => Ok(*n),
        Value::Str(s) => s.parse().map_err(|_| format!("数値ではありません: {s}")),
    }
}

fn binop(op: char, l: Value, r: Value) -> Result<Value, String> {
    // 文字列 + 文字列 は連結
    if op == '+' {
        if let (Value::Str(a), b) = (&l, &r) {
            return Ok(Value::Str(format!("{a}{}", b.to_display())));
        }
        if let (a, Value::Str(b)) = (&l, &r) {
            return Ok(Value::Str(format!("{}{b}", a.to_display())));
        }
    }
    let (a, b) = (as_num(&l)?, as_num(&r)?);
    let n = match op {
        '+' => a + b,
        '-' => a - b,
        '*' => a * b,
        '/' => {
            if b == 0.0 {
                return Err("ゼロ除算".into());
            }
            a / b
        }
        _ => unreachable!(),
    };
    Ok(Value::Num(n))
}

fn call(name: &str, args: &[Value]) -> Result<Value, String> {
    let one = |args: &[Value]| -> Result<f64, String> {
        if args.len() != 1 {
            return Err(format!("{name} は引数 1 つを取ります"));
        }
        as_num(&args[0])
    };
    match name {
        "round" => Ok(Value::Num(one(args)?.round())),
        "ceil" => Ok(Value::Num(one(args)?.ceil())),
        "floor" => Ok(Value::Num(one(args)?.floor())),
        _ => Err(format!("未定義の関数: {name}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars() -> BTreeMap<String, Value> {
        let mut m = BTreeMap::new();
        m.insert("x".into(), Value::Num(10.0));
        m.insert("name".into(), Value::Str("Mirzam".into()));
        m
    }

    #[test]
    fn arithmetic() {
        let v = vars();
        assert_eq!(eval_expr("x * 12 + 5", &v).unwrap().to_display(), "125");
        assert_eq!(eval_expr("(x + 2) / 4", &v).unwrap().to_display(), "3");
        assert_eq!(eval_expr("-x + 1", &v).unwrap().to_display(), "-9");
    }

    #[test]
    fn functions() {
        let v = vars();
        assert_eq!(eval_expr("round(x / 3)", &v).unwrap().to_display(), "3");
        assert_eq!(eval_expr("ceil(x / 3)", &v).unwrap().to_display(), "4");
    }

    #[test]
    fn string_concat() {
        let v = vars();
        assert_eq!(eval_expr("name + x", &v).unwrap().to_display(), "Mirzam10");
    }

    #[test]
    fn errors() {
        let v = vars();
        assert!(eval_expr("y + 1", &v).is_err());
        assert!(eval_expr("x /", &v).is_err());
        assert!(eval_expr("x / 0", &v).is_err());
    }
}
