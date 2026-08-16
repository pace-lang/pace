use crate::checker::TypeChecker;
use ast::*;
use diagnostics::Location;
use session::CompilerSession;
use session::types::Type;

fn make_span() -> Span {
    Span::new(0, 0, 0, Location::new(1, 1), Location::new(1, 1))
}

#[test]
fn test_valid_math() {
    let session = CompilerSession::new();
    let mut checker = TypeChecker::new(&session);
    let sym_x = session.interner.borrow_mut().intern("x");
    // let x = 10 + 5;
    let stmt = Stmt::new(
        StmtKind::Let {
            name: sym_x,
            is_private: false,
            type_annotation: None,
            initializer: Some(checker.alloc(Expr::new(
                ExprKind::Binary(
                    checker.alloc(Expr::new(ExprKind::Integer(10), make_span())),
                    BinaryOp::Add,
                    checker.alloc(Expr::new(ExprKind::Integer(5), make_span())),
                ),
                make_span(),
            ))),
        },
        make_span(),
    );

    checker.check(&[stmt]);
    assert!(checker.errors.is_empty());
    assert_eq!(
        checker.env.resolve(sym_x).unwrap(),
        session.types.borrow_mut().intern(Type::Int)
    );
}

#[test]
fn test_type_mismatch() {
    let session = CompilerSession::new();
    let mut checker = TypeChecker::new(&session);
    let sym_x = session.interner.borrow_mut().intern("x");
    let sym_hello = session.interner.borrow_mut().intern("hello");
    // let x = 10 + "hello";
    let stmt = Stmt::new(
        StmtKind::Let {
            name: sym_x,
            is_private: false,
            type_annotation: None,
            initializer: Some(checker.alloc(Expr::new(
                ExprKind::Binary(
                    checker.alloc(Expr::new(ExprKind::Integer(10), make_span())),
                    BinaryOp::Add,
                    checker.alloc(Expr::new(ExprKind::String(sym_hello), make_span())),
                ),
                make_span(),
            ))),
        },
        make_span(),
    );

    checker.check(&[stmt]);
    assert_eq!(checker.errors.len(), 1);
    assert!(
        checker.errors[0]
            .message
            .contains("Cannot apply operator to types 'Int' and 'String'")
    );
}

#[test]
fn test_if_condition_type() {
    let session = CompilerSession::new();
    let mut checker = TypeChecker::new(&session);
    // if 10 { }
    let stmt = Stmt::new(
        StmtKind::If {
            condition: checker.alloc(Expr::new(ExprKind::Integer(10), make_span())),
            then_branch: checker.alloc(Stmt::new(StmtKind::Block(vec![]), make_span())),
            else_branch: None,
        },
        make_span(),
    );

    checker.check(&[stmt]);
    assert_eq!(checker.errors.len(), 1);
    assert!(checker.errors[0].message.contains("Expected 'Boolean'"));
}

#[test]
fn test_immutable_assignment() {
    let session = CompilerSession::new();
    let mut checker = TypeChecker::new(&session);
    let sym_x = session.interner.borrow_mut().intern("x");
    // let x = 10;
    let stmt1 = Stmt::new(
        StmtKind::Let {
            name: sym_x,
            is_private: false,
            type_annotation: None,
            initializer: Some(checker.alloc(Expr::new(ExprKind::Integer(10), make_span()))),
        },
        make_span(),
    );

    // x = 20;
    let stmt2 = Stmt::new(
        StmtKind::Expression(checker.alloc(Expr::new(
            ExprKind::Assign {
                name: sym_x,
                value: checker.alloc(Expr::new(ExprKind::Integer(20), make_span())),
            },
            make_span(),
        ))),
        make_span(),
    );

    checker.check(&[stmt1, stmt2]);
    assert_eq!(checker.errors.len(), 1);
    assert!(
        checker.errors[0]
            .message
            .contains("Cannot mutate immutable variable 'x'")
    );
}
