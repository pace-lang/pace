use std::str::Chars;
use ast::{Location, Span};
use diagnostics::{Diagnostic, DiagnosticBuilder, DiagnosticCode};
use crate::token::{Token, TokenKind};

pub struct Scanner<'a> {
    source: &'a str,
    chars: Chars<'a>,
    current_idx: usize,
    current_loc: Location,
    start_idx: usize,
    start_loc: Location,
    pub diagnostics: Vec<Diagnostic>,
}

impl<'a> Scanner<'a> {
    pub fn new(source: &'a str) -> Self {
        Self {
            source,
            chars: source.chars(),
            current_idx: 0,
            current_loc: Location::new(1, 1),
            start_idx: 0,
            start_loc: Location::new(1, 1),
            diagnostics: Vec::new(),
        }
    }

    pub fn scan_tokens(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        while !self.is_at_end() {
            self.start_idx = self.current_idx;
            self.start_loc = self.current_loc;
            if let Some(token) = self.scan_token() {
                tokens.push(token);
            }
        }
        
        tokens.push(Token::new(
            TokenKind::Eof,
            Span::new(self.current_idx, self.current_idx, self.current_loc, self.current_loc)
        ));
        
        tokens
    }

    fn is_at_end(&self) -> bool {
        self.chars.clone().next().is_none()
    }

    fn advance(&mut self) -> Option<char> {
        if let Some(c) = self.chars.next() {
            self.current_idx += c.len_utf8();
            if c == '\n' {
                self.current_loc.line += 1;
                self.current_loc.column = 1;
            } else {
                self.current_loc.column += 1;
            }
            Some(c)
        } else {
            None
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.clone().next()
    }

    fn match_char(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn scan_token(&mut self) -> Option<Token> {
        let c = self.advance()?;

        let kind = match c {
            '(' => TokenKind::LeftParen,
            ')' => TokenKind::RightParen,
            '{' => TokenKind::LeftBrace,
            '}' => TokenKind::RightBrace,
            '[' => TokenKind::LeftBracket,
            ']' => TokenKind::RightBracket,
            ',' => TokenKind::Comma,
            '.' => TokenKind::Dot,
            ':' => TokenKind::Colon,
            ';' => TokenKind::Semicolon,
            '+' => TokenKind::Plus,
            '-' => {
                if self.match_char('>') {
                    TokenKind::Arrow
                } else {
                    TokenKind::Minus
                }
            }
            '*' => TokenKind::Star,
            '/' => {
                if self.match_char('/') {
                    // A comment goes until the end of the line.
                    while self.peek() != Some('\n') && !self.is_at_end() {
                        self.advance();
                    }
                    return None;
                } else {
                    TokenKind::Slash
                }
            }
            '=' => {
                if self.match_char('=') {
                    TokenKind::EqualEqual
                } else if self.match_char('>') {
                    TokenKind::FatArrow
                } else {
                    TokenKind::Equal
                }
            }
            '!' => {
                if self.match_char('=') {
                    TokenKind::BangEqual
                } else {
                    let span = Span::new(self.start_idx, self.current_idx, self.start_loc, self.current_loc);
                    self.diagnostics.push(DiagnosticBuilder::error(DiagnosticCode::UnexpectedToken, "Unexpected character '!'", span).build());
                    TokenKind::Error("Unexpected character '!'".into())
                }
            }
            '<' => {
                if self.match_char('=') {
                    TokenKind::LessEqual
                } else {
                    TokenKind::Less
                }
            }
            '>' => {
                if self.match_char('=') {
                    TokenKind::GreaterEqual
                } else {
                    TokenKind::Greater
                }
            }
            '?' => TokenKind::Question,
            ' ' | '\r' | '\t' | '\n' => {
                // Ignore whitespace.
                return None;
            }
            '"' => self.string(),
            c if c.is_ascii_digit() => self.number(),
            c if c.is_ascii_alphabetic() || c == '_' => self.identifier(),
            _ => {
                let span = Span::new(self.start_idx, self.current_idx, self.start_loc, self.current_loc);
                self.diagnostics.push(DiagnosticBuilder::error(DiagnosticCode::UnexpectedToken, format!("Unexpected character '{}'", c), span).build());
                TokenKind::Error(format!("Unexpected character '{}'", c))
            }
        };

        Some(self.make_token(kind))
    }

    fn string(&mut self) -> TokenKind {
        let mut value = String::new();
        while self.peek() != Some('"') && !self.is_at_end() {
            if let Some(c) = self.advance() {
                value.push(c);
            }
        }

        if self.is_at_end() {
            let span = Span::new(self.start_idx, self.current_idx, self.start_loc, self.current_loc);
            self.diagnostics.push(DiagnosticBuilder::error(DiagnosticCode::InvalidToken, "Unterminated string.", span).build());
            return TokenKind::Error("Unterminated string.".into());
        }

        // The closing ".
        self.advance();
        TokenKind::String(value)
    }

    fn number(&mut self) -> TokenKind {
        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.advance();
            } else {
                break;
            }
        }

        // Look for a fractional part.
        let is_float = if self.peek() == Some('.') {
            // Check if there is a digit after the dot
            let mut chars_clone = self.chars.clone();
            chars_clone.next(); // consume dot
            if let Some(c) = chars_clone.next() {
                if c.is_ascii_digit() {
                    // Consume the dot
                    self.advance();
                    while let Some(c) = self.peek() {
                        if c.is_ascii_digit() {
                            self.advance();
                        } else {
                            break;
                        }
                    }
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };

        let text = &self.source[self.start_idx..self.current_idx];
        if is_float {
            match text.parse::<f64>() {
                Ok(f) => TokenKind::Float(f),
                Err(_) => {
                    let span = Span::new(self.start_idx, self.current_idx, self.start_loc, self.current_loc);
                    self.diagnostics.push(DiagnosticBuilder::error(DiagnosticCode::InvalidToken, "Invalid float literal", span).build());
                    TokenKind::Error("Invalid float literal".into())
                }
            }
        } else {
            match text.parse::<i64>() {
                Ok(i) => TokenKind::Integer(i),
                Err(_) => {
                    let span = Span::new(self.start_idx, self.current_idx, self.start_loc, self.current_loc);
                    self.diagnostics.push(DiagnosticBuilder::error(DiagnosticCode::InvalidToken, "Invalid integer literal", span).build());
                    TokenKind::Error("Invalid integer literal".into())
                }
            }
        }
    }

    fn identifier(&mut self) -> TokenKind {
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }

        let text = &self.source[self.start_idx..self.current_idx];
        match text {
            "let" => TokenKind::Let,
            "var" => TokenKind::Var,
            "func" => TokenKind::Func,
            "init" => TokenKind::Init,
            "self" => TokenKind::SelfKeyword,
            "class" => TokenKind::Class,
            "interface" => TokenKind::Interface,
            "implements" => TokenKind::Implements,
            "type" => TokenKind::Type,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "for" => TokenKind::For,
            "while" => TokenKind::While,
            "switch" => TokenKind::Switch,
            "return" => TokenKind::Return,
            "break" => TokenKind::Break,
            "continue" => TokenKind::Continue,
            "import" => TokenKind::Import,
            "package" => TokenKind::Package,
            "async" => TokenKind::Async,
            "await" => TokenKind::Await,
            "true" => TokenKind::True,
            "false" => TokenKind::False,
            _ => TokenKind::Identifier(text.to_string()),
        }
    }

    fn make_token(&self, kind: TokenKind) -> Token {
        let span = Span::new(
            self.start_idx,
            self.current_idx,
            self.start_loc,
            self.current_loc,
        );
        Token::new(kind, span)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_tokens() {
        let source = "let count = 10;";
        let mut scanner = Scanner::new(source);
        let tokens = scanner.scan_tokens();
        
        assert_eq!(tokens.len(), 6); // let, count, =, 10, ;, EOF
        assert_eq!(tokens[0].kind, TokenKind::Let);
        assert_eq!(tokens[1].kind, TokenKind::Identifier("count".into()));
        assert_eq!(tokens[2].kind, TokenKind::Equal);
        assert_eq!(tokens[3].kind, TokenKind::Integer(10));
        assert_eq!(tokens[4].kind, TokenKind::Semicolon);
        assert_eq!(tokens[5].kind, TokenKind::Eof);
    }
    
    #[test]
    fn test_error_token() {
        let source = "let x = @;";
        let mut scanner = Scanner::new(source);
        let tokens = scanner.scan_tokens();
        
        assert_eq!(tokens[3].kind, TokenKind::Error("Unexpected character '@'".into()));
    }
}
