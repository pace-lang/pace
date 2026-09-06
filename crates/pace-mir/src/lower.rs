use crate::basic_block::{BasicBlock, BasicBlockData, SwitchTargets, Terminator};
use crate::body::{LocalDecl, LocalKind, MirBody, Mutability};
use crate::statement::{AggregateKind, Constant, Local, Operand, Place, ProjectionElem, Rvalue, Statement};
use pace_ast::BinaryOp;
use pace_hir::{Expr, ExprId, HirArena, Stmt, StmtId};
use pace_ty::{Environment, Type};
use std::collections::HashMap;
use ustr::Ustr;

pub struct MirProgram {
    pub functions: HashMap<Ustr, MirBody>,
    // Maps a concrete Class name (e.g. SetIterator_Int) to a flat array of method function names, indexed by the global interface method index
    pub vtables: HashMap<Ustr, Vec<Option<Ustr>>>,
}

pub struct MirBuilder<'a> {
    arena: &'a HirArena,
    env: &'a Environment,
    class_layouts: HashMap<Ustr, HashMap<Ustr, usize>>,
}

impl<'a> MirBuilder<'a> {
    pub fn new(arena: &'a HirArena, env: &'a Environment) -> Self {
        Self { arena, env, class_layouts: HashMap::new() }
    }

    pub fn build(mut self, stmts: &[StmtId]) -> MirProgram {
        let mut program = MirProgram { functions: HashMap::new(), vtables: HashMap::new() };
        
        // Compute VTables for every class
        let vtable_size = self.env.interface_method_count;
        for (class_name, sig) in &self.env.classes {
            let mut vtable = vec![None; vtable_size];
            if let Some(Type::Interface(iface_name)) = &sig.implements {
                if let Some(iface_sig) = self.env.interfaces.get(iface_name) {
                    for (method_name, _) in &iface_sig.methods {
                        let global_name = format!("{}_{}", iface_name.as_str(), method_name.as_str());
                        if let Some(index) = self.env.get_global_interface_method_index(&ustr::Ustr::from(global_name.as_str())) {
                            if sig.methods.contains_key(method_name) {
                                let class_method_name = format!("{}_{}", class_name.as_str(), method_name.as_str());
                                vtable[index] = Some(ustr::Ustr::from(class_method_name.as_str()));
                            }
                        }
                    }
                }
            }
            while vtable.last() == Some(&None) {
                vtable.pop();
            }
            program.vtables.insert(*class_name, vtable);
        }
        
        // First pass: collect class layouts (including nested modules)
        for &stmt_id in stmts {
            self.collect_layouts(stmt_id);
        }
        
        // Second pass: lower functions and methods
        for &stmt_id in stmts {
            self.lower_item(stmt_id, &mut program);
        }
        
        program
    }

    fn collect_layouts(&mut self, stmt_id: StmtId) {
        let stmt = self.arena.get_stmt(stmt_id);
        if let Stmt::Module { body, .. } = stmt {
            for &item_id in body {
                self.collect_layouts(item_id);
            }
        } else if let Stmt::ClassDecl { name, fields, .. } | Stmt::ActorDecl { name, fields, .. } | Stmt::StructDecl { name, fields, .. } = stmt {
            let mut field_offsets = HashMap::new();
            let mut offset = 16; // Offset 0 is RC, Offset 8 is VTable ptr
            for &field_id in fields {
                if let Stmt::VarDecl { name: field_name, .. } = self.arena.get_stmt(field_id) {
                    field_offsets.insert(*field_name, offset);
                    offset += 8;
                }
            }
            self.class_layouts.insert(*name, field_offsets);
        }
    }

    fn lower_item(&mut self, stmt_id: StmtId, program: &mut MirProgram) {
        let stmt = self.arena.get_stmt(stmt_id);
        if let Stmt::Module { body, .. } = stmt {
            for &item_id in body {
                self.lower_item(item_id, program);
            }
        } else if let Stmt::FuncDecl { name, body: func_body, params, is_extern, generic_params, .. } = stmt {
            if generic_params.is_some() && !generic_params.as_ref().unwrap().is_empty() {
                return;
            }
            let func_builder = FuncMirBuilder::new(self.arena, self.env, &self.class_layouts, *name, params, *is_extern);
            let (mir_body, closures) = func_builder.build(func_body);
            program.functions.insert(*name, mir_body);
            for closure in closures {
                program.functions.insert(closure.name, closure);
            }
        } else if let Stmt::ClassDecl { name: class_name, methods, generic_params, .. } | Stmt::ActorDecl { name: class_name, methods, generic_params, .. } = stmt {
            if generic_params.is_some() && !generic_params.as_ref().unwrap().is_empty() {
                return;
            }
            for &method_id in methods {
                if let Stmt::FuncDecl { name, body: func_body, params, is_extern, is_static, .. } = self.arena.get_stmt(method_id) {
                    let mut method_params = Vec::new();
                    if !is_static {
                        method_params.push(pace_hir::Param {
                            name: ustr::Ustr::from("self"),
                            type_annotation: pace_ast::TypeAnnotation {
                                module_prefix: None,
                                name: *class_name,
                                args: vec![],
                                is_nullable: false,
                                is_function: false,
                                function_params: None,
                                function_return: None,
                            },
                        });
                    }
                    method_params.extend(params.iter().cloned());
                    let func_builder = FuncMirBuilder::new(self.arena, self.env, &self.class_layouts, *name, &method_params, *is_extern);
                    let (mir_body, closures) = func_builder.build(func_body);
                    let mangled_name = ustr::Ustr::from(&format!("{}_{}", class_name, name));
                    // The function body itself doesn't know its mangled name, so we must set it.
                    let mut final_body = mir_body;
                    final_body.name = mangled_name;
                    program.functions.insert(mangled_name, final_body);
                    for closure in closures {
                        program.functions.insert(closure.name, closure);
                    }
                }
            }
        }
    }
}

struct FuncMirBuilder<'a> {
    arena: &'a HirArena,
    env: &'a Environment,
    class_layouts: &'a HashMap<Ustr, HashMap<Ustr, usize>>,
    body: MirBody,
    current_block: BasicBlock,
    var_map: HashMap<Ustr, Local>,
    pending_closures: Vec<MirBody>,
}

impl<'a> FuncMirBuilder<'a> {
    pub fn new(arena: &'a HirArena, env: &'a Environment, class_layouts: &'a HashMap<Ustr, HashMap<Ustr, usize>>, name: Ustr, params: &[pace_hir::Param], is_extern: bool) -> Self {
        let mut body = MirBody::new(name, params.len(), is_extern);
        // Block 0 is the entry block
        body.basic_blocks.push(BasicBlockData::new());
        // Local 0 is the return pointer
        body.local_decls.push(LocalDecl {
            ty: Type::Unknown, // Will be updated later
            mutability: Mutability::Mut,
            kind: LocalKind::ReturnPointer,
            source_info: pace_span::Span::default(),
        });
        
        let mut var_map = HashMap::new();
        // Add params
        for param in params {
            let local = Local(body.local_decls.len());
            body.local_decls.push(LocalDecl {
                ty: Type::Unknown,
                mutability: Mutability::Not,
                kind: LocalKind::User(param.name),
                source_info: pace_span::Span::default(),
            });
            var_map.insert(param.name, local);
        }
        
        Self {
            arena,
            env,
            class_layouts,
            body,
            current_block: BasicBlock(0),
            var_map,
            pending_closures: Vec::new(),
        }
    }

    pub fn build(mut self, stmts: &[StmtId]) -> (MirBody, Vec<MirBody>) {
        for &stmt_id in stmts {
            self.lower_stmt(stmt_id);
        }
        
        // If the last block doesn't have a terminator, add a Return terminator
        if self.body.basic_blocks[self.current_block.0].terminator.is_none() {
            self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Return);
        }
        
        (self.body, self.pending_closures)
    }

    fn new_local(&mut self, ty: Type, mutability: Mutability, kind: LocalKind, span: pace_span::Span) -> Local {
        let local = Local(self.body.local_decls.len());
        self.body.local_decls.push(LocalDecl {
            ty,
            mutability,
            kind,
            source_info: span,
        });
        local
    }

    fn new_temp(&mut self, ty: Type) -> Local {
        self.new_local(ty, Mutability::Not, LocalKind::Temp, pace_span::Span::default())
    }

    fn new_block(&mut self) -> BasicBlock {
        let block = BasicBlock(self.body.basic_blocks.len());
        self.body.basic_blocks.push(BasicBlockData::new());
        block
    }

    fn push_statement(&mut self, stmt: Statement) {
        self.body.basic_blocks[self.current_block.0].statements.push(stmt);
    }

    fn lower_stmt(&mut self, stmt_id: StmtId) {
        let stmt = self.arena.get_stmt(stmt_id);
        let span = self.arena.get_stmt_span(stmt_id);
        
        match stmt {
            Stmt::Expr(expr_id) => {
                let _ = self.lower_expr(*expr_id);
            }
            Stmt::VarDecl { name, initializer, .. } => {
                let ty = if let Some(init_id) = initializer {
                    self.env.node_types.get(init_id).unwrap_or(&Type::Unknown).clone()
                } else {
                    Type::Unknown
                };
                let local = self.new_local(ty, Mutability::Mut, LocalKind::User(*name), span);
                self.var_map.insert(*name, local);
                
                if let Some(init_id) = initializer {
                    let operand = self.lower_expr(*init_id);
                    self.push_statement(Statement::Assign(Place::new_local(local), Rvalue::Use(operand)));
                }
            }
            Stmt::Block(stmts) => {
                for s in stmts {
                    self.lower_stmt(*s);
                }
            }
            Stmt::Return(expr_opt) => {
                if let Some(expr_id) = expr_opt {
                    let operand = self.lower_expr(*expr_id);
                    let ret_place = Place::new_local(Local(0));
                    self.push_statement(Statement::Assign(ret_place, Rvalue::Use(operand)));
                }
                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Return);
                self.current_block = self.new_block(); // Dead code after return goes here
            }
            Stmt::If { condition, then_branch, else_branch } => {
                let cond_op = self.lower_expr(*condition);
                let then_block = self.new_block();
                let else_block = self.new_block();
                let merge_block = self.new_block();
                
                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::SwitchInt {
                    discr: cond_op,
                    targets: SwitchTargets::new(vec![0], vec![else_block, then_block]), // If false (0) -> else, default -> then
                });
                
                self.current_block = then_block;
                self.lower_stmt(*then_branch);
                if self.body.basic_blocks[self.current_block.0].terminator.is_none() {
                    self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Goto { target: merge_block });
                }
                
                self.current_block = else_block;
                if let Some(eb) = else_branch {
                    self.lower_stmt(*eb);
                }
                if self.body.basic_blocks[self.current_block.0].terminator.is_none() {
                    self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Goto { target: merge_block });
                }
                
                self.current_block = merge_block;
            }
            Stmt::While { condition, body } => {
                let cond_block = self.new_block();
                let body_block = self.new_block();
                let exit_block = self.new_block();
                
                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Goto { target: cond_block });
                
                self.current_block = cond_block;
                let cond_op = self.lower_expr(*condition);
                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::SwitchInt {
                    discr: cond_op,
                    targets: SwitchTargets::new(vec![0], vec![exit_block, body_block]),
                });
                
                self.current_block = body_block;
                self.lower_stmt(*body);
                if self.body.basic_blocks[self.current_block.0].terminator.is_none() {
                    self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Goto { target: cond_block });
                }
                
                self.current_block = exit_block;
            }
            _ => {}
        }
    }

    fn lower_expr(&mut self, expr_id: ExprId) -> Operand {
        let expr = self.arena.get_expr(expr_id);
        
        match expr {
            Expr::IntLiteral(v) => Operand::Constant(Constant::Int(*v)),
            Expr::FloatLiteral(v) => Operand::Constant(Constant::Float(*v)),
            Expr::BoolLiteral(v) => Operand::Constant(Constant::Bool(*v)),
            Expr::StringLiteral(v) => Operand::Constant(Constant::String(v.to_string())),
            Expr::ArrayLiteral(elements) => {
                let ty = self.env.node_types.get(&expr_id).unwrap_or(&Type::Unknown).clone();
                let ty_str = format!("{:?}", ty);
                let is_set = ty_str.contains("Set") && !ty_str.contains("TreeSet");
                let is_treeset = ty_str.contains("TreeSet");
                let is_queue = ty_str.contains("Queue");
                
                let mut base_name = if is_set { 
                    "pace_collections_set__Set".to_string() 
                } else if is_treeset {
                    "pace_collections_tree_set__TreeSet".to_string()
                } else if is_queue {
                    "pace_collections_queue__Queue".to_string()
                } else { 
                    "pace_collections_list__List".to_string() 
                };
                if let Type::GenericInstance { args, .. } = &ty {
                    for arg in args {
                        let arg_name = format!("{:?}", arg);
                        base_name.push('_');
                        base_name.push_str(&arg_name.replace(" ", "_"));
                    }
                } else if let Type::Class(name) = &ty {
                    base_name = name.as_str().to_string();
                }
                
                let class_name = ustr::Ustr::from(base_name.as_str());
                let init_name = ustr::Ustr::from(format!("{}_init", base_name).as_str());
                let add_single_name = if is_queue {
                    ustr::Ustr::from(format!("{}_enqueue", base_name).as_str())
                } else {
                    ustr::Ustr::from(format!("{}_add", base_name).as_str())
                };
                
                let col_temp = self.new_temp(ty.clone());
                // Size calculations for stack allocation
                let size = if is_set || is_treeset { 
                    24 // 1 field
                } else if is_queue {
                    56 // 5 fields
                } else { 
                    40 // List has 3 fields
                };
                
                self.push_statement(Statement::Assign(
                    Place::new_local(col_temp),
                    Rvalue::Aggregate(AggregateKind::Class(class_name, size), vec![])
                ));
                
                let init_temp = self.new_temp(Type::Unknown);
                let init_block = self.new_block();
                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                    func: Operand::Constant(Constant::Function(init_name)),
                    args: vec![Operand::Copy(Place::new_local(col_temp))],
                    destination: Place::new_local(init_temp),
                    target: Some(init_block),
                    cleanup: None,
                });
                self.current_block = init_block;
                
                for elem_id in elements {
                    let elem_op = self.lower_expr(*elem_id);
                    let add_temp = self.new_temp(Type::Unknown);
                    let add_block = self.new_block();
                    self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                        func: Operand::Constant(Constant::Function(add_single_name)),
                        args: vec![Operand::Copy(Place::new_local(col_temp)), elem_op],
                        destination: Place::new_local(add_temp),
                        target: Some(add_block),
                        cleanup: None,
                    });
                    self.current_block = add_block;
                }
                
                Operand::Copy(Place::new_local(col_temp))
            }
            Expr::MapLiteral(elements) => {
                let ty = self.env.node_types.get(&expr_id).unwrap_or(&Type::Unknown).clone();
                let mut base_name = "pace_collections_map__Map".to_string();
                if let Type::GenericInstance { args, .. } = &ty {
                    for arg in args {
                        let arg_name = format!("{:?}", arg);
                        base_name.push('_');
                        base_name.push_str(&arg_name.replace(" ", "_"));
                    }
                } else if let Type::Class(name) = &ty {
                    base_name = name.as_str().to_string();
                }
                
                let class_name = ustr::Ustr::from(base_name.as_str());
                let init_name = ustr::Ustr::from(format!("{}_init", base_name).as_str());
                let set_name = ustr::Ustr::from(format!("{}_set", base_name).as_str());
                let col_temp = self.new_temp(ty.clone());
                // Map has 5 Int fields (keysPtr, valsPtr, statesPtr, count, cap) = 16 + 40 = 56.
                let size = 56;
                
                self.push_statement(Statement::Assign(
                    Place::new_local(col_temp),
                    Rvalue::Aggregate(AggregateKind::Class(class_name, size), vec![])
                ));
                
                let init_temp = self.new_temp(Type::Unknown);
                let init_block = self.new_block();
                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                    func: Operand::Constant(Constant::Function(init_name)),
                    args: vec![Operand::Copy(Place::new_local(col_temp))],
                    destination: Place::new_local(init_temp),
                    target: Some(init_block),
                    cleanup: None,
                });
                self.current_block = init_block;
                
                for (k_id, v_id) in elements {
                    let k_op = self.lower_expr(*k_id);
                    let v_op = self.lower_expr(*v_id);
                    let add_temp = self.new_temp(Type::Unknown);
                    let add_block = self.new_block();
                    self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                        func: Operand::Constant(Constant::Function(set_name)),
                        args: vec![Operand::Copy(Place::new_local(col_temp)), k_op, v_op],
                        destination: Place::new_local(add_temp),
                        target: Some(add_block),
                        cleanup: None,
                    });
                    self.current_block = add_block;
                }
                
                Operand::Copy(Place::new_local(col_temp))
            }
            Expr::InterpolatedString(parts) => {
                if parts.is_empty() {
                    return Operand::Constant(Constant::String("".to_string()));
                }
                
                // let sb = StringBuilder()
                let sb_temp = self.new_temp(Type::Class(ustr::Ustr::from("StringBuilder")));
                let class_name = ustr::Ustr::from("StringBuilder");
                // StringBuilder has 3 fields (buffer, capacity, length), so size is 16 + 3*8 = 40
                self.push_statement(Statement::Assign(
                    Place::new_local(sb_temp),
                    Rvalue::Aggregate(AggregateKind::Class(class_name, 40), vec![])
                ));
                
                // sb.init()
                let init_temp = self.new_temp(Type::Unknown);
                let init_block = self.new_block();
                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                    func: Operand::Constant(Constant::Function(ustr::Ustr::from("StringBuilder_init"))),
                    args: vec![Operand::Copy(Place::new_local(sb_temp))],
                    destination: Place::new_local(init_temp),
                    target: Some(init_block),
                    cleanup: None,
                });
                self.current_block = init_block;
                
                for &part_id in parts {
                    let mut part_op = self.lower_expr(part_id);
                    let part_ty = self.env.node_types.get(&part_id).unwrap_or(&Type::Unknown);
                    
                    part_op = match part_ty {
                        Type::Int => {
                            let temp = self.new_temp(Type::Unknown);
                            let next_block = self.new_block();
                            self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                                func: Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_int_to_string"))),
                                args: vec![part_op],
                                destination: Place::new_local(temp),
                                target: Some(next_block),
                                cleanup: None,
                            });
                            self.current_block = next_block;
                            Operand::Copy(Place::new_local(temp))
                        }
                        Type::Float => {
                            let temp = self.new_temp(Type::Unknown);
                            let next_block = self.new_block();
                            self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                                func: Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_float_to_string"))),
                                args: vec![part_op],
                                destination: Place::new_local(temp),
                                target: Some(next_block),
                                cleanup: None,
                            });
                            self.current_block = next_block;
                            Operand::Copy(Place::new_local(temp))
                        }
                        Type::Bool => {
                            let temp = self.new_temp(Type::Unknown);
                            let next_block = self.new_block();
                            self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                                func: Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_bool_to_string"))),
                                args: vec![part_op],
                                destination: Place::new_local(temp),
                                target: Some(next_block),
                                cleanup: None,
                            });
                            self.current_block = next_block;
                            Operand::Copy(Place::new_local(temp))
                        }
                        _ => part_op,
                    };
                    
                    // sb.append(part_op)
                    let append_temp = self.new_temp(Type::Unknown);
                    let append_block = self.new_block();
                    self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                        func: Operand::Constant(Constant::Function(ustr::Ustr::from("StringBuilder_append"))),
                        args: vec![Operand::Copy(Place::new_local(sb_temp)), part_op],
                        destination: Place::new_local(append_temp),
                        target: Some(append_block),
                        cleanup: None,
                    });
                    self.current_block = append_block;
                }
                
                // sb.build()
                let build_temp = self.new_temp(Type::String);
                let build_block = self.new_block();
                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                    func: Operand::Constant(Constant::Function(ustr::Ustr::from("StringBuilder_build"))),
                    args: vec![Operand::Copy(Place::new_local(sb_temp))],
                    destination: Place::new_local(build_temp),
                    target: Some(build_block),
                    cleanup: None,
                });
                self.current_block = build_block;
                
                Operand::Copy(Place::new_local(build_temp))
            }
            Expr::Identifier(name) => {
                if let Some(&local) = self.var_map.get(name) {
                    Operand::Copy(Place::new_local(local))
                } else {
                    Operand::Constant(Constant::Function(*name))
                }
            }
            Expr::Binary { left, op, right } => {
                let left_op = self.lower_expr(*left);
                let right_op = self.lower_expr(*right);
                
                let left_ty = self.env.node_types.get(left).unwrap_or(&Type::Unknown);
                let right_ty = self.env.node_types.get(right).unwrap_or(&Type::Unknown);
                
                if matches!(op, pace_ast::BinaryOp::Add) && (left_ty == &Type::String || right_ty == &Type::String) {
                    let sb_temp = self.new_temp(Type::Class(ustr::Ustr::from("StringBuilder")));
                    let class_name = ustr::Ustr::from("StringBuilder");
                    
                    self.push_statement(Statement::Assign(
                        Place::new_local(sb_temp),
                        Rvalue::Aggregate(AggregateKind::Class(class_name, 40), vec![])
                    ));
                    
                    let init_temp = self.new_temp(Type::Unknown);
                    let init_block = self.new_block();
                    self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                        func: Operand::Constant(Constant::Function(ustr::Ustr::from("StringBuilder_init"))),
                        args: vec![Operand::Copy(Place::new_local(sb_temp))],
                        destination: Place::new_local(init_temp),
                        target: Some(init_block),
                        cleanup: None,
                    });
                    self.current_block = init_block;
                    
                    let mut append_part = |mut part_op: Operand, ty: &Type| {
                        part_op = match ty {
                            Type::Int => {
                                let temp = self.new_temp(Type::Unknown);
                                let next_block = self.new_block();
                                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                                    func: Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_int_to_string"))),
                                    args: vec![part_op],
                                    destination: Place::new_local(temp),
                                    target: Some(next_block),
                                    cleanup: None,
                                });
                                self.current_block = next_block;
                                Operand::Copy(Place::new_local(temp))
                            }
                            Type::Float => {
                                let temp = self.new_temp(Type::Unknown);
                                let next_block = self.new_block();
                                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                                    func: Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_float_to_string"))),
                                    args: vec![part_op],
                                    destination: Place::new_local(temp),
                                    target: Some(next_block),
                                    cleanup: None,
                                });
                                self.current_block = next_block;
                                Operand::Copy(Place::new_local(temp))
                            }
                            Type::Bool => {
                                let temp = self.new_temp(Type::Unknown);
                                let next_block = self.new_block();
                                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                                    func: Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_bool_to_string"))),
                                    args: vec![part_op],
                                    destination: Place::new_local(temp),
                                    target: Some(next_block),
                                    cleanup: None,
                                });
                                self.current_block = next_block;
                                Operand::Copy(Place::new_local(temp))
                            }
                            _ => part_op,
                        };
                        
                        let append_temp = self.new_temp(Type::Unknown);
                        let append_block = self.new_block();
                        self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                            func: Operand::Constant(Constant::Function(ustr::Ustr::from("StringBuilder_append"))),
                            args: vec![Operand::Copy(Place::new_local(sb_temp)), part_op],
                            destination: Place::new_local(append_temp),
                            target: Some(append_block),
                            cleanup: None,
                        });
                        self.current_block = append_block;
                    };
                    
                    append_part(left_op, left_ty);
                    append_part(right_op, right_ty);
                    
                    let build_temp = self.new_temp(Type::String);
                    let build_block = self.new_block();
                    self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                        func: Operand::Constant(Constant::Function(ustr::Ustr::from("StringBuilder_build"))),
                        args: vec![Operand::Copy(Place::new_local(sb_temp))],
                        destination: Place::new_local(build_temp),
                        target: Some(build_block),
                        cleanup: None,
                    });
                    self.current_block = build_block;
                    
                    Operand::Copy(Place::new_local(build_temp))
                } else {
                    let temp = self.new_temp(Type::Unknown);
                    self.push_statement(Statement::Assign(
                        Place::new_local(temp),
                        Rvalue::BinaryOp(op.clone(), left_op, right_op)
                    ));
                    Operand::Copy(Place::new_local(temp))
                }
            }
            Expr::Assign { target, value } => {
                let val_op = self.lower_expr(*value);
                if let Some(place) = self.lower_place(*target) {
                    self.push_statement(Statement::Assign(place.clone(), Rvalue::Use(val_op)));
                    Operand::Copy(place)
                } else {
                    Operand::Constant(Constant::Null)
                }
            }
            Expr::MemberAccess { .. } => {
                if let Some(place) = self.lower_place(expr_id) {
                    Operand::Copy(place)
                } else {
                    Operand::Constant(Constant::Null)
                }
            }
            Expr::Call { callee, args } => {
                let mut arg_ops = Vec::new();
                for arg in args {
                    arg_ops.push(self.lower_expr(*arg));
                }

                // Check if callee is a MemberAccess (method call)
                if let Expr::MemberAccess { object, property, computed_class: _, .. } = self.arena.get_expr(*callee) {
                    let obj_ty = self.env.node_types.get(object);
                    
                    if let Some(Type::Enum(enum_name)) = obj_ty {
                        let temp = self.new_temp(Type::Unknown);
                        self.push_statement(Statement::Assign(
                            Place::new_local(temp),
                            Rvalue::Aggregate(AggregateKind::EnumVariant(*enum_name, *property, 0), arg_ops)
                        ));
                        return Operand::Copy(Place::new_local(temp));
                    }

                    if property.as_str() == "toString" {
                        let primitive_to_string = match obj_ty {
                            Some(Type::Int) => Some("__pace_int_to_string"),
                            Some(Type::Float) => Some("__pace_float_to_string"),
                            Some(Type::Bool) => Some("__pace_bool_to_string"),
                            _ => None,
                        };
                        
                        if let Some(func_name) = primitive_to_string {
                            let obj_op = self.lower_expr(*object);
                            let func_op = Operand::Constant(Constant::Function(ustr::Ustr::from(func_name)));
                            
                            let temp = self.new_temp(Type::String);
                            let next_block = self.new_block();
                            self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                                func: func_op,
                                args: vec![obj_op],
                                destination: Place::new_local(temp),
                                target: Some(next_block),
                                cleanup: None,
                            });
                            self.current_block = next_block;
                            return Operand::Copy(Place::new_local(temp));
                        } else if let Some(Type::String) = obj_ty {
                            return self.lower_expr(*object); // String.toString() is just the string itself
                        }
                    }

                    let mut is_interface = false;
                    let class_name_str = match obj_ty {
                        Some(Type::Class(name)) | Some(Type::Actor(name)) | Some(Type::Struct(name)) => name.to_string(),
                        Some(Type::Interface(name)) => { is_interface = true; name.to_string() },
                        Some(Type::GenericInstance { base, args }) => {
                            let mut base_name = if let Type::Interface(name) = &**base {
                                is_interface = true;
                                name.to_string()
                            } else if let Type::Class(name) | Type::Struct(name) | Type::Actor(name) = &**base {
                                name.to_string()
                            } else {
                                "Unknown".to_string()
                            };
                            for arg in args {
                                let arg_name = format!("{:?}", arg);
                                base_name.push('_');
                                base_name.push_str(&arg_name.replace(" ", "_"));
                            }
                            base_name
                        }
                        _ => {
                            let mut is_static = false;
                            if let Expr::Identifier(name) = self.arena.get_expr(*object) {
                                if self.env.classes.contains_key(name) || self.env.structs.contains_key(name) || self.env.enums.contains_key(name) || self.env.actors.contains_key(name) {
                                    is_static = !self.env.is_local(*name);
                                }
                            }
                            if is_static {
                                if let Expr::Identifier(name) = self.arena.get_expr(*object) {
                                    name.to_string()
                                } else {
                                    "Unknown".to_string()
                                }
                            } else {
                                "Unknown".to_string()
                            }
                        }
                    };
                    
                    let method_name = format!("{}_{}", class_name_str, property);
                    let _class_name = ustr::Ustr::from(class_name_str.as_str());
                    
                    let temp = self.new_temp(Type::Unknown);
                    let next_block = self.new_block();

                    let mut is_static_operator = false;
                    let mut base_ident = None;
                    if let Expr::Identifier(name) = self.arena.get_expr(*object) {
                        base_ident = Some(*name);
                    } else if let Expr::GenericInstantiation { callee, .. } = self.arena.get_expr(*object) {
                        if let Expr::Identifier(name) = self.arena.get_expr(*callee) {
                            base_ident = Some(*name);
                        }
                    }
                    if let Some(name) = base_ident {
                        if self.env.classes.contains_key(&name) || self.env.structs.contains_key(&name) || self.env.enums.contains_key(&name) || self.env.actors.contains_key(&name) {
                            is_static_operator = !self.env.is_local(name);
                        }
                    }

                    if is_interface && !is_static_operator {
                        let method_ustr = ustr::Ustr::from(&method_name);
                        let method_index = self.env.get_global_interface_method_index(&method_ustr).unwrap_or(0);
                        
                        let obj_op = self.lower_expr(*object);
                        arg_ops.insert(0, obj_op.clone());
                        
                        self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::InterfaceCall {
                            obj: obj_op,
                            method_index,
                            args: arg_ops,
                            destination: Place::new_local(temp),
                            target: Some(next_block),
                            cleanup: None,
                        });
                    } else {
                        let func_op = Operand::Constant(Constant::Function(ustr::Ustr::from(&method_name)));
                        
                        // If it's not a static call, the first argument must be the object (self)!
                        if !is_static_operator {
                            let obj_op = self.lower_expr(*object);
                            arg_ops.insert(0, obj_op);
                        }
                        
                        self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                            func: func_op,
                            args: arg_ops,
                            destination: Place::new_local(temp),
                            target: Some(next_block),
                            cleanup: None,
                        });
                    }
                    
                    self.current_block = next_block;
                    return Operand::Copy(Place::new_local(temp));
                }

                // Check if it's a Class/Actor/Struct initialization
                let mut is_instantiation = false;
                let mut base_class_name = None;
                
                let callee_ty = self.env.node_types.get(callee).unwrap_or(&Type::Unknown);
                match callee_ty {
                    Type::Class(name) | Type::Struct(name) | Type::Actor(name) => {
                        is_instantiation = true;
                        base_class_name = Some(name);
                    }
                    Type::GenericInstance { base, args: _ } => {
                        if let Type::Class(name) | Type::Struct(name) | Type::Actor(name) = &**base {
                            is_instantiation = true;
                            base_class_name = Some(name);
                        }
                    }
                    _ => {}
                }

                if is_instantiation {
                    if let Some(class_name) = base_class_name {
                        let field_count = if let Some(sig) = self.env.classes.get(class_name) {
                            sig.fields.len()
                        } else if let Some(sig) = self.env.actors.get(class_name) {
                            sig.fields.len()
                        } else if let Some(sig) = self.env.structs.get(class_name) {
                            sig.fields.len()
                        } else {
                            0
                        };
                        let class_size = 16 + field_count * 8;
                        
                        let temp = self.new_temp(Type::Unknown);
                        self.push_statement(Statement::Assign(
                            Place::new_local(temp),
                            Rvalue::Aggregate(AggregateKind::Class(*class_name, class_size), vec![])
                        ));
                        
                        let has_init = if let Some(sig) = self.env.classes.get(class_name) {
                            sig.methods.contains_key(&ustr::Ustr::from("init"))
                        } else if let Some(sig) = self.env.actors.get(class_name) {
                            sig.methods.contains_key(&ustr::Ustr::from("init"))
                        } else if let Some(sig) = self.env.structs.get(class_name) {
                            sig.methods.contains_key(&ustr::Ustr::from("init"))
                        } else {
                            false
                        };

                        if has_init {
                            let init_method_name = format!("{}_init", class_name);
                            let init_func_op = Operand::Constant(Constant::Function(ustr::Ustr::from(&init_method_name)));
                            
                            let mut init_args = arg_ops.clone();
                            init_args.insert(0, Operand::Copy(Place::new_local(temp))); // pass self
                            
                            let init_temp = self.new_temp(Type::Unknown);
                            let next_block = self.new_block();
                            self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                                func: init_func_op,
                                args: init_args,
                                destination: Place::new_local(init_temp),
                                target: Some(next_block),
                                cleanup: None,
                            });
                            self.current_block = next_block;
                        } else {
                            // If there is no init method, we should populate the fields directly from arguments
                            let last_stmt_idx = self.body.basic_blocks[self.current_block.0].statements.len() - 1;
                            self.body.basic_blocks[self.current_block.0].statements[last_stmt_idx] = Statement::Assign(
                                Place::new_local(temp),
                                Rvalue::Aggregate(AggregateKind::Class(*class_name, class_size), arg_ops)
                            );
                        }
                        
                        return Operand::Copy(Place::new_local(temp));
                    }
                }
                let mut callee_op = self.lower_expr(*callee);
                
                if let Expr::Identifier(name) = self.arena.get_expr(*callee) {
                    match name.as_str() {
                        "print" => {
                            if args.len() == 1 {
                                let arg_ty = self.env.node_types.get(&args[0]).unwrap_or(&Type::Unknown);
                                match arg_ty {
                                    Type::Int => { callee_op = Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_print_int"))); },
                                    Type::Float => { callee_op = Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_print_float"))); },
                                    Type::Bool => { callee_op = Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_print_bool"))); },
                                    Type::String => { callee_op = Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_print_string"))); },
                                    _ => {
                                        let mut base_name_opt = None;
                                        if let Type::Class(name) | Type::Struct(name) | Type::Actor(name) = arg_ty {
                                            base_name_opt = Some(*name);
                                        } else if let Type::GenericInstance { base, .. } = arg_ty {
                                            if let Type::Class(name) | Type::Struct(name) | Type::Actor(name) = &**base {
                                                base_name_opt = Some(*name);
                                            }
                                        }

                                        let mut has_to_string = false;
                                        if let Some(name) = base_name_opt {
                                            has_to_string = if let Some(sig) = self.env.classes.get(&name) {
                                                sig.methods.contains_key(&ustr::Ustr::from("toString"))
                                            } else if let Some(sig) = self.env.structs.get(&name) {
                                                sig.methods.contains_key(&ustr::Ustr::from("toString"))
                                            } else if let Some(sig) = self.env.actors.get(&name) {
                                                sig.methods.contains_key(&ustr::Ustr::from("toString"))
                                            } else {
                                                false
                                            };
                                        }

                                        if has_to_string {
                                            let name = base_name_opt.unwrap();
                                            let method_name = format!("{}_toString", name);
                                            let method_op = Operand::Constant(Constant::Function(ustr::Ustr::from(&method_name)));
                                            
                                            let to_string_temp = self.new_temp(Type::String);
                                            let next_block = self.new_block();
                                            
                                            self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                                                func: method_op,
                                                args: arg_ops.clone(),
                                                destination: Place::new_local(to_string_temp),
                                                target: Some(next_block),
                                                cleanup: None,
                                            });
                                            
                                            self.current_block = next_block;
                                            
                                            arg_ops = vec![Operand::Copy(Place::new_local(to_string_temp))];
                                            callee_op = Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_print_string")));
                                        } else if let Some(name) = base_name_opt {
                                            // Fallback for classes without toString
                                            let fallback_str = format!("<{} instance>", name);
                                            let temp = self.new_temp(Type::String);
                                            self.push_statement(Statement::Assign(
                                                Place::new_local(temp),
                                                Rvalue::Use(Operand::Constant(Constant::String(fallback_str)))
                                            ));
                                            arg_ops = vec![Operand::Copy(Place::new_local(temp))];
                                            callee_op = Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_print_string")));
                                        } else {
                                            // Fallback for GenericParameter / Unknown
                                            callee_op = Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_print_string")));
                                        }
                                    }
                                }
                            }
                        }
                        "malloc" => callee_op = Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_malloc"))),
                        "free" => callee_op = Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_free"))),
                        "ptrStore" => callee_op = Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_ptr_store"))),
                        "ptrStore8" => callee_op = Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_ptr_store8"))),
                        "ptrLoad" => callee_op = Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_ptr_load"))),
                        "ptrLoad8" => callee_op = Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_ptr_load8"))),
                        "time" => callee_op = Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_time"))),
                        "getYear" => callee_op = Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_get_year"))),
                        "hash" => callee_op = Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_hash"))),
                        "retain" => callee_op = Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_retain"))),
                        "release" => callee_op = Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_release"))),
                        "int_to_string" => callee_op = Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_int_to_string"))),
                        "float_to_string" => callee_op = Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_float_to_string"))),
                        "bool_to_string" => callee_op = Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_bool_to_string"))),
                        _ => {}
                    }
                }
                let temp = self.new_temp(Type::Unknown);
                let next_block = self.new_block();
                
                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                    func: callee_op,
                    args: arg_ops,
                    destination: Place::new_local(temp),
                    target: Some(next_block),
                    cleanup: None,
                });
                
                self.current_block = next_block;
                Operand::Copy(Place::new_local(temp))
            }
            Expr::Unwrap(inner) => {
                let inner_op = self.lower_expr(*inner);
                let is_null_temp = self.new_temp(Type::Bool);
                self.push_statement(Statement::Assign(
                    Place::new_local(is_null_temp),
                    Rvalue::BinaryOp(BinaryOp::Eq, inner_op.clone(), Operand::Constant(Constant::Null))
                ));
                
                let panic_block = self.new_block();
                let continue_block = self.new_block();
                
                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::SwitchInt {
                    discr: Operand::Copy(Place::new_local(is_null_temp)),
                    targets: SwitchTargets::new(vec![0], vec![continue_block, panic_block]), // false(0) -> continue, true -> panic
                });
                
                self.body.basic_blocks[panic_block.0].terminator = Some(Terminator::Unreachable);
                self.current_block = continue_block;
                inner_op
            }
            Expr::NullCoalesce { left, right } => {
                let left_op = self.lower_expr(*left);
                let is_null_temp = self.new_temp(Type::Bool);
                self.push_statement(Statement::Assign(
                    Place::new_local(is_null_temp),
                    Rvalue::BinaryOp(BinaryOp::Eq, left_op.clone(), Operand::Constant(Constant::Null))
                ));
                
                let right_block = self.new_block();
                let continue_block = self.new_block();
                
                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::SwitchInt {
                    discr: Operand::Copy(Place::new_local(is_null_temp)),
                    targets: SwitchTargets::new(vec![0], vec![continue_block, right_block]),
                });
                
                let result_temp = self.new_temp(Type::Unknown);
                let merge_block = self.new_block();

                self.current_block = right_block;
                let right_op = self.lower_expr(*right);
                self.push_statement(Statement::Assign(Place::new_local(result_temp), Rvalue::Use(right_op)));
                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Goto { target: merge_block });
                
                self.current_block = continue_block;
                self.push_statement(Statement::Assign(Place::new_local(result_temp), Rvalue::Use(left_op)));
                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Goto { target: merge_block });
                
                self.current_block = merge_block;
                Operand::Copy(Place::new_local(result_temp))
            }
            Expr::Try(inner) => {
                let inner_op = self.lower_expr(*inner);
                let is_err_temp = self.new_temp(Type::Bool);
                let next_block = self.new_block();
                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                    func: Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_is_err"))),
                    args: vec![inner_op.clone()],
                    destination: Place::new_local(is_err_temp),
                    target: Some(next_block),
                    cleanup: None,
                });
                self.current_block = next_block;

                let err_block = self.new_block();
                let continue_block = self.new_block();

                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::SwitchInt {
                    discr: Operand::Copy(Place::new_local(is_err_temp)),
                    targets: SwitchTargets::new(vec![0], vec![continue_block, err_block]),
                });

                self.current_block = err_block;
                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Return);

                self.current_block = continue_block;
                inner_op
            }
            Expr::Await(inner) => {
                let inner_op = self.lower_expr(*inner);
                // Temporarily bypass __pace_promise_await since actor methods in MIR currently return their values directly.
                inner_op
            }
            Expr::Closure { params, return_type: _, body } => {
                let closure_name = ustr::Ustr::from(&format!("{}_closure_{}", self.body.name.as_str(), self.body.basic_blocks.len()));
                let temp = self.new_temp(Type::Unknown);
                // Lower to an aggregate containing the environment (currently empty for simplicity without a full capture pass)
                self.push_statement(Statement::Assign(
                    Place::new_local(temp),
                    Rvalue::Aggregate(AggregateKind::Closure(closure_name), vec![])
                ));
                
                // Build the closure body as a new MirBody
                let mut hir_params = vec![pace_hir::Param {
                    name: ustr::Ustr::from("env"),
                    type_annotation: pace_ast::TypeAnnotation {
                        module_prefix: None,
                        name: ustr::Ustr::from("Any"),
                        args: vec![],
                        is_nullable: false,
                        is_function: false,
                        function_params: None,
                        function_return: None,
                    }
                }];
                hir_params.extend(params.iter().map(|(n, t)| pace_hir::Param {
                    name: *n,
                    type_annotation: t.clone(),
                }));
                
                let mut closure_builder = FuncMirBuilder::new(self.arena, self.env, self.class_layouts, closure_name, &hir_params, false);
                let body_op = closure_builder.lower_expr(*body);
                // Closures return the value of their body
                closure_builder.push_statement(Statement::Assign(Place::new_local(Local(0)), Rvalue::Use(body_op)));
                closure_builder.body.basic_blocks[closure_builder.current_block.0].terminator = Some(Terminator::Return);
                
                self.pending_closures.push(closure_builder.body);
                self.pending_closures.append(&mut closure_builder.pending_closures);
                
                Operand::Copy(Place::new_local(temp))
            }

            Expr::Block(stmts) => {
                let mut last_op = Operand::Constant(Constant::Null);
                for (i, stmt_id) in stmts.iter().enumerate() {
                    if i == stmts.len() - 1 {
                        if let Stmt::Expr(expr_id) = self.arena.get_stmt(*stmt_id) {
                            last_op = self.lower_expr(*expr_id);
                            continue;
                        }
                    }
                    self.lower_stmt(*stmt_id);
                }
                last_op
            }
            _ => {
                Operand::Constant(Constant::Null)
            }
        }
    }

    fn lower_place(&mut self, expr_id: ExprId) -> Option<Place> {
        let expr = self.arena.get_expr(expr_id);
        match expr {
            Expr::Identifier(name) => {
                if let Some(&local) = self.var_map.get(name) {
                    Some(Place::new_local(local))
                } else {
                    None
                }
            }
            Expr::MemberAccess { object, property, computed_class, .. } => {
                let mut is_static_operator = false;
                let mut base_ident = None;
                if let Expr::Identifier(name) = self.arena.get_expr(*object) {
                    base_ident = Some(*name);
                } else if let Expr::GenericInstantiation { callee, .. } = self.arena.get_expr(*object) {
                    if let Expr::Identifier(name) = self.arena.get_expr(*callee) {
                        base_ident = Some(*name);
                    }
                }
                if let Some(name) = base_ident {
                    if self.env.classes.contains_key(&name) || self.env.structs.contains_key(&name) || self.env.enums.contains_key(&name) || self.env.actors.contains_key(&name) {
                        is_static_operator = !self.env.is_local(name);
                    }
                }

                if is_static_operator {
                    let mut class_name = computed_class.unwrap_or(ustr::Ustr::from("Unknown"));
                    if class_name.as_str() == "Unknown" {
                        if let Expr::Identifier(name) = self.arena.get_expr(*object) {
                            class_name = *name;
                        }
                    }
                    return Some(Place::new(crate::PlaceBase::Static(class_name, *property)));
                }
                let obj_op = self.lower_expr(*object);
                match obj_op {
                    Operand::Copy(mut place) | Operand::Move(mut place) => {
                        let mut class_name = computed_class.unwrap_or(ustr::Ustr::from("Unknown"));
                        if class_name.as_str() == "Unknown" {
                            if let Some(obj_ty) = self.env.node_types.get(object) {
                                if let pace_ty::Type::Class(name) | pace_ty::Type::Struct(name) | pace_ty::Type::Actor(name) | pace_ty::Type::Interface(name) = obj_ty {
                                    class_name = *name;
                                }
                            }
                        }
                        
                        let offset = self.class_layouts
                            .get(&class_name)
                            .and_then(|layout| layout.get(property).copied())
                            .unwrap_or(16); // Fallback offset if not found
                        place.projection.push(ProjectionElem::Field(*property, class_name, offset));
                        Some(place)
                    }
                    _ => None
                }
            }
            _ => None,
        }
    }
}
