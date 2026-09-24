use crate::ast::{BinaryOp, Equation, Expr, ExprKind, Span, UnaryOp};
use crate::lexer::{Lexer, Token, TokenKind};

#[derive(Debug, Clone, PartialEq)]
pub struct ParseError {
    pub message: String,
    pub span: Span,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} at {}:{}",
            self.message, self.span.start, self.span.end
        )
    }
}

impl std::error::Error for ParseError {}

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    eof_token: Token,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Precedence {
    Lowest = 0,
    Or = 1,
    And = 2,
    Comparison = 3,
    Term = 4,   // + -
    Factor = 5, // * / %
    Power = 6,  // ^
    Unary = 7,  // - not
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            pos: 0,
            eof_token: Token {
                kind: TokenKind::Eof,
                span: Span::new(0, 0),
            },
        }
    }

    pub fn from_source(source: &str) -> Result<Self, ParseError> {
        let lexer = Lexer::new(source);
        let tokens = lexer.tokenize_all().map_err(|msg| ParseError {
            message: msg,
            span: Span::new(0, source.len()),
        })?;
        Ok(Self::new(tokens))
    }

    fn peek(&self) -> &Token {
        self.tokens.get(self.pos).unwrap_or(&self.eof_token)
    }

    fn advance(&mut self) -> Token {
        if self.pos < self.tokens.len() {
            let tok = self.tokens[self.pos].clone();
            self.pos += 1;
            tok
        } else {
            Token {
                kind: TokenKind::Eof,
                span: Span::new(0, 0),
            }
        }
    }

    fn check(&self, kind: &TokenKind) -> bool {
        &self.peek().kind == kind
    }

    fn match_token(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: &TokenKind, err_msg: &str) -> Result<Token, ParseError> {
        let peeked = self.peek().clone();
        if &peeked.kind == kind {
            Ok(self.advance())
        } else {
            Err(ParseError {
                message: format!("{err_msg}, found {:?}", peeked.kind),
                span: peeked.span,
            })
        }
    }

    pub fn parse_equation(&mut self) -> Result<Equation, ParseError> {
        let start_span = self.peek().span;

        // Target variable name
        let target_var = match self.peek().kind.clone() {
            TokenKind::Ident(name) => {
                self.advance();
                name
            }
            _ => {
                return Err(ParseError {
                    message: "Expected equation target variable name".to_string(),
                    span: self.peek().span,
                })
            }
        };

        // Optional [n+1] or [n] or [n+k]
        let mut target_offset = 1; // default to n+1 for recurrence
        if self.match_token(&TokenKind::LBracket) {
            target_offset = self.parse_offset_inside_bracket()?;
            self.expect(&TokenKind::RBracket, "Expected ']' after index")?;
        }

        self.expect(&TokenKind::Eq, "Expected '=' in equation")?;

        let rhs = self.parse_expr(Precedence::Lowest)?;
        let span = start_span.merge(rhs.span);

        Ok(Equation {
            target_var,
            target_offset,
            rhs,
            span,
        })
    }

    fn parse_offset_inside_bracket(&mut self) -> Result<i64, ParseError> {
        let tok = self.peek().clone();
        match tok.kind {
            TokenKind::Ident(ref s) if s == "n" => {
                self.advance();
                if self.match_token(&TokenKind::Plus) {
                    let num_tok = self.advance();
                    match num_tok.kind {
                        TokenKind::Number { value, .. } => Ok(value as i64),
                        _ => Err(ParseError {
                            message: "Expected integer after '+' in index".to_string(),
                            span: num_tok.span,
                        }),
                    }
                } else if self.match_token(&TokenKind::Minus) {
                    let num_tok = self.advance();
                    match num_tok.kind {
                        TokenKind::Number { value, .. } => Ok(-(value as i64)),
                        _ => Err(ParseError {
                            message: "Expected integer after '-' in index".to_string(),
                            span: num_tok.span,
                        }),
                    }
                } else {
                    Ok(0)
                }
            }
            TokenKind::Number { value, .. } => {
                self.advance();
                Ok(value as i64)
            }
            _ => Err(ParseError {
                message: "Expected 'n' or integer offset inside brackets".to_string(),
                span: tok.span,
            }),
        }
    }

    pub fn parse_expression(&mut self) -> Result<Expr, ParseError> {
        let expr = self.parse_expr(Precedence::Lowest)?;
        if !self.check(&TokenKind::Eof) {
            return Err(ParseError {
                message: format!("Unexpected trailing token: {:?}", self.peek().kind),
                span: self.peek().span,
            });
        }
        Ok(expr)
    }

    fn parse_expr(&mut self, prec: Precedence) -> Result<Expr, ParseError> {
        let mut left = self.parse_prefix()?;

        while prec < self.peek_precedence() {
            left = self.parse_infix(left)?;
        }

        Ok(left)
    }

    fn peek_precedence(&self) -> Precedence {
        match &self.peek().kind {
            TokenKind::Or => Precedence::Or,
            TokenKind::And => Precedence::And,
            TokenKind::EqEq
            | TokenKind::NotEq
            | TokenKind::Lt
            | TokenKind::LtEq
            | TokenKind::Gt
            | TokenKind::GtEq => Precedence::Comparison,
            TokenKind::Plus | TokenKind::Minus => Precedence::Term,
            TokenKind::Star | TokenKind::Slash | TokenKind::Percent => Precedence::Factor,
            TokenKind::Caret => Precedence::Power,
            _ => Precedence::Lowest,
        }
    }

    fn parse_prefix(&mut self) -> Result<Expr, ParseError> {
        let tok = self.advance();
        match tok.kind {
            TokenKind::Number { value, .. } => Ok(Expr::new(ExprKind::Literal(value), tok.span)),
            TokenKind::Minus => {
                let right = self.parse_expr(Precedence::Unary)?;
                let span = tok.span.merge(right.span);
                Ok(Expr::new(
                    ExprKind::Unary {
                        op: UnaryOp::Neg,
                        expr: Box::new(right),
                    },
                    span,
                ))
            }
            TokenKind::Not => {
                let right = self.parse_expr(Precedence::Unary)?;
                let span = tok.span.merge(right.span);
                Ok(Expr::new(
                    ExprKind::Unary {
                        op: UnaryOp::Not,
                        expr: Box::new(right),
                    },
                    span,
                ))
            }
            TokenKind::LParen => {
                let inner = self.parse_expr(Precedence::Lowest)?;
                let closing = self.expect(&TokenKind::RParen, "Expected ')'")?;
                let span = tok.span.merge(closing.span);
                Ok(Expr::new(inner.kind, span))
            }
            TokenKind::Cond => {
                self.expect(&TokenKind::LParen, "Expected '(' after cond")?;
                let cond_expr = self.parse_expr(Precedence::Lowest)?;
                self.expect(&TokenKind::Comma, "Expected ',' after condition in cond()")?;
                let true_expr = self.parse_expr(Precedence::Lowest)?;
                self.expect(&TokenKind::Comma, "Expected ',' after true value in cond()")?;
                let false_expr = self.parse_expr(Precedence::Lowest)?;
                let closing = self.expect(&TokenKind::RParen, "Expected ')' closing cond()")?;
                let span = tok.span.merge(closing.span);
                Ok(Expr::new(
                    ExprKind::Cond {
                        condition: Box::new(cond_expr),
                        true_val: Box::new(true_expr),
                        false_val: Box::new(false_expr),
                    },
                    span,
                ))
            }
            TokenKind::Ident(name) => {
                // Check if followed by '[' -> StateRef
                if self.match_token(&TokenKind::LBracket) {
                    let offset = self.parse_offset_inside_bracket()?;
                    let closing = self.expect(&TokenKind::RBracket, "Expected ']'")?;
                    let span = tok.span.merge(closing.span);
                    Ok(Expr::new(
                        ExprKind::StateRef {
                            variable: name,
                            offset,
                        },
                        span,
                    ))
                }
                // Check if followed by '(' -> Function call
                else if self.match_token(&TokenKind::LParen) {
                    let mut args = Vec::new();
                    if !self.check(&TokenKind::RParen) {
                        loop {
                            args.push(self.parse_expr(Precedence::Lowest)?);
                            if !self.match_token(&TokenKind::Comma) {
                                break;
                            }
                        }
                    }
                    let closing = self.expect(&TokenKind::RParen, "Expected ')'")?;
                    let span = tok.span.merge(closing.span);

                    if name == "cond" && args.len() == 3 {
                        let false_val = args.pop().unwrap();
                        let true_val = args.pop().unwrap();
                        let condition = args.pop().unwrap();
                        Ok(Expr::new(
                            ExprKind::Cond {
                                condition: Box::new(condition),
                                true_val: Box::new(true_val),
                                false_val: Box::new(false_val),
                            },
                            span,
                        ))
                    } else {
                        Ok(Expr::new(ExprKind::Call { func: name, args }, span))
                    }
                }
                // Bare identifier -> Parameter or variable
                else {
                    Ok(Expr::new(ExprKind::Parameter(name), tok.span))
                }
            }
            _ => Err(ParseError {
                message: format!("Unexpected token in expression: {:?}", tok.kind),
                span: tok.span,
            }),
        }
    }

    fn parse_infix(&mut self, left: Expr) -> Result<Expr, ParseError> {
        let tok = self.advance();
        let (op, next_prec) = match tok.kind {
            TokenKind::Plus => (BinaryOp::Add, Precedence::Term),
            TokenKind::Minus => (BinaryOp::Sub, Precedence::Term),
            TokenKind::Star => (BinaryOp::Mul, Precedence::Factor),
            TokenKind::Slash => (BinaryOp::Div, Precedence::Factor),
            TokenKind::Percent => (BinaryOp::Rem, Precedence::Factor),
            // Power is right-associative, so pass lower precedence to right side
            TokenKind::Caret => (BinaryOp::Pow, Precedence::Factor),
            TokenKind::EqEq => (BinaryOp::Eq, Precedence::Comparison),
            TokenKind::NotEq => (BinaryOp::Ne, Precedence::Comparison),
            TokenKind::Lt => (BinaryOp::Lt, Precedence::Comparison),
            TokenKind::LtEq => (BinaryOp::Le, Precedence::Comparison),
            TokenKind::Gt => (BinaryOp::Gt, Precedence::Comparison),
            TokenKind::GtEq => (BinaryOp::Ge, Precedence::Comparison),
            TokenKind::And => (BinaryOp::And, Precedence::And),
            TokenKind::Or => (BinaryOp::Or, Precedence::Or),
            _ => {
                return Err(ParseError {
                    message: format!("Unexpected infix operator: {:?}", tok.kind),
                    span: tok.span,
                })
            }
        };

        let right = self.parse_expr(next_prec)?;
        let span = left.span.merge(right.span);
        Ok(Expr::new(
            ExprKind::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            },
            span,
        ))
    }
}

pub fn parse_expression_str(source: &str) -> Result<Expr, ParseError> {
    let mut parser = Parser::from_source(source)?;
    parser.parse_expression()
}

pub fn parse_equation_str(source: &str) -> Result<Equation, ParseError> {
    let mut parser = Parser::from_source(source)?;
    let eq = parser.parse_equation()?;
    if !parser.check(&TokenKind::Eof) {
        return Err(ParseError {
            message: format!("Unexpected trailing tokens: {:?}", parser.peek().kind),
            span: parser.peek().span,
        });
    }
    Ok(eq)
}
