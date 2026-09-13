use pace_ast::{BinaryOp, Type};
use pace_span::Span;

/// A unique ID for variables and definitions across the entire program.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct HirId(pub u32);

#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub statements: Vec<Stmt>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Stmt {
    Let {
        id: HirId,
        name: String,
        ty: Option<Type>,
        value: Option<Expr>,
        span: Span,
    },
    Var {
        id: HirId,
        name: String,
        ty: Option<Type>,
        value: Option<Expr>,
        span: Span,
    },
    ExprStmt(Expr, Span),
    Return(Option<Expr>, Span),
}

use pace_span::FileId;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct Program {
    pub modules: HashMap<String, Module>,
    pub module_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub name: String,
    pub file_id: FileId,
    pub declarations: Vec<Decl>,
}

impl Program {
    pub fn resolve_traits(&mut self, reporter: &mut pace_errors::Reporter) {
        let mut trait_methods = std::collections::HashMap::new();

        for module in self.modules.values() {
            for decl in &module.declarations {
                if let Decl::Trait { name, methods, .. } = decl {
                    trait_methods.insert(name.clone(), methods.clone());
                }
            }
        }

        for module in self.modules.values_mut() {
            for decl in &mut module.declarations {
                if let Decl::Struct {
                    name,
                    with,
                    methods,
                    span,
                    ..
                }
                | Decl::Class {
                    name,
                    with,
                    methods,
                    span,
                    ..
                } = decl
                {
                    for trait_name in with {
                        if let Some(t_methods) = trait_methods.get(trait_name) {
                            for t_method in t_methods {
                                if let Decl::Function {
                                    name: t_m_name,
                                    body,
                                    ..
                                } = t_method
                                {
                                    // Extract actual method name (remove trait prefix)
                                    let actual_name =
                                        t_m_name.split('_').last().unwrap_or(t_m_name);

                                    // Check if class already has this method
                                    let has_method = methods.iter().any(|m| {
                                        if let Decl::Function { name: m_name, .. } = m {
                                            m_name.split('_').last().unwrap_or(m_name)
                                                == actual_name
                                        } else {
                                            false
                                        }
                                    });

                                    if !has_method {
                                        if body.statements.is_empty() {
                                            reporter.report(pace_errors::Diagnostic::error(format!("Class/Struct '{}' must implement required method '{}' from Trait '{}'", name, actual_name, trait_name))
                                            .with_span(*span));
                                        } else {
                                            let struct_name = name.clone();
                                            // Clone and inject default method
                                            let mut new_method = t_method.clone();
                                            if let Decl::Function {
                                                ref mut name,
                                                ref mut params,
                                                ..
                                            } = new_method
                                            {
                                                *name = format!("{}_{}", struct_name, actual_name);
                                                if !params.is_empty() && params[0].1 == "self" {
                                                    params[0].2 =
                                                        pace_ast::Type::Named(pace_ast::Ident {
                                                            name: struct_name,
                                                            span: *span,
                                                        });
                                                }
                                            }
                                            methods.push(new_method);
                                        }
                                    }
                                }
                            }
                        } else {
                            reporter.report(
                                pace_errors::Diagnostic::error(format!(
                                    "Trait '{}' not found",
                                    trait_name
                                ))
                                .with_span(*span),
                            );
                        }
                    }
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Decl {
    Import {
        path: Vec<String>,
        alias: Option<String>,
        span: Span,
    },
    Let {
        id: HirId,
        name: String,
        ty: Option<Type>,
        value: Option<Expr>,
        is_private: bool,
        span: Span,
    },
    Var {
        id: HirId,
        name: String,
        ty: Option<Type>,
        value: Option<Expr>,
        is_private: bool,
        span: Span,
    },
    Const {
        id: HirId,
        name: String,
        ty: Option<Type>,
        value: Expr,
        is_private: bool,
        span: Span,
    },
    Struct {
        id: HirId,
        name: String,
        generic_params: Option<Vec<(String, Option<Type>)>>,
        with: Vec<String>,
        fields: Vec<(String, Type, Option<Expr>, bool, bool)>,
        static_fields: Vec<(String, Type, Expr, bool)>,
        const_fields: Vec<(String, Type, Expr, bool)>,
        methods: Vec<Decl>, // Lowered to global functions anyway, but kept for namespacing if needed
        is_private: bool,
        span: Span,
    },
    Class {
        id: HirId,
        name: String,
        generic_params: Option<Vec<(String, Option<Type>)>>,
        extends: Option<String>,
        with: Vec<String>,
        fields: Vec<(String, Type, Option<Expr>, bool, bool)>,
        static_fields: Vec<(String, Type, Expr, bool)>,
        const_fields: Vec<(String, Type, Expr, bool)>,
        methods: Vec<Decl>,
        is_private: bool,
        span: Span,
    },
    Trait {
        id: HirId,
        name: String,
        generic_params: Option<Vec<(String, Option<Type>)>>,
        methods: Vec<Decl>,
        is_private: bool,
        span: Span,
    },
    Enum {
        id: HirId,
        name: String,
        generic_params: Option<Vec<(String, Option<Type>)>>,
        variants: Vec<EnumVariant>,
        is_private: bool,
        span: Span,
    },
    Function {
        id: HirId,
        name: String,
        generic_params: Option<Vec<(String, Option<Type>)>>,
        params: Vec<(HirId, String, Type)>,
        return_type: Option<Type>,
        body: Block,
        is_static: bool,
        is_override: bool,
        is_private: bool,
        span: Span,
    },
    Expr(Expr, Span),
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumVariant {
    pub name: String,
    pub id: HirId,
    pub fields: Option<Vec<(String, Type)>>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    IntLiteral(String, Span),
    FloatLiteral(String, Span),
    BoolLiteral(bool, Span),
    StringLiteral(String, Span),
    InterpolatedString(Vec<Expr>, Span),
    Null(Span),
    Ident(HirId, String, Option<Vec<Type>>, Span),
    Super(Span),
    Binary {
        left: Box<Expr>,
        op: BinaryOp,
        right: Box<Expr>,
        span: Span,
    },
    MemberAccess {
        object: Box<Expr>,
        member: String,
        span: Span,
    },
    OptionalMemberAccess {
        object: Box<Expr>,
        member: String,
        span: Span,
    },
    Call {
        callee: Box<Expr>,
        args: Vec<(Option<String>, Expr)>,
        span: Span,
    },
    BuiltinCall(String, Vec<Expr>, Span),
    If {
        cond: Box<Expr>,
        then_block: Block,
        else_block: Option<Block>,
        span: Span,
    },
    While {
        cond: Box<Expr>,
        body: Block,
        span: Span,
    },
    Assign {
        target: Box<Expr>,
        value: Box<Expr>,
        span: Span,
    },
    Match {
        subject: Box<Expr>,
        arms: Vec<MatchArm>,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Expr,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Ident(HirId, String, Span),
    Variant {
        name: String,
        fields: Option<Vec<(HirId, String, Span)>>,
        span: Span,
    },
    CatchAll(Span),
}

impl Expr {
    pub fn span(&self) -> Span {
        match self {
            Expr::IntLiteral(_, span) => *span,
            Expr::FloatLiteral(_, span) => *span,
            Expr::BoolLiteral(_, span) => *span,
            Expr::Null(span) => *span,
            Expr::StringLiteral(_, span) => *span,
            Expr::InterpolatedString(_, span) => *span,
            Expr::Ident(_, _, _, span) => *span,
            Expr::Super(span) => *span,
            Expr::Binary { span, .. } => *span,
            Expr::MemberAccess { span, .. } => *span,
            Expr::OptionalMemberAccess { span, .. } => *span,
            Expr::Call { span, .. } => *span,
            Expr::BuiltinCall(_, _, span) => *span,
            Expr::If { span, .. } => *span,
            Expr::While { span, .. } => *span,
            Expr::Assign { span, .. } => *span,
            Expr::Match { span, .. } => *span,
        }
    }
}
