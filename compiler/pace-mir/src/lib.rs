pub mod mir;
pub mod builder;

pub use mir::*;
pub use builder::MirBuilder;

#[cfg(test)]
mod tests {
    use super::*;
    use pace_hir::{Expr, HirId};
    use pace_ast::BinaryOp;
    use pace_span::Span;

    #[test]
    fn test_mir_builder() {
        // Simulating the HIR for: `(x + 1) + 2`
        let expr = Expr::Binary {
            left: Box::new(Expr::Binary {
                left: Box::new(Expr::Ident(HirId(1), Span::DUMMY)),
                op: BinaryOp::Add,
                right: Box::new(Expr::IntLiteral("1".to_string(), Span::DUMMY)),
                span: Span::DUMMY,
            }),
            op: BinaryOp::Add,
            right: Box::new(Expr::IntLiteral("2".to_string(), Span::DUMMY)),
            span: Span::DUMMY,
        };

        let mut builder = MirBuilder::new();
        let result_local = builder.build_expr(&expr);
        let body = builder.finish(result_local);

        assert_eq!(body.blocks.len(), 1);
        
        let stmts = &body.blocks[0].statements;
        assert_eq!(stmts.len(), 4);
        
        match &stmts[0] {
            Statement::Assign(_, Rvalue::IntConstant(v)) => assert_eq!(v, "1"),
            _ => panic!("Expected IntConstant 1"),
        }
        
        match &stmts[1] {
            Statement::Assign(_, Rvalue::BinaryOp(BinaryOp::Add, Local(0), Local(1))) => {}
            _ => panic!("Expected Add(x, 1)"),
        }

        match &stmts[2] {
            Statement::Assign(_, Rvalue::IntConstant(v)) => assert_eq!(v, "2"),
            _ => panic!("Expected IntConstant 2"),
        }

        match &stmts[3] {
            Statement::Assign(_, Rvalue::BinaryOp(BinaryOp::Add, Local(2), Local(3))) => {}
            _ => panic!("Expected Add(_2, 2)"),
        }

        match &body.blocks[0].terminator {
            Some(Terminator::Return(Local(4))) => {}
            _ => panic!("Expected Return(_4)"),
        }
    }
}
