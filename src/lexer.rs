use crate::ast::Span;
use logos::Logos;

#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"[ \t\r\n\f]+")]
pub enum RawToken {
    #[regex(r"([0-9]+(\.[0-9]+)?|\.[0-9]+)([eE][+-]?[0-9]+)?")]
    Number,

    #[token("and")]
    #[token("&&")]
    And,

    #[token("or")]
    #[token("||")]
    Or,

    #[token("not")]
    #[token("!")]
    Not,

    #[token("cond")]
    Cond,

    #[token("+")]
    Plus,
    #[token("-")]
    Minus,
    #[token("*")]
    Star,
    #[token("/")]
    Slash,
    #[token("%")]
    Percent,
    #[token("^")]
    Caret,

    #[token("==")]
    EqEq,
    #[token("!=")]
    NotEq,
    #[token("<=")]
    LtEq,
    #[token(">=")]
    GtEq,
    #[token("<")]
    Lt,
    #[token(">")]
    Gt,
    #[token("=")]
    Eq,

    #[token("(")]
    LParen,
    #[token(")")]
    RParen,
    #[token("[")]
    LBracket,
    #[token("]")]
    RBracket,
    #[token(",")]
    Comma,

    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*")]
    Ident,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    Number { value: f64, raw: String },
    Ident(String),
    And,
    Or,
    Not,
    Cond,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Caret,
    Eq,
    EqEq,
    NotEq,
    Lt,
    LtEq,
    Gt,
    GtEq,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Comma,
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

pub struct Lexer<'a> {
    source: &'a str,
    tokens: Vec<Token>,
    idx: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(source: &'a str) -> Self {
        let mut logos_lexer = RawToken::lexer(source);
        let mut tokens = Vec::new();

        while let Some(res) = logos_lexer.next() {
            let span_range = logos_lexer.span();
            let span = Span::new(span_range.start, span_range.end);
            let slice = logos_lexer.slice();

            let kind = match res {
                Ok(raw) => match raw {
                    RawToken::Number => {
                        let value = slice.parse::<f64>().unwrap_or(0.0);
                        TokenKind::Number {
                            value,
                            raw: slice.to_string(),
                        }
                    }
                    RawToken::Ident => TokenKind::Ident(slice.to_string()),
                    RawToken::And => TokenKind::And,
                    RawToken::Or => TokenKind::Or,
                    RawToken::Not => TokenKind::Not,
                    RawToken::Cond => TokenKind::Cond,
                    RawToken::Plus => TokenKind::Plus,
                    RawToken::Minus => TokenKind::Minus,
                    RawToken::Star => TokenKind::Star,
                    RawToken::Slash => TokenKind::Slash,
                    RawToken::Percent => TokenKind::Percent,
                    RawToken::Caret => TokenKind::Caret,
                    RawToken::Eq => TokenKind::Eq,
                    RawToken::EqEq => TokenKind::EqEq,
                    RawToken::NotEq => TokenKind::NotEq,
                    RawToken::Lt => TokenKind::Lt,
                    RawToken::LtEq => TokenKind::LtEq,
                    RawToken::Gt => TokenKind::Gt,
                    RawToken::GtEq => TokenKind::GtEq,
                    RawToken::LParen => TokenKind::LParen,
                    RawToken::RParen => TokenKind::RParen,
                    RawToken::LBracket => TokenKind::LBracket,
                    RawToken::RBracket => TokenKind::RBracket,
                    RawToken::Comma => TokenKind::Comma,
                },
                Err(_) => {
                    // Unknown character token
                    continue;
                }
            };
            tokens.push(Token::new(kind, span));
        }

        tokens.push(Token::new(
            TokenKind::Eof,
            Span::new(source.len(), source.len()),
        ));

        Self {
            source,
            tokens,
            idx: 0,
        }
    }

    pub fn next_token(&mut self) -> Result<Token, String> {
        if self.idx < self.tokens.len() {
            let tok = self.tokens[self.idx].clone();
            self.idx += 1;
            Ok(tok)
        } else {
            Ok(Token::new(
                TokenKind::Eof,
                Span::new(self.source.len(), self.source.len()),
            ))
        }
    }

    pub fn tokenize_all(self) -> Result<Vec<Token>, String> {
        Ok(self.tokens)
    }
}
