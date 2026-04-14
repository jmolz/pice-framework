//! Trigger expression grammar — lexer, recursive-descent parser, evaluator.
//!
//! Grammar (shared by `review.trigger` and `layer_overrides.{}.trigger`):
//!
//! ```text
//! or_expr    := and_expr ('OR' and_expr)*
//! and_expr   := not_expr ('AND' not_expr)*
//! not_expr   := 'NOT' not_expr | primary
//! primary    := comparison | '(' or_expr ')' | literal
//! comparison := identifier CMP_OP value
//! CMP_OP     := '==' | '!=' | '>=' | '<=' | '>' | '<'
//! literal    := 'true' | 'false' | 'always'
//! identifier := 'tier' | 'layer' | 'confidence' | 'cost' | 'passes' | 'change_scope'
//! value      := integer | float | string
//! ```
//!
//! Precedence: `NOT` > `AND` > `OR`. Parentheses override.
//!
//! See PRDv2 lines 927–944 and `.claude/rules/workflow-yaml.md` lines 54–72.

use serde::Serialize;
use thiserror::Error;

/// Allowed comparison operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum CmpOp {
    Eq,
    NotEq,
    Ge,
    Le,
    Gt,
    Lt,
}

/// Value on the right-hand side of a comparison.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum TriggerValue {
    Int(i64),
    Float(f64),
    String(String),
}

/// Parsed trigger AST.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub enum TriggerAst {
    And(Box<TriggerAst>, Box<TriggerAst>),
    Or(Box<TriggerAst>, Box<TriggerAst>),
    Not(Box<TriggerAst>),
    Comparison {
        ident: String,
        op: CmpOp,
        value: TriggerValue,
    },
    Literal(bool),
}

/// Evaluator context.
#[derive(Debug, Clone, PartialEq)]
pub struct TriggerContext {
    pub tier: u8,
    pub layer: String,
    pub confidence: f64,
    pub cost: f64,
    pub passes: u32,
    pub change_scope: String,
}

const ALLOWED_IDENTIFIERS: &[&str] = &[
    "tier",
    "layer",
    "confidence",
    "cost",
    "passes",
    "change_scope",
];

// ─── Lexer ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    And,
    Or,
    Not,
    LParen,
    RParen,
    Eq,
    NotEq,
    Ge,
    Le,
    Gt,
    Lt,
    Ident(String),
    IntLit(i64),
    FloatLit(f64),
    StringLit(String),
    True,
    False,
    Always,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TokenWithPos {
    pub token: Token,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Error, Serialize, PartialEq)]
#[error("lex error at line {line}, column {column}: {message}")]
pub struct LexError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

/// Tokenize a trigger expression.
pub fn lex(input: &str) -> Result<Vec<TokenWithPos>, LexError> {
    let mut tokens = Vec::new();
    let mut line = 1usize;
    let mut col = 1usize;
    let mut chars = input.char_indices().peekable();

    while let Some(&(_idx, ch)) = chars.peek() {
        match ch {
            ' ' | '\t' => {
                chars.next();
                col += 1;
            }
            '\n' => {
                chars.next();
                line += 1;
                col = 1;
            }
            '\r' => {
                chars.next();
                col += 1;
            }
            '(' => {
                tokens.push(TokenWithPos {
                    token: Token::LParen,
                    line,
                    column: col,
                });
                chars.next();
                col += 1;
            }
            ')' => {
                tokens.push(TokenWithPos {
                    token: Token::RParen,
                    line,
                    column: col,
                });
                chars.next();
                col += 1;
            }
            '=' => {
                let start_col = col;
                chars.next();
                col += 1;
                match chars.peek() {
                    Some(&(_, '=')) => {
                        chars.next();
                        col += 1;
                        tokens.push(TokenWithPos {
                            token: Token::Eq,
                            line,
                            column: start_col,
                        });
                    }
                    _ => {
                        return Err(LexError {
                            message: "unexpected '='; did you mean '=='?".into(),
                            line,
                            column: start_col,
                        });
                    }
                }
            }
            '!' => {
                let start_col = col;
                chars.next();
                col += 1;
                match chars.peek() {
                    Some(&(_, '=')) => {
                        chars.next();
                        col += 1;
                        tokens.push(TokenWithPos {
                            token: Token::NotEq,
                            line,
                            column: start_col,
                        });
                    }
                    _ => {
                        return Err(LexError {
                            message: "unexpected '!'; did you mean '!='?".into(),
                            line,
                            column: start_col,
                        });
                    }
                }
            }
            '>' => {
                let start_col = col;
                chars.next();
                col += 1;
                if let Some(&(_, '=')) = chars.peek() {
                    chars.next();
                    col += 1;
                    tokens.push(TokenWithPos {
                        token: Token::Ge,
                        line,
                        column: start_col,
                    });
                } else {
                    tokens.push(TokenWithPos {
                        token: Token::Gt,
                        line,
                        column: start_col,
                    });
                }
            }
            '<' => {
                let start_col = col;
                chars.next();
                col += 1;
                if let Some(&(_, '=')) = chars.peek() {
                    chars.next();
                    col += 1;
                    tokens.push(TokenWithPos {
                        token: Token::Le,
                        line,
                        column: start_col,
                    });
                } else {
                    tokens.push(TokenWithPos {
                        token: Token::Lt,
                        line,
                        column: start_col,
                    });
                }
            }
            '"' | '\'' => {
                let quote = ch;
                let start_col = col;
                chars.next();
                col += 1;
                let mut s = String::new();
                let mut terminated = false;
                while let Some(&(_, c)) = chars.peek() {
                    chars.next();
                    col += 1;
                    if c == quote {
                        terminated = true;
                        break;
                    }
                    if c == '\n' {
                        line += 1;
                        col = 1;
                    }
                    s.push(c);
                }
                if !terminated {
                    return Err(LexError {
                        message: "unterminated string literal".into(),
                        line,
                        column: start_col,
                    });
                }
                tokens.push(TokenWithPos {
                    token: Token::StringLit(s),
                    line,
                    column: start_col,
                });
            }
            c if c.is_ascii_digit() => {
                let start_col = col;
                let mut s = String::new();
                let mut saw_dot = false;
                while let Some(&(_, c)) = chars.peek() {
                    if c.is_ascii_digit() || c == '.' {
                        if c == '.' {
                            if saw_dot {
                                break;
                            }
                            saw_dot = true;
                        }
                        s.push(c);
                        chars.next();
                        col += 1;
                    } else {
                        break;
                    }
                }
                if saw_dot {
                    let v: f64 = s.parse().map_err(|_| LexError {
                        message: format!("invalid float literal '{s}'"),
                        line,
                        column: start_col,
                    })?;
                    tokens.push(TokenWithPos {
                        token: Token::FloatLit(v),
                        line,
                        column: start_col,
                    });
                } else {
                    let v: i64 = s.parse().map_err(|_| LexError {
                        message: format!("invalid integer literal '{s}'"),
                        line,
                        column: start_col,
                    })?;
                    tokens.push(TokenWithPos {
                        token: Token::IntLit(v),
                        line,
                        column: start_col,
                    });
                }
            }
            c if c.is_alphabetic() || c == '_' => {
                let start_col = col;
                let mut s = String::new();
                while let Some(&(_, c)) = chars.peek() {
                    if c.is_alphanumeric() || c == '_' {
                        s.push(c);
                        chars.next();
                        col += 1;
                    } else {
                        break;
                    }
                }
                let token = match s.as_str() {
                    "AND" | "and" => Token::And,
                    "OR" | "or" => Token::Or,
                    "NOT" | "not" => Token::Not,
                    "true" => Token::True,
                    "false" => Token::False,
                    "always" => Token::Always,
                    _ => Token::Ident(s),
                };
                tokens.push(TokenWithPos {
                    token,
                    line,
                    column: start_col,
                });
            }
            c => {
                return Err(LexError {
                    message: format!("unexpected character '{c}'"),
                    line,
                    column: col,
                });
            }
        }
    }

    Ok(tokens)
}

// ─── Parser ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Error, Serialize, PartialEq)]
#[error("parse error at line {line}, column {column}: {message}")]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub column: usize,
}

impl From<LexError> for ParseError {
    fn from(e: LexError) -> Self {
        Self {
            message: e.message,
            line: e.line,
            column: e.column,
        }
    }
}

/// Parse a trigger expression into an AST.
pub fn parse(input: &str) -> Result<TriggerAst, ParseError> {
    let tokens = lex(input)?;
    let mut p = Parser {
        tokens: &tokens,
        pos: 0,
        depth: 0,
    };
    let ast = p.parse_or()?;
    if p.pos < tokens.len() {
        let t = &tokens[p.pos];
        return Err(ParseError {
            message: format!("unexpected token after expression: {:?}", t.token),
            line: t.line,
            column: t.column,
        });
    }
    Ok(ast)
}

/// Maximum nesting depth for `(` groups and chained `NOT`. Beyond this, parsing
/// returns a `ParseError` rather than overflowing the Rust stack. Real-world
/// triggers are shallow (< 10 levels); 128 leaves massive headroom while
/// bounding the recursion budget under malformed input.
const MAX_PARSE_DEPTH: usize = 128;

struct Parser<'a> {
    tokens: &'a [TokenWithPos],
    pos: usize,
    /// Current recursion depth across the two descent points — `(` groups in
    /// `parse_primary` and chained `NOT` in `parse_not`. Incremented before
    /// recursing, decremented after. The rest of the grammar is iterative
    /// (while-loops in `parse_or`/`parse_and`) and does not grow the stack.
    depth: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Option<&TokenWithPos> {
        self.tokens.get(self.pos)
    }

    fn bump(&mut self) -> Option<&TokenWithPos> {
        let t = self.tokens.get(self.pos);
        if t.is_some() {
            self.pos += 1;
        }
        t
    }

    fn eof_pos(&self) -> (usize, usize) {
        self.tokens
            .last()
            .map(|t| (t.line, t.column))
            .unwrap_or((1, 1))
    }

    fn err(&self, msg: impl Into<String>) -> ParseError {
        let (line, column) = self
            .peek()
            .map(|t| (t.line, t.column))
            .unwrap_or_else(|| self.eof_pos());
        ParseError {
            message: msg.into(),
            line,
            column,
        }
    }

    fn parse_or(&mut self) -> Result<TriggerAst, ParseError> {
        let mut left = self.parse_and()?;
        while matches!(self.peek().map(|t| &t.token), Some(Token::Or)) {
            self.bump();
            let right = self.parse_and()?;
            left = TriggerAst::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_and(&mut self) -> Result<TriggerAst, ParseError> {
        let mut left = self.parse_not()?;
        while matches!(self.peek().map(|t| &t.token), Some(Token::And)) {
            self.bump();
            let right = self.parse_not()?;
            left = TriggerAst::And(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    fn parse_not(&mut self) -> Result<TriggerAst, ParseError> {
        if matches!(self.peek().map(|t| &t.token), Some(Token::Not)) {
            self.bump();
            self.depth += 1;
            if self.depth > MAX_PARSE_DEPTH {
                self.depth -= 1;
                return Err(self.err(format!(
                    "trigger expression too deeply nested (max depth {MAX_PARSE_DEPTH})"
                )));
            }
            let inner_result = self.parse_not();
            self.depth -= 1;
            Ok(TriggerAst::Not(Box::new(inner_result?)))
        } else {
            self.parse_primary()
        }
    }

    fn parse_primary(&mut self) -> Result<TriggerAst, ParseError> {
        let token = self
            .peek()
            .cloned()
            .ok_or_else(|| self.err("unexpected end of expression"))?;

        match &token.token {
            Token::LParen => {
                self.bump();
                self.depth += 1;
                if self.depth > MAX_PARSE_DEPTH {
                    self.depth -= 1;
                    return Err(self.err(format!(
                        "trigger expression too deeply nested (max depth {MAX_PARSE_DEPTH})"
                    )));
                }
                let inner_result = self.parse_or();
                self.depth -= 1;
                let inner = inner_result?;
                match self.peek().map(|t| &t.token) {
                    Some(Token::RParen) => {
                        self.bump();
                        Ok(inner)
                    }
                    _ => Err(self.err("unclosed parenthesis; expected ')'")),
                }
            }
            Token::True => {
                self.bump();
                Ok(TriggerAst::Literal(true))
            }
            Token::False => {
                self.bump();
                Ok(TriggerAst::Literal(false))
            }
            Token::Always => {
                self.bump();
                Ok(TriggerAst::Literal(true))
            }
            Token::Ident(name) => {
                let name = name.clone();
                if !ALLOWED_IDENTIFIERS.contains(&name.as_str()) {
                    return Err(ParseError {
                        message: format!(
                            "unknown identifier '{name}'; allowed: {}",
                            ALLOWED_IDENTIFIERS.join(", ")
                        ),
                        line: token.line,
                        column: token.column,
                    });
                }
                self.bump();
                let op = self.parse_cmp_op()?;
                let value = self.parse_value()?;
                Ok(TriggerAst::Comparison {
                    ident: name,
                    op,
                    value,
                })
            }
            other => Err(ParseError {
                message: format!("expected identifier, literal, or '('; got {other:?}"),
                line: token.line,
                column: token.column,
            }),
        }
    }

    fn parse_cmp_op(&mut self) -> Result<CmpOp, ParseError> {
        let t = self
            .peek()
            .cloned()
            .ok_or_else(|| self.err("expected comparison operator"))?;
        let op = match t.token {
            Token::Eq => CmpOp::Eq,
            Token::NotEq => CmpOp::NotEq,
            Token::Ge => CmpOp::Ge,
            Token::Le => CmpOp::Le,
            Token::Gt => CmpOp::Gt,
            Token::Lt => CmpOp::Lt,
            ref other => {
                return Err(ParseError {
                    message: format!("expected comparison operator; got {other:?}"),
                    line: t.line,
                    column: t.column,
                });
            }
        };
        self.bump();
        Ok(op)
    }

    fn parse_value(&mut self) -> Result<TriggerValue, ParseError> {
        let t = self
            .peek()
            .cloned()
            .ok_or_else(|| self.err("expected value (int, float, string, or identifier)"))?;
        let v = match t.token {
            Token::IntLit(n) => TriggerValue::Int(n),
            Token::FloatLit(f) => TriggerValue::Float(f),
            Token::StringLit(ref s) => TriggerValue::String(s.clone()),
            // Bare identifiers on the RHS are treated as string literals
            // (e.g., `layer == infrastructure`).
            Token::Ident(ref s) => TriggerValue::String(s.clone()),
            ref other => {
                return Err(ParseError {
                    message: format!("expected value literal; got {other:?}"),
                    line: t.line,
                    column: t.column,
                });
            }
        };
        self.bump();
        Ok(v)
    }
}

// ─── Evaluator ──────────────────────────────────────────────────────────────

/// Evaluate a parsed trigger AST against a context.
pub fn evaluate_ast(ast: &TriggerAst, ctx: &TriggerContext) -> bool {
    match ast {
        TriggerAst::Literal(b) => *b,
        TriggerAst::Not(inner) => !evaluate_ast(inner, ctx),
        TriggerAst::And(a, b) => evaluate_ast(a, ctx) && evaluate_ast(b, ctx),
        TriggerAst::Or(a, b) => evaluate_ast(a, ctx) || evaluate_ast(b, ctx),
        TriggerAst::Comparison { ident, op, value } => eval_comparison(ident, *op, value, ctx),
    }
}

fn eval_comparison(ident: &str, op: CmpOp, value: &TriggerValue, ctx: &TriggerContext) -> bool {
    match ident {
        "tier" | "passes" => {
            let lhs = if ident == "tier" {
                i64::from(ctx.tier)
            } else {
                i64::from(ctx.passes)
            };
            let rhs = match value {
                TriggerValue::Int(n) => *n,
                TriggerValue::Float(f) => *f as i64,
                TriggerValue::String(_) => {
                    tracing::warn!(
                        ident,
                        "string comparison against integer identifier; always false"
                    );
                    return false;
                }
            };
            cmp_num(lhs, rhs, op)
        }
        "confidence" | "cost" => {
            let lhs = if ident == "confidence" {
                ctx.confidence
            } else {
                ctx.cost
            };
            let rhs = match value {
                TriggerValue::Float(f) => *f,
                TriggerValue::Int(n) => *n as f64,
                TriggerValue::String(_) => {
                    tracing::warn!(
                        ident,
                        "string comparison against float identifier; always false"
                    );
                    return false;
                }
            };
            cmp_num(lhs, rhs, op)
        }
        "layer" | "change_scope" => {
            let lhs = if ident == "layer" {
                ctx.layer.as_str()
            } else {
                ctx.change_scope.as_str()
            };
            let rhs = match value {
                TriggerValue::String(s) => s.as_str(),
                _ => {
                    tracing::warn!(
                        ident,
                        "non-string value for string identifier; always false"
                    );
                    return false;
                }
            };
            match op {
                CmpOp::Eq => lhs == rhs,
                CmpOp::NotEq => lhs != rhs,
                _ => {
                    tracing::warn!(
                        ident,
                        "only == and != allowed for string identifier; always false"
                    );
                    false
                }
            }
        }
        _ => {
            tracing::warn!(ident, "unknown identifier in evaluator; always false");
            false
        }
    }
}

fn cmp_num<T: PartialOrd + PartialEq>(lhs: T, rhs: T, op: CmpOp) -> bool {
    match op {
        CmpOp::Eq => lhs == rhs,
        CmpOp::NotEq => lhs != rhs,
        CmpOp::Ge => lhs >= rhs,
        CmpOp::Le => lhs <= rhs,
        CmpOp::Gt => lhs > rhs,
        CmpOp::Lt => lhs < rhs,
    }
}

// ─── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx() -> TriggerContext {
        TriggerContext {
            tier: 2,
            layer: "frontend".into(),
            confidence: 0.90,
            cost: 1.5,
            passes: 3,
            change_scope: "mixed".into(),
        }
    }

    // ── Lexer ────────────────────────────────────────────────────────────

    #[test]
    fn lex_simple_comparison() {
        let toks = lex("tier >= 3").unwrap();
        let kinds: Vec<_> = toks.iter().map(|t| &t.token).cloned().collect();
        assert_eq!(
            kinds,
            vec![Token::Ident("tier".into()), Token::Ge, Token::IntLit(3),]
        );
    }

    #[test]
    fn lex_tracks_line_and_column() {
        let toks = lex("tier\n>= 3").unwrap();
        assert_eq!(toks[0].line, 1);
        assert_eq!(toks[0].column, 1);
        assert_eq!(toks[1].line, 2);
        assert_eq!(toks[1].column, 1);
    }

    #[test]
    fn lex_unknown_char_errors_with_position() {
        let err = lex("tier $ 3").unwrap_err();
        assert_eq!(err.line, 1);
        assert_eq!(err.column, 6);
        assert!(err.message.contains("$"));
    }

    #[test]
    fn lex_single_equals_errors() {
        let err = lex("tier = 3").unwrap_err();
        assert!(err.message.contains("=="));
    }

    #[test]
    fn lex_single_bang_errors() {
        let err = lex("tier ! 3").unwrap_err();
        assert!(err.message.contains("!="));
    }

    #[test]
    fn lex_quoted_string() {
        let toks = lex("change_scope == \"css_only\"").unwrap();
        assert!(matches!(toks[2].token, Token::StringLit(ref s) if s == "css_only"));
    }

    #[test]
    fn lex_float_literal() {
        let toks = lex("confidence < 0.95").unwrap();
        assert!(matches!(toks[2].token, Token::FloatLit(f) if (f - 0.95).abs() < 1e-9));
    }

    // ── Parser — happy path ──────────────────────────────────────────────

    #[test]
    fn parse_tier_ge_3() {
        let ast = parse("tier >= 3").unwrap();
        assert_eq!(
            ast,
            TriggerAst::Comparison {
                ident: "tier".into(),
                op: CmpOp::Ge,
                value: TriggerValue::Int(3),
            }
        );
    }

    #[test]
    fn parse_layer_or_deployment() {
        let ast = parse("layer == infrastructure OR layer == deployment").unwrap();
        assert!(matches!(ast, TriggerAst::Or(_, _)));
    }

    #[test]
    fn parse_confidence_and_tier() {
        let ast = parse("confidence < 0.95 AND tier >= 2").unwrap();
        assert!(matches!(ast, TriggerAst::And(_, _)));
    }

    #[test]
    fn parse_not_with_grouping() {
        let ast = parse("NOT (change_scope == css_only)").unwrap();
        assert!(matches!(ast, TriggerAst::Not(_)));
    }

    #[test]
    fn parse_always_literal() {
        let ast = parse("always").unwrap();
        assert_eq!(ast, TriggerAst::Literal(true));
    }

    #[test]
    fn parse_true_literal() {
        let ast = parse("true").unwrap();
        assert_eq!(ast, TriggerAst::Literal(true));
    }

    #[test]
    fn parse_precedence_and_binds_tighter_than_or() {
        // tier == 1 AND layer == a OR cost == 3 → (tier AND layer) OR cost
        let ast = parse("tier == 1 AND layer == a OR cost == 3").unwrap();
        match ast {
            TriggerAst::Or(left, _right) => {
                assert!(matches!(*left, TriggerAst::And(_, _)));
            }
            _ => panic!("expected Or at top-level"),
        }
    }

    #[test]
    fn parse_parens_override_precedence() {
        let ast = parse("tier == 1 AND (layer == a OR cost == 3)").unwrap();
        match ast {
            TriggerAst::And(_, right) => {
                assert!(matches!(*right, TriggerAst::Or(_, _)));
            }
            _ => panic!("expected And at top-level"),
        }
    }

    // ── Parser — error paths ─────────────────────────────────────────────

    #[test]
    fn parse_unclosed_paren() {
        let err = parse("(tier >= 3").unwrap_err();
        assert!(err.message.contains("unclosed") || err.message.contains("')'"));
        assert!(err.line >= 1);
    }

    #[test]
    fn parse_missing_operand() {
        let err = parse("tier >=").unwrap_err();
        assert!(err.message.contains("value") || err.message.contains("expected"));
    }

    #[test]
    fn parse_unknown_identifier() {
        let err = parse("foo == 1").unwrap_err();
        assert!(err.message.contains("foo"));
        assert!(err.message.contains("allowed"));
        assert_eq!(err.line, 1);
        assert_eq!(err.column, 1);
    }

    #[test]
    fn parse_invalid_comparison_missing_rhs() {
        let err = parse("tier == ==").unwrap_err();
        assert!(err.message.contains("expected"));
    }

    #[test]
    fn parse_error_includes_line_and_column() {
        let err = parse("tier == 3 AND foo == 1").unwrap_err();
        assert!(err.line >= 1);
        assert!(err.column >= 1);
        assert!(err.message.contains("foo"));
    }

    // ── Evaluator ────────────────────────────────────────────────────────

    #[test]
    fn eval_literal_true() {
        assert!(evaluate_ast(&parse("true").unwrap(), &ctx()));
    }

    #[test]
    fn eval_always_is_true() {
        assert!(evaluate_ast(&parse("always").unwrap(), &ctx()));
    }

    #[test]
    fn eval_not_inverts() {
        assert!(!evaluate_ast(&parse("NOT true").unwrap(), &ctx()));
        assert!(evaluate_ast(&parse("NOT false").unwrap(), &ctx()));
    }

    #[test]
    fn eval_tier_ge() {
        assert!(evaluate_ast(&parse("tier >= 2").unwrap(), &ctx()));
        assert!(!evaluate_ast(&parse("tier >= 3").unwrap(), &ctx()));
    }

    #[test]
    fn eval_confidence_lt() {
        assert!(evaluate_ast(&parse("confidence < 0.95").unwrap(), &ctx()));
        assert!(!evaluate_ast(&parse("confidence < 0.5").unwrap(), &ctx()));
    }

    #[test]
    fn eval_layer_equality() {
        assert!(evaluate_ast(&parse("layer == frontend").unwrap(), &ctx()));
        assert!(!evaluate_ast(&parse("layer == backend").unwrap(), &ctx()));
    }

    #[test]
    fn eval_layer_string_ops_only_eq_neq() {
        // `layer > foo` is meaningless; evaluator returns false with warning
        assert!(!evaluate_ast(&parse("layer > backend").unwrap(), &ctx()));
    }

    #[test]
    fn eval_and_or_combination() {
        assert!(evaluate_ast(
            &parse("tier >= 2 AND confidence < 0.95").unwrap(),
            &ctx()
        ));
        assert!(!evaluate_ast(
            &parse("tier >= 3 AND confidence < 0.95").unwrap(),
            &ctx()
        ));
        assert!(evaluate_ast(
            &parse("tier >= 3 OR confidence < 0.95").unwrap(),
            &ctx()
        ));
    }

    #[test]
    fn eval_grouping_semantics() {
        // NOT (layer == frontend) = false (layer IS frontend)
        assert!(!evaluate_ast(
            &parse("NOT (layer == frontend)").unwrap(),
            &ctx()
        ));
    }

    #[test]
    fn deeply_nested_parens_return_parse_error_not_overflow() {
        // Construct a pathological expression of 200 nested `(` groups. Prior
        // to the depth limit this would recurse past the Rust stack and abort
        // the process. With the MAX_PARSE_DEPTH guard, parse returns a
        // ParseError cleanly.
        let depth = 200;
        let expr = format!(
            "{}tier >= 1{}",
            "(".repeat(depth),
            ")".repeat(depth)
        );
        let err = parse(&expr).expect_err("expected depth-limit parse error");
        assert!(
            err.message.contains("too deeply nested"),
            "unexpected error message: {}",
            err.message
        );
    }

    #[test]
    fn deeply_nested_not_returns_parse_error_not_overflow() {
        // 200 chained NOT keywords is well beyond MAX_PARSE_DEPTH. Must return
        // a ParseError, not crash the process.
        let expr = format!("{}tier >= 1", "NOT ".repeat(200));
        let err = parse(&expr).expect_err("expected depth-limit parse error");
        assert!(
            err.message.contains("too deeply nested"),
            "unexpected error message: {}",
            err.message
        );
    }

    #[test]
    fn moderate_nesting_under_limit_parses() {
        // Sanity: 64 levels of parens is well within MAX_PARSE_DEPTH (128).
        let depth = 64;
        let expr = format!(
            "{}tier >= 1{}",
            "(".repeat(depth),
            ")".repeat(depth)
        );
        parse(&expr).expect("64-level nesting should parse cleanly");
    }
}
