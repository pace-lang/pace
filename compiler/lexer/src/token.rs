use ast::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Keywords
    Let,
    Var,
    Func,
    Init,
    SelfKeyword,
    Class,
    Interface,
    Implements,
    Type,
    If,
    Else,
    For,
    While,
    Switch,
    Return,
    Break,
    Continue,
    Import,
    Package,
    Async,
    Await,
    True,
    False,
    Weak,

    // Identifiers and Literals
    Identifier(String),
    Integer(i64),
    Float(f64),
    String(String),

    // Punctuation
    Plus,
    Minus,
    Star,
    Slash,
    Equal,
    EqualEqual,
    BangEqual,
    Greater,
    GreaterEqual,
    Less,
    LessEqual,
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Comma,
    Dot,
    Colon,
    Semicolon,
    Arrow, // ->
    FatArrow, // =>
    Question,

    // Special
    Error(String),
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
