use crate::expr::Expr;
use diagnostics::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum TypeExpr<'a> {
    Named(session::Symbol),
    GenericInstance(session::Symbol, Vec<TypeExpr<'a>>),
    Optional(&'a TypeExpr<'a>),
    Array(&'a TypeExpr<'a>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumField<'a> {
    pub name: Option<session::Symbol>,
    pub ty: TypeExpr<'a>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant<'a> {
    pub name: session::Symbol,
    pub fields: Option<Vec<EnumField<'a>>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StmtKind<'a> {
    /// An enum declaration: `enum Name<T> { Variant(Int), Unit }`
    Enum {
        name: session::Symbol,
        type_params: Vec<session::Symbol>,
        variants: Vec<EnumVariant<'a>>,
        methods: Vec<Stmt<'a>>,
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
    Export { path: session::Symbol },
    /// A let declaration: `let name = expression;`
    Let {
        name: session::Symbol,
        type_annotation: Option<TypeExpr<'a>>,
        initializer: Option<&'a Expr<'a>>,
        is_private: bool,
    },
    /// A var declaration: `[weak] var name: type = expression;`
    Var {
        name: session::Symbol,
        type_annotation: Option<TypeExpr<'a>>,
        initializer: Option<&'a Expr<'a>>,
        is_weak: bool,
        is_private: bool,
    },
    /// An expression evaluated for side effects: `10 + 20;`
    Expression(&'a Expr<'a>),
    /// A block of statements: `{ ... }`
    Block(Vec<Stmt<'a>>),
    /// An if statement: `if condition { then_branch } else { else_branch }`
    If {
        condition: &'a Expr<'a>,
        then_branch: &'a Stmt<'a>,
        else_branch: Option<&'a Stmt<'a>>,
    },
    /// A while loop: `while condition { body }`
    While {
        condition: &'a Expr<'a>,
        body: &'a Stmt<'a>,
    },
    /// A for loop: `for item in iterator { body }`
    For {
        item_name: session::Symbol,
        iterator: &'a Expr<'a>,
        body: &'a Stmt<'a>,
    },
    /// A function declaration: `func name<T>(params) -> return_type { body }`
    Func {
        name: session::Symbol,
        type_params: Vec<session::Symbol>,
        params: Vec<(session::Symbol, TypeExpr<'a>)>, // (name, type)
        return_type: Option<TypeExpr<'a>>,
        body: &'a Stmt<'a>,
        is_private: bool,
    },
    /// A foreign function declaration: `foreign func name<T>(params) -> return_type;`
    ForeignFunc {
        name: session::Symbol,
        type_params: Vec<session::Symbol>,
        params: Vec<(session::Symbol, TypeExpr<'a>)>,
        return_type: Option<TypeExpr<'a>>,
        is_private: bool,
    },
    /// A return statement: `return value;`
    Return { value: Option<&'a Expr<'a>> },
    /// A class declaration: `class name<T> { fields; methods; }`
    Class {
        name: session::Symbol,
        type_params: Vec<session::Symbol>,
        implements: Vec<TypeExpr<'a>>,
        methods: Vec<Stmt<'a>>,
        fields: Vec<Stmt<'a>>,
        is_private: bool,
    },
    /// A struct declaration: `struct name<T> { fields; methods; }`
    Struct {
        name: session::Symbol,
        type_params: Vec<session::Symbol>,
        methods: Vec<Stmt<'a>>,
        fields: Vec<Stmt<'a>>,
        is_private: bool,
    },
    /// A type alias declaration: `type name<T> = target_type;`
    TypeAlias {
        name: session::Symbol,
        type_params: Vec<session::Symbol>,
        target_type: TypeExpr<'a>,
        is_private: bool,
    },
    /// An interface declaration: `interface name<T> { methods; }`
    Interface {
        name: session::Symbol,
        type_params: Vec<session::Symbol>,
        methods: Vec<Stmt<'a>>,
        is_private: bool,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stmt<'a> {
    pub kind: StmtKind<'a>,
    pub span: Span,
}

impl<'a> Stmt<'a> {
    pub fn new(kind: StmtKind<'a>, span: Span) -> Self {
        Self { kind, span }
    }
}
