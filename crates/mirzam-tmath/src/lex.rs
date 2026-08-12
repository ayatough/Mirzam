//! Tokens, with the byte range each one came from.

use crate::ast::Span;
use crate::{err_at, Error};

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Tok {
    pub kind: TokKind,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum TokKind {
    /// A number, possibly with a decimal point.
    Num(String),
    /// A run of letters, dotted-variant names included: `subset.eq` is one
    /// word. Read as anything else it would silently render as the base
    /// symbol, a stray dot and some letters.
    Word(String),
    /// `"literal text"`, without the quotes.
    Str(String),
    /// `#c`: the character `c`, stripped of any meaning it had.
    Esc(char),
    /// A multi-character operator: its spelling and its LaTeX.
    Sym {
        src: &'static str,
        latex: &'static str,
    },
    /// Any other single character: `+`, `(`, `/`, `^`, …
    Ch(char),
}

impl TokKind {
    pub fn describe(&self) -> String {
        match self {
            TokKind::Num(n) => n.clone(),
            TokKind::Word(w) => w.clone(),
            TokKind::Str(_) => "\"…\"".into(),
            TokKind::Esc(c) => format!("#{c}"),
            TokKind::Sym { src, .. } => src.to_string(),
            TokKind::Ch(c) => c.to_string(),
        }
    }
}

pub(crate) fn lex(src: &str) -> Result<Vec<Tok>, Error> {
    let mut tokens = Vec::new();
    let mut chars = src.char_indices().peekable();
    let end = src.len();
    while let Some(&(at, c)) = chars.peek() {
        match c {
            _ if c.is_whitespace() => {
                chars.next();
            }
            '"' => {
                chars.next();
                let mut s = String::new();
                loop {
                    match chars.next() {
                        Some((close, '"')) => {
                            tokens.push(Tok {
                                kind: TokKind::Str(s),
                                span: Span {
                                    start: at,
                                    end: close + 1,
                                },
                            });
                            break;
                        }
                        Some((_, c)) => s.push(c),
                        None => return err_at("unterminated string literal", at),
                    }
                }
            }
            '#' => {
                chars.next();
                match chars.next() {
                    Some((i, c)) => tokens.push(Tok {
                        kind: TokKind::Esc(c),
                        span: Span {
                            start: at,
                            end: i + c.len_utf8(),
                        },
                    }),
                    None => return err_at("`#` needs a character to escape", at),
                }
            }
            _ if c.is_ascii_digit() => {
                let mut n = String::new();
                while let Some(&(_, d)) = chars.peek() {
                    if d.is_ascii_digit() {
                        n.push(d);
                        chars.next();
                    } else {
                        break;
                    }
                }
                // A decimal point, but not a trailing dot (`1.` is `1` `.`).
                if chars.peek().map(|&(_, d)| d) == Some('.') {
                    let mut ahead = chars.clone();
                    ahead.next();
                    if ahead.peek().is_some_and(|&(_, d)| d.is_ascii_digit()) {
                        n.push('.');
                        chars.next();
                        while let Some(&(_, d)) = chars.peek() {
                            if d.is_ascii_digit() {
                                n.push(d);
                                chars.next();
                            } else {
                                break;
                            }
                        }
                    }
                }
                let stop = chars.peek().map(|&(i, _)| i).unwrap_or(end);
                tokens.push(Tok {
                    kind: TokKind::Num(n),
                    span: Span {
                        start: at,
                        end: stop,
                    },
                });
            }
            _ if c.is_alphabetic() => {
                let mut w = String::new();
                loop {
                    while let Some(&(_, l)) = chars.peek() {
                        if l.is_alphabetic() {
                            w.push(l);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    // A dotted variant name — `subset.eq`, `dots.c` — is one
                    // word.
                    if chars.peek().map(|&(_, d)| d) == Some('.') {
                        let mut ahead = chars.clone();
                        ahead.next();
                        if ahead.peek().is_some_and(|&(_, l)| l.is_alphabetic()) {
                            w.push('.');
                            chars.next();
                            continue;
                        }
                    }
                    break;
                }
                let stop = chars.peek().map(|&(i, _)| i).unwrap_or(end);
                tokens.push(Tok {
                    kind: TokKind::Word(w),
                    span: Span {
                        start: at,
                        end: stop,
                    },
                });
            }
            '-' | '=' | '<' | '>' | '!' | '.' | '|' => {
                // Multi-character operators, longest first.
                let rest: String = chars.clone().map(|(_, c)| c).take(3).collect();
                let sym = |src, latex| Some(TokKind::Sym { src, latex });
                let (kind, len) = if rest.starts_with("...") {
                    (sym("...", "\\dots"), 3)
                } else if rest.starts_with("|->") {
                    (sym("|->", "\\mapsto"), 3)
                } else if rest.starts_with("->") {
                    (sym("->", "\\to"), 2)
                } else if rest.starts_with("=>") {
                    (sym("=>", "\\Rightarrow"), 2)
                } else if rest.starts_with("!=") {
                    (sym("!=", "\\neq"), 2)
                } else if rest.starts_with("<=") {
                    (sym("<=", "\\leq"), 2)
                } else if rest.starts_with(">=") {
                    (sym(">=", "\\geq"), 2)
                } else if rest.starts_with("<-") {
                    (sym("<-", "\\leftarrow"), 2)
                } else if rest.starts_with("<<") {
                    (sym("<<", "\\ll"), 2)
                } else if rest.starts_with(">>") {
                    (sym(">>", "\\gg"), 2)
                } else {
                    (None, 1)
                };
                for _ in 0..len {
                    chars.next();
                }
                let stop = chars.peek().map(|&(i, _)| i).unwrap_or(end);
                tokens.push(Tok {
                    kind: kind.unwrap_or(TokKind::Ch(c)),
                    span: Span {
                        start: at,
                        end: stop,
                    },
                });
            }
            '+' | '*' | '/' | '^' | '_' | '&' | '\\' | '(' | ')' | '[' | ']' | ',' | ';' | '\''
            | ':' | '?' | '%' | '@' | '~' => {
                chars.next();
                tokens.push(Tok {
                    kind: TokKind::Ch(c),
                    span: Span {
                        start: at,
                        end: at + c.len_utf8(),
                    },
                });
            }
            _ => return err_at(format!("unexpected character `{c}`"), at),
        }
    }
    Ok(tokens)
}
