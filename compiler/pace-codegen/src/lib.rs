pub mod c_backend;

pub use c_backend::CGenerator;

#[cfg(test)]
mod tests {
    use super::*;
    use pace_mir::{BasicBlock, Local, MirBody, Rvalue, Statement, Terminator};

    #[test]
    fn test_c_generation() {
        // Construct a mock MirBody for `_0 = 42; _1 = "hello"; retain(_1); return _0;`
        let body = MirBody {
            locals: 2,
            blocks: vec![
                BasicBlock {
                    statements: vec![
                        Statement::Assign(Local(0), Rvalue::IntConstant("42".to_string())),
                        Statement::Assign(Local(1), Rvalue::StringConstant("hello".to_string())),
                        Statement::Retain(Local(1)),
                    ],
                    terminator: Some(Terminator::Return(Local(0))),
                }
            ],
        };

        let mut generator = CGenerator::new();
        let code = generator.generate(&body);

        assert!(code.contains("long long _0 = 0;"));
        assert!(code.contains("long long _1 = 0;"));
        assert!(code.contains("_0 = 42;"));
        assert!(code.contains("_1 = (long long)\"hello\";"));
        assert!(code.contains("PACE_RETAIN(_1);"));
        assert!(code.contains("return _0;"));
    }
}
