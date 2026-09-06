#[derive(Clone, Debug, PartialEq)]
pub enum Token<'a> {
    Ident(&'a str),
    Int(i64),
    Float(f64),
    String(String),
    Bool(bool),
    DocComment(&'a str),
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
    Extern,
    Async,
    Await,
    Actor,
    Static,
    Weak,
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
    LBracket,
    RBracket,
    Question,
    Bang,
    BitNot,
    QuestionDot,
    QuestionQuestion,
    Eof,
}

#[derive(Clone)]
pub struct Lexer<'a> {
    src: &'a str,
    pub byte_pos: usize,
    pub comments: Vec<(usize, usize, String)>,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            src,
            byte_pos: 0,
            comments: Vec::new(),
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
                let start_pos = self.byte_pos;
                if let Some(idx) = tail.find('\n') {
                    self.byte_pos += idx + 1;
                    let text = tail[..idx].to_string();
                    self.comments.push((start_pos, start_pos + idx, text));
                } else {
                    self.byte_pos = self.src.len();
                    let text = tail.to_string();
                    self.comments.push((start_pos, self.src.len(), text));
                }
            } else if tail.starts_with("/*") {
                let start_pos = self.byte_pos;
                if let Some(idx) = tail.find("*/") {
                    self.byte_pos += idx + 2;
                    let text = tail[..idx + 2].to_string();
                    self.comments.push((start_pos, start_pos + idx + 2, text));
                } else {
                    self.byte_pos = self.src.len();
                    let text = tail.to_string();
                    self.comments.push((start_pos, self.src.len(), text));
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

    pub fn next_token(&mut self) -> (Token<'a>, (usize, usize)) {
        self.skip_whitespace();
        let start = self.byte_pos;
        let token = self.next_token_inner();
        let end = self.byte_pos;
        (token, (start, end - start))
    }

    fn next_token_inner(&mut self) -> Token<'a> {
        let tail = &self.src[self.byte_pos..];
        if tail.is_empty() {
            return Token::Eof;
        }

        if tail.starts_with("///") {
            self.byte_pos += 3; // consume '///'
            let start = self.byte_pos;
            while let Some(c) = self.peek() {
                if c == '\n' {
                    break;
                }
                self.advance();
            }
            let s = &self.src[start..self.byte_pos];
            return Token::DocComment(s.trim());
        }

        let c = self.peek().unwrap();

        if c.is_alphabetic() || c == '_' {
            return self.ident();
        }
        if c.is_ascii_digit() {
            return self.number();
        }

        // Multi-character operators
        if tail.starts_with("==") {
            self.byte_pos += 2;
            return Token::EqEq;
        }
        if tail.starts_with("=>") {
            self.byte_pos += 2;
            return Token::FatArrow;
        }
        if tail.starts_with("?.") {
            self.byte_pos += 2;
            return Token::QuestionDot;
        }
        if tail.starts_with("??") {
            self.byte_pos += 2;
            return Token::QuestionQuestion;
        }
        if tail.starts_with("!=") {
            self.byte_pos += 2;
            return Token::NotEq;
        }
        if tail.starts_with("->") {
            self.byte_pos += 2;
            return Token::Arrow;
        }
        if tail.starts_with("<=") {
            self.byte_pos += 2;
            return Token::LessEq;
        }
        if tail.starts_with(">=") {
            self.byte_pos += 2;
            return Token::GreaterEq;
        }
        if tail.starts_with("&&") {
            self.byte_pos += 2;
            return Token::AndAnd;
        }
        if tail.starts_with("||") {
            self.byte_pos += 2;
            return Token::PipePipe;
        }

        self.advance(); // consume 1 char
        match c {
            ':' => Token::Colon,
            '.' => Token::Dot,
            ';' => Token::Semi,
            '=' => Token::Eq,
            '?' => Token::Question,
            '!' => Token::Bang,
            '~' => Token::BitNot,
            '+' => Token::Plus,
            '-' => Token::Minus,
            '<' => Token::Less,
            '>' => Token::Greater,
            '&' => Token::Ident("&"),
            '|' => Token::Ident("|"),
            '*' => Token::Star,
            '/' => Token::Slash,
            '%' => Token::Mod,
            '(' => Token::LParen,
            ')' => Token::RParen,
            '{' => Token::LBrace,
            '}' => Token::RBrace,
            '[' => Token::LBracket,
            ']' => Token::RBracket,
            ',' => Token::Comma,
            '"' => self.string_inner(),
            _ => {
                let s = &self.src[self.byte_pos - c.len_utf8()..self.byte_pos];
                Token::Ident(s)
            }
        }
    }

    fn ident(&mut self) -> Token<'a> {
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

            "extern" => Token::Extern,
            "async" => Token::Async,
            "await" => Token::Await,
            "actor" => Token::Actor,
            "static" => Token::Static,
            "weak" => Token::Weak,
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
            _ => Token::Ident(s),
        }
    }

    fn number(&mut self) -> Token<'a> {
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

    fn string_inner(&mut self) -> Token<'a> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keywords() {
        let mut lexer = Lexer::new("func class interface enum let var const if else return");
        assert_eq!(lexer.next_token().0, Token::Func);
        assert_eq!(lexer.next_token().0, Token::Class);
        assert_eq!(lexer.next_token().0, Token::Interface);
        assert_eq!(lexer.next_token().0, Token::Enum);
        assert_eq!(lexer.next_token().0, Token::Let);
        assert_eq!(lexer.next_token().0, Token::Var);
        assert_eq!(lexer.next_token().0, Token::Const);
        assert_eq!(lexer.next_token().0, Token::If);
        assert_eq!(lexer.next_token().0, Token::Else);
        assert_eq!(lexer.next_token().0, Token::Return);
        assert_eq!(lexer.next_token().0, Token::Eof);
    }

    #[test]
    fn test_literals() {
        let mut lexer = Lexer::new("42 3.14 \"hello\" true false null");
        assert_eq!(lexer.next_token().0, Token::Int(42));
        assert_eq!(lexer.next_token().0, Token::Float(3.14));
        assert_eq!(lexer.next_token().0, Token::String("hello".to_string()));
        assert_eq!(lexer.next_token().0, Token::Bool(true));
        assert_eq!(lexer.next_token().0, Token::Bool(false));
        assert_eq!(lexer.next_token().0, Token::Null);
        assert_eq!(lexer.next_token().0, Token::Eof);
    }

    #[test]
    fn test_identifiers_and_symbols() {
        let mut lexer = Lexer::new("my_var = 10; my_var == 10 =>");
        assert_eq!(lexer.next_token().0, Token::Ident("my_var"));
        assert_eq!(lexer.next_token().0, Token::Eq);
        assert_eq!(lexer.next_token().0, Token::Int(10));
        assert_eq!(lexer.next_token().0, Token::Semi);
        assert_eq!(lexer.next_token().0, Token::Ident("my_var"));
        assert_eq!(lexer.next_token().0, Token::EqEq);
        assert_eq!(lexer.next_token().0, Token::Int(10));
        assert_eq!(lexer.next_token().0, Token::FatArrow);
        assert_eq!(lexer.next_token().0, Token::Eof);
    }
}
