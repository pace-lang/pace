use diagnostics::Span;
use crate::expr::Expr;

#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr {
    Named(session::Symbol),
    GenericInstance(session::Symbol, Vec<TypeExpr>),
    Optional(Box<TypeExpr>),
    Array(Box<TypeExpr>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumField {
    pub name: Option<session::Symbol>,
    pub ty: TypeExpr,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: session::Symbol,
    pub fields: Option<Vec<EnumField>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind {
    /// An enum declaration: `enum Name<T> { Variant(Int), Unit }`
    Enum {
        name: session::Symbol,
        type_params: Vec<session::Symbol>,
        variants: Vec<EnumVariant>,
        is_private: bool,
    },
    /// An import statement: `import "path" [as alias] [show a, b] [hide x, y]`
    Import {
        path: session::Symbol,
        alias: Option<session::Symbol>,
        show: Vec<session::Symbol>,
        hide: Vec<session::Symbol>,
    },
    /// An export statement: `export "path"`
    Export {
        path: session::Symbol,
    },
    /// A let declaration: `let name = expression;`
    Let {
        name: session::Symbol,
        type_annotation: Option<TypeExpr>,
        initializer: Option<Expr>,
        is_private: bool,
    },
    /// A var declaration: `[weak] var name: type = expression;`
    Var {
        name: session::Symbol,
        type_annotation: Option<TypeExpr>,
        initializer: Option<Expr>,
        is_weak: bool,
        is_private: bool,
    },
    /// An expression evaluated for side effects: `10 + 20;`
    Expression(Expr),
    /// A block of statements: `{ ... }`
    Block(Vec<Stmt>),
    /// An if statement: `if condition { then_branch } else { else_branch }`
    If {
        condition: Expr,
        then_branch: Box<Stmt>,
        else_branch: Option<Box<Stmt>>,
    },
    /// A while loop: `while condition { body }`
    While {
        condition: Expr,
        body: Box<Stmt>,
    },
    /// A for loop: `for item in iterator { body }`
    For {
        item_name: session::Symbol,
        iterator: Expr,
        body: Box<Stmt>,
    },
    /// A function declaration: `func name<T>(params) -> return_type { body }`
    Func {
        name: session::Symbol,
        type_params: Vec<session::Symbol>,
        params: Vec<(session::Symbol, TypeExpr)>, // (name, type)
        return_type: Option<TypeExpr>,
        body: Box<Stmt>,
        is_private: bool,
    },
    /// A foreign function declaration: `foreign func name<T>(params) -> return_type;`
    ForeignFunc {
        name: session::Symbol,
        type_params: Vec<session::Symbol>,
        params: Vec<(session::Symbol, TypeExpr)>,
        return_type: Option<TypeExpr>,
        is_private: bool,
    },
    /// A return statement: `return value;`
    Return {
        value: Option<Expr>,
    },
    /// A class declaration: `class name<T> { fields; methods; }`
    Class {
        name: session::Symbol,
        type_params: Vec<session::Symbol>,
        implements: Vec<session::Symbol>,
        methods: Vec<Stmt>,
        fields: Vec<Stmt>,
        is_private: bool,
    },
    /// An interface declaration: `interface name { methods; }`
    Interface {
        name: session::Symbol,
        methods: Vec<Stmt>,
        is_private: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

impl Stmt {
    pub fn new(kind: StmtKind, span: Span) -> Self {
        Self { kind, span }
    }
}
