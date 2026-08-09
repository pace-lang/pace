use ast::{Expr, ExprKind, Stmt, StmtKind};
use mir::{BasicBlock, BlockId, Function, Inst, Place, RValue, Terminator, Value};

pub struct MirBuilder {
    function: Function,
    current_block: BlockId,
    temp_counter: usize,
}

impl MirBuilder {
    pub fn new(name: String) -> Self {
        let mut function = Function::new(name);
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
        self.function
    }

    fn lower_stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Block(stmts) => {
                for s in stmts {
                    self.lower_stmt(s);
                }
            }
            StmtKind::Let { name, initializer } | StmtKind::Var { name, initializer } => {
                let val = self.lower_expr(initializer);
                self.current().instructions.push(Inst::Assign(Place::Var(name.clone()), RValue::Use(val)));
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
            StmtKind::Func { .. } => {
                // A nested function definition would typically spawn a new MIR function context.
                // Skipped for this simplified basic pass.
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
            initializer: Expr::new(ExprKind::Binary(
                Box::new(Expr::new(ExprKind::Integer(10), make_span())),
                BinaryOp::Add,
                Box::new(Expr::new(ExprKind::Integer(5), make_span())),
            ), make_span()),
        }, make_span());

        let builder = MirBuilder::new("main".into());
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
                    initializer: Expr::new(ExprKind::Integer(1), make_span()),
                }, make_span())
            ]), make_span())),
            else_branch: None,
        }, make_span());

        let builder = MirBuilder::new("main".into());
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
