#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Ident(String),
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Let,
    Var,
    Const,
    If,
    Else,
    Return,
    Null,
    Func,
    Class,
    Interface,
    Implement,
    Public,
    Private,
    Async,
    Await,
    Actor,
    Struct,
    Import,
    For,
    In,
    While,
    Loop,
    Match,
    Arrow,
    Comma,
    Colon,
    Dot,
    Eq,
    Semi,
    Plus,
    Minus,
    Star,
    Slash,
    EqEq,
    NotEq,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Eof,
}

pub struct Lexer<'a> {
    src: std::iter::Peekable<std::str::Chars<'a>>,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            src: src.chars().peekable(),
        }
    }

    fn advance(&mut self) -> Option<char> {
        self.src.next()
    }

    fn peek(&mut self) -> Option<&char> {
        self.src.peek()
    }

    fn skip_whitespace(&mut self) {
        while let Some(&c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }
    }

    pub fn next_token(&mut self) -> Token {
        self.skip_whitespace();

        if let Some(&c) = self.peek() {
            if c.is_alphabetic() {
                return self.ident();
            }
            if c.is_ascii_digit() {
                return self.number();
            }
            match c {
                ':' => { self.advance(); Token::Colon }
                '.' => { self.advance(); Token::Dot }
                ';' => { self.advance(); Token::Semi }
                '=' => {
                    self.advance();
                    if self.peek() == Some(&'=') {
                        self.advance();
                        Token::EqEq
                    } else {
                        Token::Eq
                    }
                }
                '!' => {
                    self.advance();
                    if self.peek() == Some(&'=') {
                        self.advance();
                        Token::NotEq
                    } else {
                        Token::Ident("!".to_string())
                    }
                }
                '+' => { self.advance(); Token::Plus }
                '-' => {
                    self.advance();
                    if self.peek() == Some(&'>') {
                        self.advance();
                        Token::Arrow
                    } else {
                        Token::Minus
                    }
                }
                '*' => { self.advance(); Token::Star }
                '/' => { self.advance(); Token::Slash }
                '(' => { self.advance(); Token::LParen }
                ')' => { self.advance(); Token::RParen }
                '{' => { self.advance(); Token::LBrace }
                '}' => { self.advance(); Token::RBrace }
                ',' => { self.advance(); Token::Comma }
                '"' => self.string(),
                _ => {
                    self.advance();
                    Token::Ident(c.to_string())
                }
            }
        } else {
            Token::Eof
        }
    }

    fn ident(&mut self) -> Token {
        let mut s = String::new();
        while let Some(&c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                s.push(self.advance().unwrap());
            } else {
                break;
            }
        }
        match s.as_str() {
            "let" => Token::Let,
            "var" => Token::Var,
            "const" => Token::Const,
            "if" => Token::If,
            "else" => Token::Else,
            "return" => Token::Return,
            "null" => Token::Null,
            "true" => Token::Bool(true),
            "false" => Token::Bool(false),
            "func" => Token::Func,
            "class" => Token::Class,
            "interface" => Token::Interface,
            "implement" => Token::Implement,
            "public" => Token::Public,
            "private" => Token::Private,
            "async" => Token::Async,
            "await" => Token::Await,
            "actor" => Token::Actor,
            "struct" => Token::Struct,
            "import" => Token::Import,
            "for" => Token::For,
            "in" => Token::In,
            "while" => Token::While,
            "loop" => Token::Loop,
            "match" => Token::Match,
            _ => Token::Ident(s),
        }
    }

    fn number(&mut self) -> Token {
        let mut s = String::new();
        let mut is_float = false;
        while let Some(&c) = self.peek() {
            if c.is_ascii_digit() {
                s.push(self.advance().unwrap());
            } else if c == '.' {
                is_float = true;
                s.push(self.advance().unwrap());
            } else {
                break;
            }
        }
        if is_float {
            Token::Float(s.parse().unwrap())
        } else {
            Token::Int(s.parse().unwrap())
        }
    }

    fn string(&mut self) -> Token {
        self.advance(); // skip quote
        let mut s = String::new();
        while let Some(&c) = self.peek() {
            if c != '"' {
                s.push(self.advance().unwrap());
            } else {
                break;
            }
        }
        self.advance(); // skip quote
        Token::String(s)
    }
}
