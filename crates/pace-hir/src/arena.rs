use crate::expr::Expr;
use crate::stmt::Stmt;
use pace_span::Span;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ExprId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StmtId(pub usize);

#[derive(Debug, Default)]
pub struct HirArena {
    pub exprs: Vec<Expr>,
    pub stmts: Vec<Stmt>,
    pub expr_spans: Vec<Span>,
    pub stmt_spans: Vec<Span>,
}

impl HirArena {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn alloc_expr(&mut self, expr: Expr, span: Span) -> ExprId {
        let id = ExprId(self.exprs.len());
        self.exprs.push(expr);
        self.expr_spans.push(span);
        id
    }

    pub fn alloc_stmt(&mut self, stmt: Stmt, span: Span) -> StmtId {
        let id = StmtId(self.stmts.len());
        self.stmts.push(stmt);
        self.stmt_spans.push(span);
        id
    }

    pub fn get_expr(&self, id: ExprId) -> &Expr {
        &self.exprs[id.0]
    }

    pub fn get_stmt(&self, id: StmtId) -> &Stmt {
        &self.stmts[id.0]
    }

    pub fn get_expr_span(&self, id: ExprId) -> Span {
        self.expr_spans[id.0]
    }

    pub fn get_stmt_span(&self, id: StmtId) -> Span {
        self.stmt_spans[id.0]
    }
}
