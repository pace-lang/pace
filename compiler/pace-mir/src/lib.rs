pub mod builder;
pub mod mir;

pub use builder::MirBuilder;
pub use mir::*;

#[cfg(test)]
mod tests {
    use super::*;
    use pace_ast::BinaryOp;
    use pace_hir::{Expr, HirId};
    use pace_span::Span;
    use std::collections::HashMap;

    #[test]
    fn test_mir_builder() {
        // Simulating the HIR for: `(x + 1) + 2`
        let expr = Expr::Binary {
            left: Box::new(Expr::Binary {
                left: Box::new(Expr::Ident(HirId(1), "x".to_string(), None, Span::DUMMY)),
                op: BinaryOp::Add,
                right: Box::new(Expr::IntLiteral("1".to_string(), Span::DUMMY)),
                span: Span::DUMMY,
            }),
            op: BinaryOp::Add,
            right: Box::new(Expr::IntLiteral("2".to_string(), Span::DUMMY)),
            span: Span::DUMMY,
        };

        let global_fns = HashMap::new();
        let struct_defs = HashMap::new();
        let class_defs = HashMap::new();
        let class_vtables = HashMap::new();
        let enum_defs = HashMap::new();
        let global_env = HashMap::new();
        let local_types = HashMap::new();
        let named_types = HashMap::new();
        let static_fields_env = HashMap::new();
        let methods_env = HashMap::new();
        let class_parents = HashMap::new();

        let global_functions_env = HashMap::new();
        let resolved_global_names = HashMap::new();

        let mut builder = MirBuilder::new(
            global_fns,
            &struct_defs,
            &class_defs,
            &class_vtables,
            &enum_defs,
            &global_env,
            &local_types,
            &named_types,
            &static_fields_env,
            &methods_env,
            &class_parents,
            &global_functions_env,
            &resolved_global_names,
        );
        builder.locals.push(pace_ty::Ty::Int);
        builder.hir_to_local.insert(HirId(1), Local(0));

        let _result_local = builder.build_expr(&expr);
        let body = builder.finish(&[]);

        assert_eq!(body.blocks.len(), 1);

        let stmts = &body.blocks[0].statements;
        assert_eq!(stmts.len(), 6);

        match &stmts[0] {
            Statement::Assign(_, Rvalue::Use(Local(0))) => {}
            _ => panic!("Expected Use(Local(0))"),
        }

        match &stmts[1] {
            Statement::Assign(_, Rvalue::IntConstant(v)) => assert_eq!(v, "1"),
            _ => panic!("Expected IntConstant 1"),
        }

        match &stmts[2] {
            Statement::Assign(_, Rvalue::BinaryOp(BinaryOp::Add, Local(1), Local(2))) => {}
            _ => panic!("Expected Add(1, 2)"),
        }

        match &stmts[3] {
            Statement::Assign(_, Rvalue::IntConstant(v)) => assert_eq!(v, "2"),
            _ => panic!("Expected IntConstant 2"),
        }

        match &stmts[4] {
            Statement::Assign(_, Rvalue::BinaryOp(BinaryOp::Add, Local(3), Local(4))) => {}
            _ => panic!("Expected Add(3, 4)"),
        }

        match &stmts[5] {
            Statement::Assign(_, Rvalue::IntConstant(v)) => assert_eq!(v, "0"),
            _ => panic!("Expected IntConstant 0 for return"),
        }

        match &body.blocks[0].terminator {
            Some(Terminator::Return(Local(6))) => {}
            _ => panic!("Expected Return(_6)"),
        }
    }
}
