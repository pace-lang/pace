use ast::{Expr, ExprKind, Stmt, StmtKind};
use mir::{BasicBlock, BlockId, Function, Inst, Place, RValue, Terminator, Value, Program};

pub struct ProgramBuilder {
    program: Program,
    current_class: Option<String>,
}

impl ProgramBuilder {
    pub fn new() -> Self {
        Self {
            program: Program::new(),
            current_class: None,
        }
    }

    pub fn build(mut self, statements: &[Stmt]) -> Program {
        let mut main_stmts = Vec::new();
        for stmt in statements {
            if let StmtKind::Class { name, methods, fields } = &stmt.kind {
                let mut field_names = Vec::new();
                for field in fields {
                    if let StmtKind::Var { name: f_name, .. } | StmtKind::Let { name: f_name, .. } = &field.kind {
                        field_names.push(f_name.clone());
                    }
                }
                let class_def = mir::ClassDef {
                    name: name.clone(),
                    fields: field_names,
                };
                self.program.classes.insert(name.clone(), class_def);

                let prev_class = self.current_class.clone();
                self.current_class = Some(name.clone());

                for method in methods {
                    if let StmtKind::Func { name: m_name, params, body, .. } = &method.kind {
                        let mut param_names = vec!["self".to_string()];
                        for (p, _) in params {
                            param_names.push(p.clone());
                        }
                        let actual_name = format!("{}::{}", name, m_name);
                        let builder = MirBuilder::new(actual_name.clone(), param_names);
                        let mir_func = match &body.kind {
                            StmtKind::Block(stmts) => builder.build(stmts),
                            _ => builder.build(std::slice::from_ref(body)),
                        };
                        self.program.functions.insert(actual_name, mir_func);
                    }
                }

                self.current_class = prev_class;
            } else if let StmtKind::Func { name, params, body, .. } = &stmt.kind {
                let mut param_names = Vec::new();
                for (p, _) in params {
                    param_names.push(p.clone());
                }
                let builder = MirBuilder::new(name.clone(), param_names);
                let mir_func = match &body.kind {
                    StmtKind::Block(stmts) => builder.build(stmts),
                    _ => builder.build(std::slice::from_ref(body)),
                };
                self.program.functions.insert(name.clone(), mir_func);
            } else {
                main_stmts.push(stmt.clone());
            }
        }
        
        let builder = MirBuilder::new("main".into(), vec![]);
        let main_func = builder.build(&main_stmts);
        self.program.functions.insert("main".into(), main_func);

        self.program
    }
}

pub struct MirBuilder {
    function: Function,
    current_block: BlockId,
    temp_counter: usize,
}

impl MirBuilder {
    pub fn new(name: String, parameters: Vec<String>) -> Self {
        let mut function = Function::new(name, parameters);
        let start_block = BlockId(0);
        function.blocks.push(BasicBlock::new(start_block));

        Self {
            function,
            current_block: start_block,
            temp_counter: 0,
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

    pub fn build(mut self, statements: &[Stmt]) -> Function {
        for stmt in statements {
            self.lower_stmt(stmt);
        }
        if self.current().terminator.is_none() {
            self.current().terminator = Some(Terminator::Return(None));
        }
        self.function
    }

    fn lower_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Block(stmts) => {
                for s in stmts {
                    self.lower_stmt(s);
                }
            }
            StmtKind::Let { name, initializer, .. } | StmtKind::Var { name, initializer, .. } => {
                if let Some(init) = initializer {
                    let val = self.lower_expr(init);
                    self.current().instructions.push(Inst::Assign(Place::Var(name.clone()), RValue::Use(val)));
                } else {
                    self.current().instructions.push(Inst::Assign(Place::Var(name.clone()), RValue::Use(Value::Void)));
                }
            }
            StmtKind::Expression(expr) => {
                self.lower_expr(expr);
            }
            StmtKind::If { condition, then_branch, else_branch } => {
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
            StmtKind::While { condition, body } => {
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
            StmtKind::Func { .. } | StmtKind::Class { .. } => {
                // Nested functions or classes are not handled in this basic pass
            }
            StmtKind::For { .. } => {
                // Lowering 'for' loops requires desugaring into an iterator while loop.
                // Skipped for this simplified pass.
            }
            StmtKind::Return { value } => {
                let val = value.as_ref().map(|v| self.lower_expr(v));
                self.current().terminator = Some(Terminator::Return(val));
                self.current_block = self.new_block(); // Any following code goes to a dead block
            }
        }
    }

    fn lower_expr(&mut self, expr: &Expr) -> Value {
        match &expr.kind {
            ExprKind::Integer(i) => Value::Int(*i),
            ExprKind::Float(f) => Value::Float(*f),
            ExprKind::String(s) => Value::String(s.clone()),
            ExprKind::Boolean(b) => Value::Boolean(*b),
            ExprKind::Variable(name) => Value::Place(Place::Var(name.clone())),
            ExprKind::Grouping(inner) => self.lower_expr(inner),
            ExprKind::Get { object, name } => {
                let obj_val = self.lower_expr(object);
                let temp = self.new_temp();
                self.current().instructions.push(Inst::Assign(temp.clone(), RValue::GetProperty(obj_val, name.clone())));
                Value::Place(temp)
            }
            ExprKind::Set { object, name, value } => {
                let obj_val = self.lower_expr(object);
                let val_val = self.lower_expr(value);
                self.current().instructions.push(Inst::SetProperty(obj_val, name.clone(), val_val.clone()));
                val_val
            }
            ExprKind::Assign { name, value } => {
                let val = self.lower_expr(value);
                self.current().instructions.push(Inst::Assign(Place::Var(name.clone()), RValue::Use(val.clone())));
                val
            }
            ExprKind::SelfRef => {
                Value::Place(Place::Var("self".to_string()))
            }
            ExprKind::Call { callee, arguments } => {
                let mut arg_values = Vec::new();
                for arg in arguments {
                    arg_values.push(self.lower_expr(arg));
                }

                if let ExprKind::Get { object, name } = &callee.kind {
                    let obj_val = self.lower_expr(object);
                    let temp = self.new_temp();
                    self.current().instructions.push(Inst::Assign(temp.clone(), RValue::MethodCall(obj_val, name.clone(), arg_values)));
                    return Value::Place(temp);
                }

                let func_name = if let ExprKind::Variable(name) = &callee.kind {
                    name.clone()
                } else {
                    panic!("Only direct function calls by name are currently supported.");
                };

                let temp = self.new_temp();
                self.current().instructions.push(Inst::Assign(temp.clone(), RValue::Call(func_name, arg_values)));
                Value::Place(temp)
            }
            ExprKind::Unary(op, right) => {
                let right_val = self.lower_expr(right);
                let temp = self.new_temp();
                self.current().instructions.push(Inst::Assign(temp.clone(), RValue::UnaryOp(op.clone(), right_val)));
                Value::Place(temp)
            }
            ExprKind::Binary(left, op, right) => {
                let left_val = self.lower_expr(left);
                let right_val = self.lower_expr(right);
                let temp = self.new_temp();
                self.current().instructions.push(Inst::Assign(temp.clone(), RValue::BinaryOp(op.clone(), left_val, right_val)));
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
        let stmt = Stmt::new(StmtKind::Let {
            name: "x".into(),
            type_annotation: None,
            initializer: Some(Expr::new(ExprKind::Binary(
                Box::new(Expr::new(ExprKind::Integer(10), make_span())),
                BinaryOp::Add,
                Box::new(Expr::new(ExprKind::Integer(5), make_span())),
            ), make_span())),
        }, make_span());

        let builder = MirBuilder::new("main".into(), vec![]);
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
        let stmt = Stmt::new(StmtKind::If {
            condition: Expr::new(ExprKind::Boolean(true), make_span()),
            then_branch: Box::new(Stmt::new(StmtKind::Block(vec![
                Stmt::new(StmtKind::Let {
                    name: "x".into(),
                    type_annotation: None,
                    initializer: Some(Expr::new(ExprKind::Integer(1), make_span())),
                }, make_span())
            ]), make_span())),
            else_branch: None,
        }, make_span());

        let builder = MirBuilder::new("main".into(), vec![]);
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
