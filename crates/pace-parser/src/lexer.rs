#[derive(Clone, Debug, PartialEq)]
pub enum Token {
    Ident(String),
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    DocComment(String),
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
    Enum,
    Implement,
    Private,
    Async,
    Await,
    Actor,
    Static,
    Struct,
    Import,
    Export,
    From,
    As,
    Show,
    Hide,
    For,
    In,
    While,
    Loop,
    Match,
    Arrow,
    FatArrow,
    Comma,
    Colon,
    ColonColon,
    Dot,
    Eq,
    Semi,
    Plus,
    Minus,
    Star,
    Slash,
    Mod,
    EqEq,
    NotEq,
    Less,
    LessEq,
    Greater,
    GreaterEq,
    AndAnd,
    PipePipe,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Question,
    Bang,
    QuestionDot,
    QuestionQuestion,
    Eof,
}

#[derive(Clone)]
pub struct Lexer<'a> {
    src: &'a str,
    pub byte_pos: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            src,
            byte_pos: 0,
        }
    }

    fn peek(&self) -> Option<char> {
        self.src[self.byte_pos..].chars().next()
    }

    fn advance(&mut self) -> Option<char> {
        if let Some(c) = self.peek() {
            self.byte_pos += c.len_utf8();
            Some(c)
        } else {
            None
        }
    }

    fn skip_whitespace(&mut self) {
        loop {
            let tail = &self.src[self.byte_pos..];
            if tail.is_empty() {
                break;
            }

            if tail.starts_with("///") && !tail.starts_with("////") {
                // Doc comment, stop skipping
                break;
            } else if tail.starts_with("//") {
                if let Some(idx) = tail.find('\n') {
                    self.byte_pos += idx + 1;
                } else {
                    self.byte_pos = self.src.len();
                }
            } else if tail.starts_with("/*") {
                if let Some(idx) = tail.find("*/") {
                    self.byte_pos += idx + 2;
                } else {
                    self.byte_pos = self.src.len();
                }
            } else if let Some(c) = tail.chars().next() {
                if c.is_whitespace() {
                    self.byte_pos += c.len_utf8();
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    pub fn next_token(&mut self) -> (Token, (usize, usize)) {
        self.skip_whitespace();
        let start = self.byte_pos;
        let token = self.next_token_inner();
        let end = self.byte_pos;
        (token, (start, end - start))
    }

    fn next_token_inner(&mut self) -> Token {
        let tail = &self.src[self.byte_pos..];
        if tail.is_empty() {
            return Token::Eof;
        }

        if tail.starts_with("///") {
            self.byte_pos += 3; // consume '///'
            let mut comment = String::new();
            while let Some(c) = self.advance() {
                if c == '\n' {
                    break;
                }
                comment.push(c);
            }
            return Token::DocComment(comment.trim().to_string());
        }

        let c = self.peek().unwrap();

        if c.is_alphabetic() {
            return self.ident();
        }
        if c.is_ascii_digit() {
            return self.number();
        }

        // Multi-character operators
        if tail.starts_with("::") { self.byte_pos += 2; return Token::ColonColon; }
        if tail.starts_with("==") { self.byte_pos += 2; return Token::EqEq; }
        if tail.starts_with("=>") { self.byte_pos += 2; return Token::FatArrow; }
        if tail.starts_with("?.") { self.byte_pos += 2; return Token::QuestionDot; }
        if tail.starts_with("??") { self.byte_pos += 2; return Token::QuestionQuestion; }
        if tail.starts_with("!=") { self.byte_pos += 2; return Token::NotEq; }
        if tail.starts_with("->") { self.byte_pos += 2; return Token::Arrow; }
        if tail.starts_with("<=") { self.byte_pos += 2; return Token::LessEq; }
        if tail.starts_with(">=") { self.byte_pos += 2; return Token::GreaterEq; }
        if tail.starts_with("&&") { self.byte_pos += 2; return Token::AndAnd; }
        if tail.starts_with("||") { self.byte_pos += 2; return Token::PipePipe; }

        self.advance(); // consume 1 char
        match c {
            ':' => Token::Colon,
            '.' => Token::Dot,
            ';' => Token::Semi,
            '=' => Token::Eq,
            '?' => Token::Question,
            '!' => Token::Bang,
            '+' => Token::Plus,
            '-' => Token::Minus,
            '<' => Token::Less,
            '>' => Token::Greater,
            '&' => Token::Ident("&".to_string()),
            '|' => Token::Ident("|".to_string()),
            '*' => Token::Star,
            '/' => Token::Slash,
            '%' => Token::Mod,
            '(' => Token::LParen,
            ')' => Token::RParen,
            '{' => Token::LBrace,
            '}' => Token::RBrace,
            ',' => Token::Comma,
            '"' => self.string_inner(),
            _ => Token::Ident(c.to_string()),
        }
    }

    fn ident(&mut self) -> Token {
        let start = self.byte_pos;
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }
        let s = &self.src[start..self.byte_pos];
        match s {
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
            "private" => Token::Private,
            "async" => Token::Async,
            "await" => Token::Await,
            "actor" => Token::Actor,
            "static" => Token::Static,
            "struct" => Token::Struct,
            "enum" => Token::Enum,
            "import" => Token::Import,
            "export" => Token::Export,
            "from" => Token::From,
            "as" => Token::As,
            "show" => Token::Show,
            "hide" => Token::Hide,
            "for" => Token::For,
            "in" => Token::In,
            "while" => Token::While,
            "loop" => Token::Loop,
            "match" => Token::Match,
            _ => Token::Ident(s.to_string()),
        }
    }

    fn number(&mut self) -> Token {
        let start = self.byte_pos;
        let mut is_float = false;
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.advance();
            } else if c == '.' {
                is_float = true;
                self.advance();
            } else {
                break;
            }
        }
        let s = &self.src[start..self.byte_pos];
        if is_float {
            Token::Float(s.parse().unwrap_or(0.0))
        } else {
            Token::Int(s.parse().unwrap_or(0))
        }
    }

    fn string_inner(&mut self) -> Token {
        // the opening quote has already been consumed
        let mut s = String::new();
        while let Some(c) = self.advance() {
            match c {
                '"' => break,
                '\\' => {
                    if let Some(ec) = self.advance() {
                        match ec {
                            'n' => s.push('\n'),
                            'r' => s.push('\r'),
                            't' => s.push('\t'),
                            '\\' => s.push('\\'),
                            '"' => s.push('"'),
                            _ => s.push(ec),
                        }
                    }
                }
                _ => s.push(c),
            }
        }
        Token::String(s)
    }
}
