pub mod assignment;
pub mod binary;
pub mod call;
pub mod match_expr;
pub mod primary;
pub mod unary;

use super::Parser;
use ast::*;

impl<'a> Parser<'a> {
    pub(crate) fn expression(&mut self) -> Option<Expr<'a>> {
        self.assignment()
    }
}
