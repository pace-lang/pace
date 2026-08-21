use ast::{TypeExpr, TypedExprKind, TypedStmt, TypedStmtKind};
use mir::{
    BasicBlock, BlockId, ForeignAbiType, ForeignFunction, Function, Inst, Place, Program, RValue,
    Terminator, Value,
};

pub mod expr;
pub mod stmt;
pub mod async_transform;
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
            !["Int", "Float", "Bool"].contains(&session.interner.borrow().lookup(*name))
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



pub(crate) fn is_ref_type_id(
    ty: session::TypeId,
    session: &session::CompilerSession,
    struct_names: &std::collections::HashSet<String>,
) -> bool {
    match session.types.borrow().get(ty) {
        session::types::Type::Int
        | session::types::Type::Float
        | session::types::Type::Bool
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
        | session::types::Type::Array(_)
        | session::types::Type::Class(..) => true,
        session::types::Type::Instance(sym) => {
            let interner = session.interner.borrow();
            let name = interner.lookup(*sym).to_string();
            if name == "String" || name.starts_with("Array") {
                true
            } else if struct_names.contains(&name) {
                false // Struct instances are not reference types!
            } else {
                true
            }
        }
        session::types::Type::Optional(inner) => is_ref_type_id(*inner, session, struct_names),
        session::types::Type::GenericInstance(_, _) => true,
        session::types::Type::Enum(..) | session::types::Type::Struct(..) => false,
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

    fn build_func(
        &mut self,
        actual_name: String,
        param_names: Vec<String>,
        ref_params: std::collections::HashSet<String>,
        returns_ref: bool,
        is_async: bool,
        enums_map: std::collections::HashMap<String, Vec<String>>,
        body: &TypedStmt,
    ) {
        if !self.program.classes.contains_key("Task") {
            let class_def = mir::ClassDef {
                name: "Task".to_string(),
                is_struct: false,
                is_actor: false,
                fields: vec!["context".to_string(), "poll_fn".to_string(), "waker".to_string(), "result".to_string()],
                static_fields: Vec::new(),
                weak_fields: std::collections::HashSet::new(),
                reference_fields: std::collections::HashSet::new(),
            };
            self.program.classes.insert("Task".to_string(), class_def);
        }
        let original_builder = MirBuilder::new(
            actual_name.clone(),
            param_names.clone(),
            ref_params.clone(),
            returns_ref,
            enums_map,
            self.session,
            self.program.classes.iter().filter_map(|(k, v)| if v.is_struct { Some(k.clone()) } else { None }).collect(),
            self.program.classes.iter().filter_map(|(k, v)| if v.is_actor { Some(k.clone()) } else { None }).collect(),
        );
        let original_mir = match &body.kind {
            TypedStmtKind::Block(stmts) => original_builder.build(stmts),
            _ => original_builder.build(std::slice::from_ref(body)),
        };

        if is_async {
            // 1. Generate Context Struct
            let context_name = format!("{}_Context", actual_name.replace("::", "_"));
            let mut ctx_fields = vec!["state".to_string(), "result".to_string()];
            ctx_fields.extend(param_names.clone());
            
            // Add temp_0 .. temp_{N-1}
            for i in 0..original_mir.temp_count {
                ctx_fields.push(format!("temp_{}", i));
            }
            
            let class_def = mir::ClassDef {
                name: context_name.clone(),
                is_struct: false, // Context is heap allocated
                is_actor: false,
                fields: ctx_fields,
                static_fields: Vec::new(),
                weak_fields: std::collections::HashSet::new(),
                reference_fields: ref_params.clone(),
            };
            self.program.classes.insert(context_name.clone(), class_def);

            // 2. Generate the state machine _poll function
            let poll_func = async_transform::lower_async_to_poll(
                &original_mir,
                &context_name,
                original_mir.temp_count,
            );
            let poll_func_name = poll_func.name.clone();
            self.program.functions.insert(poll_func_name.clone(), poll_func);

            // 3. Rewrite the original function to allocate Context and return Task
            let mut mir_func = mir::Function::new(
                actual_name.clone(),
                param_names.clone(),
                ref_params.clone(),
                true, // Returns a reference (Task)
            );
            
            let mut start_block = mir::BasicBlock::new(mir::BlockId(0));
            
            let ctx_place = mir::Place::Temp(0);
            start_block.instructions.push(mir::Inst::Assign(
                ctx_place.clone(),
                mir::RValue::AllocateObject(context_name.clone()),
            ));
            
            start_block.instructions.push(mir::Inst::SetProperty(
                mir::Value::Place(ctx_place.clone()),
                "state".to_string(),
                context_name.clone(),
                mir::Value::Int(0),
                false,
            ));
            
            let task_place = mir::Place::Temp(1);
            start_block.instructions.push(mir::Inst::Assign(
                task_place.clone(),
                mir::RValue::AllocateTask(poll_func_name),
            ));
            start_block.instructions.push(mir::Inst::SetProperty(
                mir::Value::Place(task_place.clone()),
                "context".to_string(),
                "Task".to_string(),
                mir::Value::Place(ctx_place.clone()),
                false,
            ));
            
            start_block.instructions.push(mir::Inst::Retain(mir::Value::Place(ctx_place.clone())));
            
            start_block.terminator = Some(mir::Terminator::Return(Some(mir::Value::Place(task_place))));
            mir_func.blocks.push(start_block);
            
            self.program.functions.insert(actual_name, mir_func);
        } else {
            self.program.functions.insert(actual_name, original_mir);
        }
    }

    pub fn build(mut self, statements: &[TypedStmt]) -> Program {
        let mut struct_names = std::collections::HashSet::new();
        for stmt in statements {
            if let TypedStmtKind::Struct { name, .. } = &stmt.kind {
                struct_names.insert(self.session.interner.borrow().lookup(*name).to_string());
            }
        }
        for stmt in statements {
            if let TypedStmtKind::Enum { name, variants, .. } = &stmt.kind {
                let mut variant_defs = Vec::new();
                for v in variants {
                    let mut reference_payloads = std::collections::HashSet::new();
                    let mut struct_payloads = std::collections::HashMap::new();
                    if let Some(fields) = &v.fields {
                        for (idx, field) in fields.iter().enumerate() {
                            if is_ref_type(&field.ty, self.session) {
                                reference_payloads.insert(idx);
                            }
                            if let ast::TypeExpr::Named(n) = &field.ty {
                                let n_str = self.session.interner.borrow().lookup(*n).to_string();
                                if struct_names.contains(&n_str) {
                                    struct_payloads.insert(idx, n_str);
                                }
                            }
                        }
                    }
                    variant_defs.push(mir::EnumVariantDef {
                        name: self.session.interner.borrow().lookup(v.name).to_string(),
                        reference_payloads,
                        struct_payloads,
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
                let mut static_fields = Vec::new();
                let mut weak_fields = std::collections::HashSet::new();
                let mut reference_fields = std::collections::HashSet::new();

                let is_struct = matches!(stmt.kind, TypedStmtKind::Struct { .. });
                let is_actor = match &stmt.kind {
                    TypedStmtKind::Class { is_actor, .. } => *is_actor,
                    _ => false,
                };

                for field in fields {
                    match &field.kind {
                        TypedStmtKind::Var {
                            name: f_name,
                            is_weak,
                            type_annotation,
                            is_static,
                            ..
                        } => {
                            if *is_static {
                                static_fields.push(self.session.interner.borrow().lookup(*f_name).to_string());
                                continue;
                            }
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
                            is_static,
                            ..
                        } => {
                            if *is_static {
                                static_fields.push(self.session.interner.borrow().lookup(*f_name).to_string());
                                continue;
                            }
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
                if is_actor {
                    field_names.push("__mailbox".to_string());
                }

                let class_def = mir::ClassDef {
                    name: self.session.interner.borrow().lookup(*name).to_string(),
                    is_struct,
                    is_actor,
                    fields: field_names,
                    static_fields,
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
                        is_async,
                        is_static,
                        ..
                    } = &method.kind
                    {
                        let mut param_names = Vec::new();
                        let mut ref_params = std::collections::HashSet::new();
                        if !*is_static {
                            param_names.push("self".to_string());
                            ref_params.insert("self".to_string());
                        }
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
                        self.build_func(
                            actual_name,
                            param_names,
                            ref_params,
                            returns_ref,
                            *is_async,
                            enums_map.clone(),
                            body,
                        );
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
                        is_async,
                        is_static,
                        ..
                    } = &method.kind
                    {
                        let mut param_names = Vec::new();
                        let mut ref_params = std::collections::HashSet::new();
                        if !*is_static {
                            param_names.push("self".to_string());
                            ref_params.insert("self".to_string());
                        }
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
                        self.build_func(
                            actual_name,
                            param_names,
                            ref_params,
                            returns_ref,
                            *is_async,
                            enums_map.clone(),
                            body,
                        );
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
                        is_async,
                        is_static,
                        ..
                    } = &method.kind
                    {
                        let mut param_names = Vec::new();
                        let mut ref_params = std::collections::HashSet::new();
                        if !*is_static {
                            param_names.push("self".to_string());
                            ref_params.insert("self".to_string());
                        }
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
                        self.build_func(
                            actual_name,
                            param_names,
                            ref_params,
                            returns_ref,
                            *is_async,
                            enums_map.clone(),
                            body,
                        );
                    }
                }

                self.current_class = prev_class;
            } else if let TypedStmtKind::Func {
                name,
                params,
                return_type,
                body,
                is_async,
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
                
                let _func_name_str = self.session.interner.borrow().lookup(*name).to_string();
                let returns_ref = return_type
                    .as_ref()
                    .is_some_and(|t| is_ref_type(t, self.session));

                let actual_name = self.session.interner.borrow().lookup(*name).to_string();
                self.build_func(
                    actual_name,
                    param_names,
                    ref_params,
                    returns_ref,
                    *is_async,
                    enums_map.clone(),
                    body,
                );
            } else if let TypedStmtKind::ForeignFunc {
                name,
                base_name,
                params,
                return_type,
                is_static: _,
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
                self.program.classes.iter().filter_map(|(k, v)| if v.is_struct { Some(k.clone()) } else { None }).collect(),
                self.program.classes.iter().filter_map(|(k, v)| if v.is_actor { Some(k.clone()) } else { None }).collect(),
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
    struct_names: std::collections::HashSet<String>,
    actor_names: std::collections::HashSet<String>,
}

impl<'a> MirBuilder<'a> {
    pub fn new(
        name: String,
        parameters: Vec<String>,
        weak_vars: std::collections::HashSet<String>,
        returns_reference: bool,
        enums_map: std::collections::HashMap<String, Vec<String>>,
        session: &'a session::CompilerSession,
        struct_names: std::collections::HashSet<String>,
        actor_names: std::collections::HashSet<String>,
    ) -> Self {
        let mut function = Function::new(name, parameters, weak_vars, returns_reference);
        let start_block = BlockId(0);
        function.blocks.push(BasicBlock::new(start_block));

        Self {
            session,
            function,
            current_block: start_block,
            temp_counter: 0,
            enums_map,
            struct_names,
            actor_names,
        }
    }

    pub fn get_struct_name(&self, ty: session::TypeId) -> Option<String> {
        match self.session.types.borrow().get(ty) {
            session::types::Type::Struct(sym, _) => Some(self.session.interner.borrow().lookup(*sym).to_string()),
            session::types::Type::Instance(sym) => {
                let name = self.session.interner.borrow().lookup(*sym).to_string();
                if self.struct_names.contains(&name) {
                    Some(name)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    pub fn emit_assignment(&mut self, place: Place, ty: session::TypeId, rvalue: RValue) {
        if let Some(struct_name) = self.get_struct_name(ty) {
            self.function.struct_places.insert(place.clone(), struct_name.clone());
            if let RValue::Use(val) = &rvalue {
                let inst = Inst::MemCopy(Value::Place(place), val.clone(), struct_name);
                self.current().instructions.push(inst);
                return;
            } else if let RValue::Call(..) = &rvalue {
                // For calls, we handle sret elsewhere, so we just emit assign and let sret translation handle it.
                // Wait, if it's a Call, we emit Assign.
            }
        }
        self.current().instructions.push(Inst::Assign(place, rvalue));
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
        self.function.temp_count = self.temp_counter;
        self.function
    }
}
