use logos::Logos;

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[ \t\n\f]+")] // Skip whitespace
#[logos(skip r"//[^\n]*")]   // Skip normal comments
pub enum TokenKind<'a> {
    // Declarations
    #[token("let")] Let,
    #[token("var")] Var,
    #[token("const")] Const,
    #[token("fn")] Fn,
    #[token("struct")] Struct,
    #[token("class")] Class,
    #[token("trait")] Trait,
    #[token("enum")] Enum,
    #[token("type")] Type,

    // Control Flow
    #[token("if")] If,
    #[token("else")] Else,
    #[token("while")] While,
    #[token("for")] For,
    #[token("loop")] Loop,
    #[token("match")] Match,
    #[token("break")] Break,
    #[token("continue")] Continue,
    #[token("return")] Return,

    // Memory / Type
    #[token("is")] Is,
    #[token("as")] As,
    #[token("with")] With,
    #[token("extends")] Extends,

    // Modules
    #[token("import")] Import,

    // Errors
    #[token("throw")] Throw,
    #[token("try")] Try,
    #[token("catch")] Catch,

    // Concurrency
    #[token("async")] Async,
    #[token("await")] Await,

    // FFI
    #[token("extern")] Extern,
    #[token("unsafe")] Unsafe,

    // Visibility
    #[token("private")] Private,

    // Identifiers
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| lex.slice())]
    Ident(&'a str),

    // Literals
    #[regex(r"[0-9]+\.[0-9]+", |lex| lex.slice())]
    Float(&'a str),

    #[regex(r"[0-9]+", |lex| lex.slice())]
    Int(&'a str),

    #[regex(r#""([^"\\]|\\.)*""#, |lex| lex.slice())]
    String(&'a str),

    // Doc Comments
    #[regex(r"///[^\n]*", |lex| lex.slice())]
    DocComment(&'a str),

    // Operators & Punctuation
    #[token("(")] LParen,
    #[token(")")] RParen,
    #[token("{")] LBrace,
    #[token("}")] RBrace,
    #[token("[")] LBracket,
    #[token("]")] RBracket,
    #[token(",")] Comma,
    #[token(".")] Dot,
    #[token(":")] Colon,
    #[token(";")] Semi,
    
    #[token("=")] Eq,
    #[token("==")] EqEq,
    #[token("!=")] NotEq,
    #[token("<")] Lt,
    #[token("<=")] LtEq,
    #[token(">")] Gt,
    #[token(">=")] GtEq,
    
    #[token("+")] Plus,
    #[token("-")] Minus,
    #[token("*")] Star,
    #[token("/")] Slash,
    #[token("%")] Percent,
    
    #[token("&&")] AndAnd,
    #[token("||")] OrOr,
    #[token("!")] Bang,
    
    #[token("??")] NullCoalesce,
    #[token("?.")] OptChain,
    #[token("?")] Question,
    
    #[token("=>")] FatArrow,
    #[token("->")] Arrow,
}
