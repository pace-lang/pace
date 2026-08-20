pub mod classes;
pub mod enums;
pub mod extensions;
pub mod foreign;
pub mod functions;
pub mod imports_exports;
pub mod interfaces;
pub mod structs;
pub mod types;
pub mod variables;

use super::Parser;
use ast::*;
use lexer::*;

impl<'a> Parser<'a> {
    pub(crate) fn parse_visibility(&mut self) -> bool {
        if self.match_token(&[TokenKind::Private]) {
            true
        } else {
            let _ = self.match_token(&[TokenKind::Public]);
            false
        }
    }

    pub(crate) fn declaration(&mut self) -> Option<Stmt<'a>> {
        let is_private = self.parse_visibility();

        let res = if self.match_token(&[TokenKind::Interface]) {
            self.interface_declaration(is_private)
        } else if self.match_token(&[TokenKind::Type]) {
            self.type_alias_declaration(is_private)
        } else if self.match_token(&[TokenKind::Struct]) {
            self.struct_declaration(is_private)
        } else if self.match_token(&[TokenKind::Class]) {
            self.class_declaration(is_private)
        } else if self.match_token(&[TokenKind::Enum]) {
            self.enum_declaration(is_private)
        } else if self.match_token(&[TokenKind::Extend]) {
            if is_private {
                self.error_at_current("Visibility modifiers are not allowed on extensions.");
            }
            self.extension_declaration()
        } else if self.match_token(&[TokenKind::Actor]) {
            self.actor_declaration(is_private)
        } else if self.match_token(&[TokenKind::Async]) {
            if self.match_token(&[TokenKind::Func]) {
                self.function_declaration(is_private, true)
            } else {
                self.error_at_current("Expected 'func' after 'async'.");
                None
            }
        } else if self.match_token(&[TokenKind::Func]) {
            self.function_declaration(is_private, false)
        } else if self.match_token(&[TokenKind::Let]) {
            self.variable_declaration(false, false, is_private)
        } else if self.match_token(&[TokenKind::Var]) {
            self.variable_declaration(true, false, is_private)
        } else if self.match_token(&[TokenKind::Weak]) {
            if self.match_token(&[TokenKind::Var]) {
                self.variable_declaration(true, true, is_private)
            } else {
                self.error_at_current("Expected 'var' after 'weak'.");
                None
            }
        } else if self.match_token(&[TokenKind::Foreign]) {
            self.foreign_declaration(is_private)
        } else if self.match_token(&[TokenKind::Import]) {
            if is_private {
                self.error_at_current("Visibility modifiers are not allowed on imports.");
            }
            self.import_declaration()
        } else if self.match_token(&[TokenKind::Export]) {
            if is_private {
                self.error_at_current("Visibility modifiers are not allowed on exports.");
            }
            self.export_declaration()
        } else {
            if is_private {
                self.error_at_current("Visibility modifiers are only allowed on declarations.");
            }
            self.statement()
        };

        if res.is_none() {
            self.synchronize();
        }
        res
    }
}
