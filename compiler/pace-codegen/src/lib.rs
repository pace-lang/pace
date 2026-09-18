pub mod c_backend;

pub use c_backend::CGenerator;

#[cfg(test)]
mod tests {
    use super::*;
    use pace_mir::{BasicBlock, Constant, Local, Lvalue, MirBody, MirProgram, Rvalue, Statement, Terminator};
    use pace_ty::Ty;

    #[test]
    fn test_c_generation() {
        // Construct a mock MirBody for `_0 = 42; _1 = "hello"; retain(_1); return _0;`
        let body = MirBody {
            locals: vec![Ty::Int, Ty::String],
            blocks: vec![BasicBlock {
                statements: vec![
                    Statement::Assign(
                        Lvalue::Local(Local(0)),
                        Rvalue::Constant(Constant::Int("42".to_string())),
                    ),
                    Statement::Assign(
                        Lvalue::Local(Local(1)),
                        Rvalue::Constant(Constant::String("hello".to_string())),
                    ),
                    Statement::Retain(Lvalue::Local(Local(1))),
                ],
                terminator: Some(Terminator::Return(Local(0))),
            }],
        };

        let program = MirProgram {
            functions: vec![],
            main_body: body,
            struct_defs: Default::default(),
            class_defs: Default::default(),
            class_vtables: Default::default(),
            enum_defs: Default::default(),
            global_vars: vec![],
        };

        let mut generator = CGenerator::new();
        let code = generator.generate(&program);

        assert!(code.contains("long long _0 = 0;"));
        assert!(code.contains("char* _1 = NULL;"));
        assert!(code.contains("_0 = 42;"));
        assert!(code.contains("_1 = \"hello\";"));
        assert!(code.contains("pace_retain(_1);"));
    }
}
