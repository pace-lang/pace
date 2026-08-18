use ast::{TypeExpr, TypedExprKind, TypedStmt, TypedStmtKind};
use mir::{
    BasicBlock, BlockId, ForeignAbiType, ForeignFunction, Function, Inst, Place, Program, RValue,
    Terminator, Value,
};

pub mod expr;
pub mod stmt;
#[cfg(test)]
mod tests;

pub struct ProgramBuilder<'a> {
    session: &'a session::CompilerSession,
    program: Program,
    current_class: Option<String>,
}

fn type_expr_to_abi(type_expr: &TypeExpr, session: &session::CompilerSession) -> ForeignAbiType {
    match type_expr {
        TypeExpr::Named(name) => match session.interner.borrow().lookup(*name) {
            "CInt" => ForeignAbiType::I32,
            "CUInt" => ForeignAbiType::I32,
            "CChar" => ForeignAbiType::I8,
            "CSize" => ForeignAbiType::I64,
            "Int" => ForeignAbiType::I64,
            "Float" => ForeignAbiType::F64,
            "Void" => ForeignAbiType::I64, // Not really used for params
            _ => ForeignAbiType::I64,
        },
        TypeExpr::GenericInstance(name, _)
            if session.interner.borrow().lookup(*name) == "Pointer" =>
        {
            ForeignAbiType::Pointer
        }
        _ => ForeignAbiType::I64,
    }
}

fn is_ref_type_opt(te: &Option<ast::TypeExpr>, session: &session::CompilerSession) -> bool {
    match te {
        Some(ast::TypeExpr::Named(name)) => {
            !["Int", "Float", "Boolean"].contains(&session.interner.borrow().lookup(*name))
        }
        Some(ast::TypeExpr::Optional(inner)) => is_ref_type_opt(&Some((**inner).clone()), session),
        Some(ast::TypeExpr::Array(_)) => true,
        Some(ast::TypeExpr::GenericInstance(name, _)) => {
            session.interner.borrow().lookup(*name) != "Pointer"
        }
        _ => false,
    }
}

fn is_ref_type(te: &ast::TypeExpr, session: &session::CompilerSession) -> bool {
    is_ref_type_opt(&Some(te.clone()), session)
}

pub(crate) fn is_ref_type_id(ty: session::TypeId, session: &session::CompilerSession) -> bool {
    match session.types.borrow().get(ty) {
        session::types::Type::Int
        | session::types::Type::Float
        | session::types::Type::Boolean
        | session::types::Type::Pointer(_)
        | session::types::Type::Void
        | session::types::Type::Null
        | session::types::Type::Error
        | session::types::Type::CInt
        | session::types::Type::CUInt
        | session::types::Type::CChar
        | session::types::Type::CSize
        | session::types::Type::BuiltinFunc
        | session::types::Type::Function(..)
        | session::types::Type::OverloadedFunction(..)
        | session::types::Type::EnumVariantConstructor(..)
        | session::types::Type::Any
        | session::types::Type::Range => false,
        session::types::Type::String
        | session::types::Type::Instance(_)
        | session::types::Type::Array(_)
        | session::types::Type::Class(..) => true,
        session::types::Type::Optional(inner) => is_ref_type_id(*inner, session),
        session::types::Type::GenericInstance(_, _)
        | session::types::Type::Enum(..)
        | session::types::Type::Struct(..) => true,
        _ => true,
    }
}

impl<'a> ProgramBuilder<'a> {
    pub fn new(session: &'a session::CompilerSession) -> Self {
        Self {
            session,
            program: Program::new(),
            current_class: None,
        }
    }

    pub fn build(mut self, statements: &[TypedStmt]) -> Program {
        for stmt in statements {
            if let TypedStmtKind::Enum { name, variants, .. } = &stmt.kind {
                let mut variant_defs = Vec::new();
                for v in variants {
                    let mut reference_payloads = std::collections::HashSet::new();
                    if let Some(fields) = &v.fields {
                        for (idx, field) in fields.iter().enumerate() {
                            if is_ref_type(&field.ty, self.session) {
                                reference_payloads.insert(idx);
                            }
                        }
                    }
                    variant_defs.push(mir::EnumVariantDef {
                        name: self.session.interner.borrow().lookup(v.name).to_string(),
                        reference_payloads,
                    });
                }
                self.program.enums.insert(
                    self.session.interner.borrow().lookup(*name).to_string(),
                    mir::EnumDef {
                        name: self.session.interner.borrow().lookup(*name).to_string(),
                        variants: variant_defs,
                    },
                );
            }
        }

        let mut enums_map = std::collections::HashMap::new();
        for (name, def) in &self.program.enums {
            let names: Vec<String> = def.variants.iter().map(|v| v.name.clone()).collect();
            enums_map.insert(name.clone(), names);
        }

        let mut main_stmts = Vec::new();
        for stmt in statements {
            if let TypedStmtKind::Class {
                name,
                methods,
                fields,
                ..
            }
            | TypedStmtKind::Struct {
                name,
                methods,
                fields,
                ..
            } = &stmt.kind
            {
                let mut field_names = Vec::new();
                let mut weak_fields = std::collections::HashSet::new();
                let mut reference_fields = std::collections::HashSet::new();

                let is_struct = matches!(stmt.kind, TypedStmtKind::Struct { .. });

                for field in fields {
                    match &field.kind {
                        TypedStmtKind::Var {
                            name: f_name,
                            is_weak,
                            type_annotation,
                            ..
                        } => {
                            field_names
                                .push(self.session.interner.borrow().lookup(*f_name).to_string());
                            if *is_weak {
                                weak_fields.insert(
                                    self.session.interner.borrow().lookup(*f_name).to_string(),
                                );
                            } else if is_ref_type_opt(type_annotation, self.session) {
                                reference_fields.insert(
                                    self.session.interner.borrow().lookup(*f_name).to_string(),
                                );
                            }
                        }
                        TypedStmtKind::Let {
                            name: f_name,
                            type_annotation,
                            ..
                        } => {
                            field_names
                                .push(self.session.interner.borrow().lookup(*f_name).to_string());
                            if is_ref_type_opt(type_annotation, self.session) {
                                reference_fields.insert(
                                    self.session.interner.borrow().lookup(*f_name).to_string(),
                                );
                            }
                        }
                        _ => {}
                    }
                }
                let class_def = mir::ClassDef {
                    name: self.session.interner.borrow().lookup(*name).to_string(),
                    is_struct,
                    fields: field_names,
                    weak_fields,
                    reference_fields,
                };
                self.program.classes.insert(
                    self.session.interner.borrow().lookup(*name).to_string(),
                    class_def,
                );

                let prev_class = self.current_class.clone();
                self.current_class = Some(self.session.interner.borrow().lookup(*name).to_string());

                for method in methods {
                    if let TypedStmtKind::Func {
                        name: m_name,
                        params,
                        return_type,
                        body,
                        ..
                    } = &method.kind
                    {
                        let mut param_names = vec!["self".to_string()];
                        let mut ref_params = std::collections::HashSet::new();
                        ref_params.insert("self".to_string());
                        for (p, ty) in params {
                            param_names.push(self.session.interner.borrow().lookup(*p).to_string());
                            if is_ref_type(ty, self.session) {
                                ref_params
                                    .insert(self.session.interner.borrow().lookup(*p).to_string());
                            }
                        }
                        let returns_ref = return_type
                            .as_ref()
                            .is_some_and(|t| is_ref_type(t, self.session));
                        let actual_name = format!(
                            "{}::{}",
                            self.session.interner.borrow().lookup(*name),
                            self.session.interner.borrow().lookup(*m_name)
                        );
                        let builder = MirBuilder::new(
                            actual_name.clone(),
                            param_names,
                            ref_params,
                            returns_ref,
                            enums_map.clone(),
                            self.session,
                        );
                        let mir_func = match &body.kind {
                            TypedStmtKind::Block(stmts) => builder.build(stmts),
                            _ => builder.build(std::slice::from_ref(body)),
                        };
                        self.program.functions.insert(actual_name, mir_func);
                    }
                }

                self.current_class = prev_class;
            } else if let TypedStmtKind::Enum { name, methods, .. } = &stmt.kind {
                // Enum methods are compiled just like class/struct methods
                let prev_class = self.current_class.clone();
                self.current_class = Some(self.session.interner.borrow().lookup(*name).to_string());

                for method in methods {
                    if let TypedStmtKind::Func {
                        name: m_name,
                        params,
                        return_type,
                        body,
                        ..
                    } = &method.kind
                    {
                        let mut param_names = vec!["self".to_string()];
                        let mut ref_params = std::collections::HashSet::new();
                        ref_params.insert("self".to_string());
                        for (p, ty) in params {
                            param_names.push(self.session.interner.borrow().lookup(*p).to_string());
                            if is_ref_type(ty, self.session) {
                                ref_params
                                    .insert(self.session.interner.borrow().lookup(*p).to_string());
                            }
                        }
                        let returns_ref = return_type
                            .as_ref()
                            .is_some_and(|t| is_ref_type(t, self.session));
                        let actual_name = format!(
                            "{}::{}",
                            self.session.interner.borrow().lookup(*name),
                            self.session.interner.borrow().lookup(*m_name)
                        );
                        let builder = MirBuilder::new(
                            actual_name.clone(),
                            param_names,
                            ref_params,
                            returns_ref,
                            enums_map.clone(),
                            self.session,
                        );
                        let mir_func = match &body.kind {
                            TypedStmtKind::Block(stmts) => builder.build(stmts),
                            _ => builder.build(std::slice::from_ref(body)),
                        };
                        self.program.functions.insert(actual_name, mir_func);
                    }
                }

                self.current_class = prev_class;
            } else if let TypedStmtKind::Extension {
                target_type,
                methods,
            } = &stmt.kind
            {
                let target_name_raw = self.session.format_type(*target_type);
                let target_name = target_name_raw
                    .replace("<", "_")
                    .replace(">", "")
                    .replace(" ", "")
                    .replace(",", "_");
                let prev_class = self.current_class.clone();
                self.current_class = Some(target_name.clone());

                for method in methods {
                    if let TypedStmtKind::Func {
                        name: m_name,
                        params,
                        return_type,
                        body,
                        ..
                    } = &method.kind
                    {
                        let mut param_names = vec!["self".to_string()];
                        let mut ref_params = std::collections::HashSet::new();
                        ref_params.insert("self".to_string());
                        for (p, ty) in params {
                            param_names.push(self.session.interner.borrow().lookup(*p).to_string());
                            if is_ref_type(ty, self.session) {
                                ref_params
                                    .insert(self.session.interner.borrow().lookup(*p).to_string());
                            }
                        }
                        let returns_ref = return_type
                            .as_ref()
                            .is_some_and(|t| is_ref_type(t, self.session));
                        let actual_name = format!(
                            "{}::{}",
                            target_name,
                            self.session.interner.borrow().lookup(*m_name)
                        );
                        let builder = MirBuilder::new(
                            actual_name.clone(),
                            param_names,
                            ref_params,
                            returns_ref,
                            enums_map.clone(),
                            self.session,
                        );
                        let mir_func = match &body.kind {
                            TypedStmtKind::Block(stmts) => builder.build(stmts),
                            _ => builder.build(std::slice::from_ref(body)),
                        };
                        self.program.functions.insert(actual_name, mir_func);
                    }
                }

                self.current_class = prev_class;
            } else if let TypedStmtKind::Func {
                name,
                params,
                return_type,
                body,
                ..
            } = &stmt.kind
            {
                let mut param_names = Vec::new();
                let mut ref_params = std::collections::HashSet::new();
                for (p, ty) in params {
                    param_names.push(self.session.interner.borrow().lookup(*p).to_string());
                    if is_ref_type(ty, self.session) {
                        ref_params.insert(self.session.interner.borrow().lookup(*p).to_string());
                    }
                }
                let returns_ref = return_type
                    .as_ref()
                    .is_some_and(|t| is_ref_type(t, self.session));
                let builder = MirBuilder::new(
                    self.session.interner.borrow().lookup(*name).to_string(),
                    param_names,
                    ref_params,
                    returns_ref,
                    enums_map.clone(),
                    self.session,
                );
                let mir_func = match &body.kind {
                    TypedStmtKind::Block(stmts) => builder.build(stmts),
                    _ => builder.build(std::slice::from_ref(body)),
                };
                self.program.functions.insert(
                    self.session.interner.borrow().lookup(*name).to_string(),
                    mir_func,
                );
            } else if let TypedStmtKind::ForeignFunc {
                name,
                base_name,
                params,
                return_type,
            } = &stmt.kind
            {
                let mut param_types = Vec::new();
                for (_, ty) in params {
                    param_types.push(type_expr_to_abi(ty, self.session));
                }
                let ret_ty = return_type
                    .as_ref()
                    .map(|t| type_expr_to_abi(t, self.session));
                let symbol = self
                    .session
                    .interner
                    .borrow()
                    .lookup(*base_name)
                    .to_string();

                self.program.foreign_functions.insert(
                    self.session.interner.borrow().lookup(*name).to_string(),
                    ForeignFunction {
                        name: self.session.interner.borrow().lookup(*name).to_string(),
                        symbol,
                        param_types,
                        return_type: ret_ty,
                    },
                );
            } else if let TypedStmtKind::Interface { .. } = &stmt.kind {
                // Ignore interface declarations in MIR, as they are fully erased
                // and used strictly for compile-time type checking.
            } else if let TypedStmtKind::Enum { .. } = &stmt.kind {
                // Already collected in pre-pass
            } else if let TypedStmtKind::Block(stmts) = &stmt.kind {
                if !stmts.is_empty() {
                    main_stmts.push(stmt.clone());
                }
            } else {
                main_stmts.push(stmt.clone());
            }
        }
        if !main_stmts.is_empty() {
            let builder = MirBuilder::new(
                "main".into(),
                vec![],
                std::collections::HashSet::new(),
                false,
                enums_map.clone(),
                self.session,
            );
            let main_func = builder.build(&main_stmts);
            self.program.functions.insert("main".into(), main_func);
        }

        self.program
    }
}

pub struct MirBuilder<'a> {
    session: &'a session::CompilerSession,
    function: Function,
    current_block: BlockId,
    temp_counter: usize,
    enums_map: std::collections::HashMap<String, Vec<String>>,
}

impl<'a> MirBuilder<'a> {
    pub fn new(
        name: String,
        parameters: Vec<String>,
        reference_parameters: std::collections::HashSet<String>,
        returns_reference: bool,
        enums_map: std::collections::HashMap<String, Vec<String>>,
        session: &'a session::CompilerSession,
    ) -> Self {
        let mut function = Function::new(name, parameters, reference_parameters, returns_reference);
        let start_block = BlockId(0);
        function.blocks.push(BasicBlock::new(start_block));

        Self {
            session,
            function,
            current_block: start_block,
            temp_counter: 0,
            enums_map,
        }
    }

    fn new_block(&mut self) -> BlockId {
        let id = BlockId(self.function.blocks.len());
        self.function.blocks.push(BasicBlock::new(id));
        id
    }

    fn new_temp(&mut self) -> Place {
        let id = self.temp_counter;
        self.temp_counter += 1;
        Place::Temp(id)
    }

    fn current(&mut self) -> &mut BasicBlock {
        let id = self.current_block.0;
        &mut self.function.blocks[id]
    }

    pub fn build(mut self, statements: &[TypedStmt]) -> Function {
        for stmt in statements {
            self.lower_stmt(stmt);
        }
        if self.current().terminator.is_none() {
            self.current().terminator = Some(Terminator::Return(None));
        }
        self.function
    }
}
