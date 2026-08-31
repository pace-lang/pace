pub mod checker;
pub mod env;

pub use checker::TypeChecker;
pub use env::{Environment, Type};
pub use pace_errors::TypeError;

pub use checker::check;

#[cfg(test)]
mod tests {
    use super::*;
    use pace_ast::arena::AstArena;

    fn check_source(src: &str) -> Result<(), Vec<pace_errors::TypeError>> {
        let mut arena = AstArena::new();
        let (stmts, _) = pace_parser::parse(&mut arena, src, "test").expect("Syntax error in test");
        let mut sources = std::collections::HashMap::new();
        sources.insert(ustr::Ustr::from("test"), src.to_string());
        let (_, errors, _) = crate::check(&mut arena, &stmts, sources, "test");
        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    #[test]
    fn test_valid_assignments() {
        let src = "
            func main() {
                let x: Int = 10;
                let y: Float = 10.5;
                let z: String = \"hello\";
                let w: Bool = true;
            }
        ";
        assert!(check_source(src).is_ok());
    }

    #[test]
    fn test_invalid_assignments() {
        let src = "
            func main() {
                let x: Int = \"hello\";
            }
        ";
        let res = check_source(src);
        assert!(res.is_err());
        let errs = res.unwrap_err();
        println!("{:#?}", errs);
        assert_eq!(errs.len(), 1);
        if let pace_errors::TypeError::Generic { message, .. } = &errs[0] {
            assert!(message.contains("expected Int, found String"));
        } else {
            panic!("Expected Generic error");
        }
    }

    #[test]
    fn test_function_signature_mismatch() {
        let src = "
            func add(a: Int, b: Int) -> Int {
                return a + b;
            }
            func main() {
                add(10, \"20\");
            }
        ";
        let res = check_source(src);
        assert!(res.is_err());
        let errs = res.unwrap_err();
        println!("{:#?}", errs);
        assert_eq!(errs.len(), 1);
        if let pace_errors::TypeError::Generic { message, .. } = &errs[0] {
            assert!(message.contains("expected Int, got String"));
        } else {
            panic!("Expected Generic error");
        }
    }

    #[test]
    fn test_undefined_variable() {
        let src = "
            func main() {
                let x: Int = y;
            }
        ";
        let res = check_source(src);
        assert!(res.is_err());
        let errs = res.unwrap_err();
        println!("{:#?}", errs);
        assert!(!errs.is_empty());
        if let pace_errors::TypeError::UnknownIdentifier { name, .. } = &errs[0] {
            assert_eq!(name, "y");
        } else {
            panic!("Expected UnknownIdentifier");
        }
    }
}
