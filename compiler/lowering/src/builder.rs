use ast::{TypedExpr, TypedExprKind, TypedStmt, TypedStmtKind, TypeExpr, types::Type};
use mir::{BasicBlock, BlockId, ForeignAbiType, ForeignFunction, Function, Inst, Place, Program, RValue, Terminator, Value};

pub struct ProgramBuilder {
    program: Program,
    current_class: Option<String>,
}

fn type_expr_to_abi(type_expr: &TypeExpr) -> ForeignAbiType {
    match type_expr {
        TypeExpr::Named(name) => match name.as_str() {
            "CInt" => ForeignAbiType::I32,
            "CUInt" => ForeignAbiType::I32,
            "CChar" => ForeignAbiType::I8,
            "CSize" => ForeignAbiType::I64,
            "Int" => ForeignAbiType::I64,
            "Float" => ForeignAbiType::F64,
            "Void" => ForeignAbiType::I64, // Not really used for params
            _ => ForeignAbiType::I64,
        },
        TypeExpr::GenericInstance(name, _) if name == "Pointer" => ForeignAbiType::Pointer,
        _ => ForeignAbiType::I64,
    }
}

fn is_ref_type_opt(te: &Option<ast::TypeExpr>) -> bool {
    match te {
        Some(ast::TypeExpr::Named(name)) => {
            !["Int", "Float", "Boolean"].contains(&name.as_str())
        }
        Some(ast::TypeExpr::Optional(inner)) => {
            is_ref_type_opt(&Some((**inner).clone()))
        }
        Some(ast::TypeExpr::Array(_)) => true,
        Some(ast::TypeExpr::GenericInstance(_, _)) => true,
        _ => false
    }
}

fn is_ref_type(te: &ast::TypeExpr) -> bool {
    is_ref_type_opt(&Some(te.clone()))
}

impl ProgramBuilder {
    pub fn new() -> Self {
        Self {
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
                            if is_ref_type(&field.ty) {
                                reference_payloads.insert(idx);
                            }
                        }
                    }
                    variant_defs.push(mir::EnumVariantDef {
                        name: v.name.clone(),
                        reference_payloads,
                    });
                }
                self.program.enums.insert(name.clone(), mir::EnumDef {
                    name: name.clone(),
                    variants: variant_defs,
                });
            }
        }

        let mut enums_map = std::collections::HashMap::new();
        for (name, def) in &self.program.enums {
            let names: Vec<String> = def.variants.iter().map(|v| v.name.clone()).collect();
            enums_map.insert(name.clone(), names);
        }

        let mut main_stmts = Vec::new();
        for stmt in statements {
            if let TypedStmtKind::Class { name, type_params: _, implements: _, methods, fields } = &stmt.kind {
                let mut field_names = Vec::new();
                let mut weak_fields = std::collections::HashSet::new();
                let mut reference_fields = std::collections::HashSet::new();
                
                for field in fields {
                    match &field.kind {
                        TypedStmtKind::Var { name: f_name, is_weak, type_annotation, .. } => {
                            field_names.push(f_name.clone());
                            if *is_weak {
                                weak_fields.insert(f_name.clone());
                            } else if is_ref_type_opt(type_annotation) {
                                reference_fields.insert(f_name.clone());
                            }
                        }
                        TypedStmtKind::Let { name: f_name, type_annotation, .. } => {
                            field_names.push(f_name.clone());
                            if is_ref_type_opt(type_annotation) {
                                reference_fields.insert(f_name.clone());
                            }
                        }
                        _ => {}
                    }
                }
                let class_def = mir::ClassDef {
                    name: name.clone(),
                    fields: field_names,
                    weak_fields,
                    reference_fields,
                };
                self.program.classes.insert(name.clone(), class_def);

                let prev_class = self.current_class.clone();
                self.current_class = Some(name.clone());

                for method in methods {
                    if let TypedStmtKind::Func { name: m_name, params, return_type, body, .. } = &method.kind {
                        let mut param_names = vec!["self".to_string()];
                        let mut ref_params = std::collections::HashSet::new();
                        ref_params.insert("self".to_string());
                        for (p, ty) in params {
                            param_names.push(p.clone());
                            if is_ref_type(ty) {
                                ref_params.insert(p.clone());
                            }
                        }
                        let returns_ref = return_type.as_ref().map_or(false, |ty| is_ref_type(ty));
                        let actual_name = format!("{}::{}", name, m_name);
                        let builder = MirBuilder::new(actual_name.clone(), param_names, ref_params, returns_ref, enums_map.clone());
                        let mir_func = match &body.kind {
                            TypedStmtKind::Block(stmts) => builder.build(stmts),
                            _ => builder.build(std::slice::from_ref(body)),
                        };
                        self.program.functions.insert(actual_name, mir_func);
                    }
                }

                self.current_class = prev_class;
            } else if let TypedStmtKind::Func { name, params, return_type, body, .. } = &stmt.kind {
                let mut param_names = Vec::new();
                let mut ref_params = std::collections::HashSet::new();
                for (p, ty) in params {
                    param_names.push(p.clone());
                    if is_ref_type(ty) {
                        ref_params.insert(p.clone());
                    }
                }
                let returns_ref = return_type.as_ref().map_or(false, |ty| is_ref_type(ty));
                let builder = MirBuilder::new(name.clone(), param_names, ref_params, returns_ref, enums_map.clone());
                let mir_func = match &body.kind {
                    TypedStmtKind::Block(stmts) => builder.build(stmts),
                    _ => builder.build(std::slice::from_ref(body)),
                };
                self.program.functions.insert(name.clone(), mir_func);
            } else if let TypedStmtKind::ForeignFunc { name, params, return_type } = &stmt.kind {
                let mut param_types = Vec::new();
                for (_, ty) in params {
                    param_types.push(type_expr_to_abi(ty));
                }
                let ret_ty = return_type.as_ref().map(type_expr_to_abi);
                self.program.foreign_functions.insert(name.clone(), ForeignFunction {
                    name: name.clone(),
                    symbol: name.clone(),
                    param_types,
                    return_type: ret_ty,
                });
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
            let builder = MirBuilder::new("main".into(), vec![], std::collections::HashSet::new(), false, enums_map.clone());
            let main_func = builder.build(&main_stmts);
            self.program.functions.insert("main".into(), main_func);
        }

        self.program
    }
}

pub struct MirBuilder {
    function: Function,
    current_block: BlockId,
    temp_counter: usize,
    enums_map: std::collections::HashMap<String, Vec<String>>,
}

impl MirBuilder {
    pub fn new(name: String, parameters: Vec<String>, reference_parameters: std::collections::HashSet<String>, returns_reference: bool, enums_map: std::collections::HashMap<String, Vec<String>>) -> Self {
        let mut function = Function::new(name, parameters, reference_parameters, returns_reference);
        let start_block = BlockId(0);
        function.blocks.push(BasicBlock::new(start_block));

        Self {
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

    fn lower_stmt(&mut self, stmt: &TypedStmt) {
        match &stmt.kind {
            TypedStmtKind::Block(stmts) => {
                for s in stmts {
                    self.lower_stmt(s);
                }
            }
            TypedStmtKind::Let { name, initializer, .. } => {
                if let Some(init) = initializer {
                    let val = self.lower_expr(init);
                    self.current().instructions.push(Inst::Assign(Place::Var(name.clone()), RValue::Use(val)));
                } else {
                    self.current().instructions.push(Inst::Assign(Place::Var(name.clone()), RValue::Use(Value::Void)));
                }
            }
            TypedStmtKind::Var { name, initializer, is_weak, .. } => {
                if *is_weak {
                    self.function.weak_vars.insert(name.clone());
                }
                if let Some(init) = initializer {
                    let val = self.lower_expr(init);
                    self.current().instructions.push(Inst::Assign(Place::Var(name.clone()), RValue::Use(val)));
                } else {
                    self.current().instructions.push(Inst::Assign(Place::Var(name.clone()), RValue::Use(Value::Void)));
                }
            }
            TypedStmtKind::Expression(expr) => {
                self.lower_expr(expr);
            }
            TypedStmtKind::If { condition, then_branch, else_branch } => {
                let cond_val = self.lower_expr(condition);
                
                let then_block = self.new_block();
                let merge_block = self.new_block();
                
                let else_block = if else_branch.is_some() {
                    self.new_block()
                } else {
                    merge_block
                };

                self.current().terminator = Some(Terminator::Branch {
                    cond: cond_val,
                    then_block,
                    else_block,
                });

                self.current_block = then_block;
                self.lower_stmt(then_branch);
                if self.current().terminator.is_none() {
                    self.current().terminator = Some(Terminator::Jump(merge_block));
                }

                if let Some(e_branch) = else_branch {
                    self.current_block = else_block;
                    self.lower_stmt(e_branch);
                    if self.current().terminator.is_none() {
                        self.current().terminator = Some(Terminator::Jump(merge_block));
                    }
                }

                self.current_block = merge_block;
            }
            TypedStmtKind::While { condition, body } => {
                let cond_block = self.new_block();
                let body_block = self.new_block();
                let merge_block = self.new_block();

                self.current().terminator = Some(Terminator::Jump(cond_block));
                self.current_block = cond_block;

                let cond_val = self.lower_expr(condition);
                self.current().terminator = Some(Terminator::Branch {
                    cond: cond_val,
                    then_block: body_block,
                    else_block: merge_block,
                });

                self.current_block = body_block;
                self.lower_stmt(body);
                if self.current().terminator.is_none() {
                    self.current().terminator = Some(Terminator::Jump(cond_block));
                }

                self.current_block = merge_block;
            }
            TypedStmtKind::Func { .. } | TypedStmtKind::Class { .. } | TypedStmtKind::Interface { .. } | TypedStmtKind::ForeignFunc { .. } | TypedStmtKind::Enum { .. } => {
                // Nested functions/classes/interfaces are not fully supported in MIR yet,
                // or are handled at the top level.
            }
            TypedStmtKind::For { .. } => {
                // Lowering 'for' loops requires desugaring into an iterator while loop.
                // Skipped for this simplified pass.
            }
            TypedStmtKind::Return { value } => {
                let val = value.as_ref().map(|v| self.lower_expr(v));
                self.current().terminator = Some(Terminator::Return(val));
                self.current_block = self.new_block(); // Any following code goes to a dead block
            }
        }
    }

    fn lower_expr(&mut self, expr: &TypedExpr) -> Value {
        match &expr.kind {
            TypedExprKind::Integer(i) => Value::Int(*i),
            TypedExprKind::Float(f) => Value::Float(*f),
            TypedExprKind::String(s) => Value::String(s.clone()),
            TypedExprKind::Boolean(b) => Value::Boolean(*b),
            TypedExprKind::Null => Value::Null,
            TypedExprKind::Variable(name) => Value::Place(Place::Var(name.clone())),
            TypedExprKind::Array(elements) => {
                let mut vals = Vec::new();
                for el in elements {
                    vals.push(self.lower_expr(el));
                }
                let temp = self.new_temp();
                self.current().instructions.push(Inst::Assign(temp.clone(), RValue::Array(vals, false)));
                Value::Place(temp)
            }
            TypedExprKind::ArrayRepeat { value, count } => {
                let val = self.lower_expr(value);
                let count_val = self.lower_expr(count);
                let temp = self.new_temp();
                self.current().instructions.push(Inst::Assign(temp.clone(), RValue::ArrayRepeat(val, count_val, false)));
                Value::Place(temp)
            }
            TypedExprKind::Match { value, arms } => {
                let match_val = self.lower_expr(value);
                let tag_temp = self.new_temp();
                self.current().instructions.push(Inst::Assign(tag_temp.clone(), RValue::GetVariantTag(match_val.clone())));
                
                let current_block = self.current_block;
                let end_block = self.new_block();
                let result_temp = self.new_temp();
                
                let mut cases = Vec::new();
                let mut default_block = None;
                
                let mut enum_name_opt = None;
                match &value.ty {
                    ast::Type::GenericInstance(name, _) => enum_name_opt = Some(name.clone()),
                    ast::Type::Instance(name) => enum_name_opt = Some(name.clone()),
                    _ => {}
                }
                let resolved_enum_name = enum_name_opt.unwrap_or_else(|| "".to_string());
                
                for arm in arms {
                    let arm_block = self.new_block();
                    
                    if let ast::Pattern::Variant { path, bindings } = &arm.pattern {
                        let enum_name = &resolved_enum_name;
                        let variant_name = path.last().unwrap();
                        let variant_idx = self.enums_map.get(enum_name)
                            .and_then(|variants| variants.iter().position(|v| v == variant_name))
                            .unwrap_or(0); // Fallback if enum not found
                        cases.push((variant_idx, arm_block));
                        
                        self.current_block = arm_block;
                        if let Some(binds) = bindings {
                            for (field_idx, bind) in binds.iter().enumerate() {
                                if bind != "_" {
                                    let field_temp = self.new_temp();
                                    self.current().instructions.push(Inst::Assign(field_temp.clone(), RValue::ExtractPayload(match_val.clone(), variant_idx, field_idx)));
                                    self.current().instructions.push(Inst::Assign(Place::Var(bind.clone()), RValue::Use(Value::Place(field_temp))));
                                }
                            }
                        }
                    } else if let ast::Pattern::Wildcard = &arm.pattern {
                        default_block = Some(arm_block);
                        self.current_block = arm_block;
                    }
                    
                    let arm_val = self.lower_expr(&arm.body);
                    self.current().instructions.push(Inst::Assign(result_temp.clone(), RValue::Use(arm_val)));
                    self.current().terminator = Some(Terminator::Jump(end_block));
                }
                
                let switch_block = &mut self.function.blocks[current_block.0];
                switch_block.terminator = Some(Terminator::Switch {
                    cond: Value::Place(tag_temp),
                    cases,
                    default: default_block,
                });
                
                self.current_block = end_block;
                Value::Place(result_temp)
            }
            TypedExprKind::EnumVariant { enum_name, variant_name } => {
                let variant_idx = self.enums_map.get(enum_name)
                    .and_then(|variants| variants.iter().position(|v| v == variant_name))
                    .unwrap_or(0);
                let temp = self.new_temp();
                self.current().instructions.push(Inst::Assign(temp.clone(), RValue::ConstructVariant(enum_name.clone(), variant_idx, Vec::new())));
                Value::Place(temp)
            }
            TypedExprKind::IndexGet { object, index } => {
                let obj_val = self.lower_expr(object);
                let idx_val = self.lower_expr(index);
                let temp = self.new_temp();
                self.current().instructions.push(Inst::Assign(temp.clone(), RValue::IndexGet(obj_val, idx_val)));
                Value::Place(temp)
            }
            TypedExprKind::IndexSet { object, index, value } => {
                let obj_val = self.lower_expr(object);
                let idx_val = self.lower_expr(index);
                let val_val = self.lower_expr(value);
                self.current().instructions.push(Inst::IndexSet(obj_val, idx_val, val_val.clone()));
                val_val
            }
            TypedExprKind::Grouping(inner) => self.lower_expr(inner),
            TypedExprKind::Get { object, name } => {
                let obj_val = self.lower_expr(object);
                let temp = self.new_temp();
                self.current().instructions.push(Inst::Assign(temp.clone(), RValue::GetProperty(obj_val, name.clone())));
                Value::Place(temp)
            }
            TypedExprKind::ForceUnwrap(inner) => {
                let inner_val = self.lower_expr(inner);
                let temp = self.new_temp();
                self.current().instructions.push(Inst::Assign(temp.clone(), RValue::ForceUnwrap(inner_val)));
                Value::Place(temp)
            }
            TypedExprKind::OptionalGet { object, name } => {
                let obj_val = self.lower_expr(object);
                let temp = self.new_temp();
                
                let then_block = self.new_block();
                let else_block = self.new_block();
                let merge_block = self.new_block();
                
                let is_null_temp = self.new_temp();
                self.current().instructions.push(Inst::Assign(
                    is_null_temp.clone(), 
                    RValue::BinaryOp(ast::BinaryOp::Equal, obj_val.clone(), Value::Null)
                ));
                
                self.current().terminator = Some(Terminator::Branch {
                    cond: Value::Place(is_null_temp),
                    then_block,
                    else_block,
                });
                
                self.current_block = then_block;
                self.current().instructions.push(Inst::Assign(temp.clone(), RValue::Use(Value::Null)));
                self.current().terminator = Some(Terminator::Jump(merge_block));
                
                self.current_block = else_block;
                self.current().instructions.push(Inst::Assign(temp.clone(), RValue::GetProperty(obj_val, name.clone())));
                self.current().terminator = Some(Terminator::Jump(merge_block));
                
                self.current_block = merge_block;
                Value::Place(temp)
            }
            TypedExprKind::Set { object, name, value } => {
                let obj_val = self.lower_expr(object);
                let val_val = self.lower_expr(value);
                self.current().instructions.push(Inst::SetProperty(obj_val, name.clone(), val_val.clone()));
                val_val
            }
            TypedExprKind::Assign { name, value } => {
                let val = self.lower_expr(value);
                self.current().instructions.push(Inst::Assign(Place::Var(name.clone()), RValue::Use(val.clone())));
                val
            }
            TypedExprKind::SelfRef => {
                Value::Place(Place::Var("self".to_string()))
            }
            TypedExprKind::Call { callee, type_args: _, arguments } => {
                let mut arg_values = Vec::new();
                for arg in arguments {
                    arg_values.push(self.lower_expr(arg));
                }

                if let TypedExprKind::Get { object, name } = &callee.kind {
                    let obj_val = self.lower_expr(object);
                    let temp = self.new_temp();
                    
                    if let Type::Instance(class_name) | Type::GenericInstance(class_name, _) = &object.ty {
                        let actual_name = format!("{}::{}", class_name, name);
                        arg_values.insert(0, obj_val);
                        self.current().instructions.push(Inst::Assign(temp.clone(), RValue::Call(actual_name, arg_values)));
                        return Value::Place(temp);
                    }
                    
                    self.current().instructions.push(Inst::Assign(temp.clone(), RValue::MethodCall(obj_val.clone(), name.clone(), arg_values)));
                    return Value::Place(temp);
                }

                if let TypedExprKind::EnumVariant { enum_name, variant_name } = &callee.kind {
                    let temp = self.new_temp();
                    let variant_idx = self.enums_map.get(enum_name)
                        .and_then(|variants| variants.iter().position(|v| v == variant_name))
                        .unwrap_or(0);
                    self.current().instructions.push(Inst::Assign(temp.clone(), RValue::ConstructVariant(enum_name.clone(), variant_idx, arg_values)));
                    return Value::Place(temp);
                }

                if let Type::Class(class_name, _) = &callee.ty {
                    let obj_temp = self.new_temp();
                    self.current().instructions.push(Inst::Assign(obj_temp.clone(), RValue::AllocateObject(class_name.clone())));
                    
                    let actual_name = format!("{}::init", class_name);
                    arg_values.insert(0, Value::Place(obj_temp.clone()));
                    let init_temp = self.new_temp();
                    self.current().instructions.push(Inst::Assign(init_temp, RValue::Call(actual_name, arg_values)));
                    
                    return Value::Place(obj_temp);
                }

                let mut func_name = if let TypedExprKind::Variable(name) = &callee.kind {
                    name.clone()
                } else {
                    panic!("Only direct function calls by name are currently supported.");
                };
                
                if func_name == "print" && arguments.len() == 1 {
                    if let ast::Type::String = arguments[0].ty {
                        func_name = "print_str".to_string();
                    }
                }

                let temp = self.new_temp();
                self.current().instructions.push(Inst::Assign(temp.clone(), RValue::Call(func_name, arg_values)));
                Value::Place(temp)
            }
            TypedExprKind::Unary(op, right) => {
                let right_val = self.lower_expr(right);
                let temp = self.new_temp();
                self.current().instructions.push(Inst::Assign(temp.clone(), RValue::UnaryOp(op.clone(), right_val)));
                Value::Place(temp)
            }
            TypedExprKind::Binary(left, op, right) => {
                let left_ty = left.ty.clone();
                let right_ty = right.ty.clone();
                let left_val = self.lower_expr(left);
                let right_val = self.lower_expr(right);
                let temp = self.new_temp();
                
                if op == &ast::BinaryOp::Add && left_ty == ast::types::Type::String && right_ty == ast::types::Type::String {
                    self.current().instructions.push(Inst::Assign(temp.clone(), RValue::Call("stringConcat".to_string(), vec![left_val, right_val])));
                } else {
                    self.current().instructions.push(Inst::Assign(temp.clone(), RValue::BinaryOp(op.clone(), left_val, right_val)));
                }
                Value::Place(temp)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ast::{BinaryOp, Location, Span};

    fn make_span() -> Span {
        Span::new(0, 0, Location::new(1, 1), Location::new(1, 1))
    }

    #[test]
    fn test_lower_assignment() {
        // let x = 10 + 5;
        let stmt = TypedStmt::new(TypedStmtKind::Let {
            name: "x".into(),
            type_annotation: None,
            initializer: Some(TypedExpr::new(TypedExprKind::Binary(
                Box::new(TypedExpr::new(TypedExprKind::Integer(10), ast::types::Type::Int, make_span())),
                BinaryOp::Add,
                Box::new(TypedExpr::new(TypedExprKind::Integer(5), ast::types::Type::Int, make_span())),
            ), ast::types::Type::Int, make_span())),
        }, make_span());

        let builder = MirBuilder::new("main".into(), vec![], std::collections::HashSet::new(), false, std::collections::HashMap::new());
        let fun = builder.build(&[stmt]);

        assert_eq!(fun.blocks.len(), 1);
        let block = &fun.blocks[0];
        
        assert_eq!(block.instructions.len(), 2);
        
        match &block.instructions[0] {
            Inst::Assign(Place::Temp(0), RValue::BinaryOp(BinaryOp::Add, Value::Int(10), Value::Int(5))) => {}
            _ => panic!("Expected binary op assignment"),
        }

        match &block.instructions[1] {
            Inst::Assign(Place::Var(name), RValue::Use(Value::Place(Place::Temp(0)))) => {
                assert_eq!(name, "x");
            }
            _ => panic!("Expected variable assignment"),
        }
    }

    #[test]
    fn test_lower_if_statement() {
        // if true { let x = 1; }
        let stmt = TypedStmt::new(TypedStmtKind::If {
            condition: TypedExpr::new(TypedExprKind::Boolean(true), ast::types::Type::Boolean, make_span()),
            then_branch: Box::new(TypedStmt::new(TypedStmtKind::Block(vec![
                TypedStmt::new(TypedStmtKind::Let {
                    name: "x".into(),
                    type_annotation: None,
                    initializer: Some(TypedExpr::new(TypedExprKind::Integer(1), ast::types::Type::Int, make_span())),
                }, make_span())
            ]), make_span())),
            else_branch: None,
        }, make_span());

        let builder = MirBuilder::new("main".into(), vec![], std::collections::HashSet::new(), false, std::collections::HashMap::new());
        let fun = builder.build(&[stmt]);

        // Start block, Then Block, Merge Block
        assert_eq!(fun.blocks.len(), 3);
        
        // Start block should branch
        match &fun.blocks[0].terminator {
            Some(Terminator::Branch { cond: Value::Boolean(true), then_block, else_block }) => {
                assert_eq!(then_block.0, 1);
                assert_eq!(else_block.0, 2);
            }
            _ => panic!("Expected branch terminator"),
        }

        // Then block should jump to merge block
        match &fun.blocks[1].terminator {
            Some(Terminator::Jump(BlockId(2))) => {}
            _ => panic!("Expected jump to merge block"),
        }
    }
}
