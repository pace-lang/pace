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
    Struct,
    Interface,
    Extend,
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
    Export,
    Private,
    Enum,
    Match,
    As,
    Async,
    Await,
    Actor,
    Spawn,
    True,
    False,
    Weak,
    Null,
    Foreign,
    Static,

    // Identifiers and Literals
    Identifier(session::Symbol),
    Integer(i64),
    Float(f64),
    StringStart,
    StringPart(session::Symbol),
    StringEnd,
    InterpolationStart,
    InterpolationEnd,

    // Punctuation
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
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
    DotDot,
    Colon,
    Semicolon,
    Underscore,
    Arrow,    // ->
    FatArrow, // =>
    Question,
    QuestionDot,           // ?.
    QuestionQuestion,      // ??
    QuestionQuestionEqual, // ??=
    Bang,                  // !
    AndAnd,                // &&
    OrOr,                  // ||
    Ampersand,             // &
    Pipe,                  // |
    Caret,                 // ^
    Tilde,                 // ~
    LessLess,              // <<
    GreaterGreater,        // >>
    PlusEqual,             // +=
    MinusEqual,            // -=
    StarEqual,             // *=
    SlashEqual,            // /=
    PercentEqual,          // %=
    AmpersandEqual,        // &=
    PipeEqual,             // |=
    CaretEqual,            // ^=
    LessLessEqual,         // <<=
    GreaterGreaterEqual,   // >>=

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
