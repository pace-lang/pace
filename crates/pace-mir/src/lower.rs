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
}

pub struct MirBuilder<'a> {
    arena: &'a HirArena,
}

impl<'a> MirBuilder<'a> {
    pub fn new(arena: &'a HirArena, _env: &'a Environment) -> Self {
        Self { arena }
    }

    pub fn build(self, stmts: &[StmtId]) -> MirProgram {
        let mut program = MirProgram { functions: HashMap::new() };
        
        for &stmt_id in stmts {
            let stmt = self.arena.get_stmt(stmt_id);
            if let Stmt::Module { body, .. } = stmt {
                for &item_id in body {
                    let item = self.arena.get_stmt(item_id);
                    if let Stmt::FuncDecl { name, body: func_body, params, .. } = item {
                        let func_builder = FuncMirBuilder::new(self.arena, *name, params.len());
                        let mir_body = func_builder.build(func_body);
                        program.functions.insert(*name, mir_body);
                    } else if let Stmt::ClassDecl { name: class_name, methods, .. } = item {
                        for &method_id in methods {
                            if let Stmt::FuncDecl { name, body: func_body, params, .. } = self.arena.get_stmt(method_id) {
                                let func_builder = FuncMirBuilder::new(self.arena, *name, params.len());
                                let mir_body = func_builder.build(func_body);
                                let mangled_name = ustr::Ustr::from(&format!("{}_{}", class_name, name));
                                program.functions.insert(mangled_name, mir_body);
                            }
                        }
                    }
                }
            } else if let Stmt::FuncDecl { name, body: func_body, params, .. } = stmt {
                let func_builder = FuncMirBuilder::new(self.arena, *name, params.len());
                let mir_body = func_builder.build(func_body);
                program.functions.insert(*name, mir_body);
            }
        }
        
        program
    }
}

struct FuncMirBuilder<'a> {
    arena: &'a HirArena,
    body: MirBody,
    current_block: BasicBlock,
    var_map: HashMap<Ustr, Local>,
}

impl<'a> FuncMirBuilder<'a> {
    pub fn new(arena: &'a HirArena, name: Ustr, arg_count: usize) -> Self {
        let mut body = MirBody::new(name, arg_count);
        // Block 0 is the entry block
        body.basic_blocks.push(BasicBlockData::new());
        // Local 0 is the return pointer
        body.local_decls.push(LocalDecl {
            ty: Type::Unknown, // Will be updated later
            mutability: Mutability::Mut,
            kind: LocalKind::ReturnPointer,
            source_info: pace_span::Span::default(),
        });
        
        Self {
            arena,
            body,
            current_block: BasicBlock(0),
            var_map: HashMap::new(),
        }
    }

    pub fn build(mut self, stmts: &[StmtId]) -> MirBody {
        for &stmt_id in stmts {
            self.lower_stmt(stmt_id);
        }
        
        // If the last block doesn't have a terminator, add a Return terminator
        if self.body.basic_blocks[self.current_block.0].terminator.is_none() {
            self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Return);
        }
        
        self.body
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
                let local = self.new_local(Type::Unknown, Mutability::Mut, LocalKind::User(*name), span);
                self.var_map.insert(*name, local);
                
                if let Some(init_id) = initializer {
                    let operand = self.lower_expr(*init_id);
                    self.push_statement(Statement::Assign(Place::new(local), Rvalue::Use(operand)));
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
                    let ret_place = Place::new(Local(0));
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
            Expr::Identifier(name) => {
                if let Some(&local) = self.var_map.get(name) {
                    Operand::Copy(Place::new(local))
                } else {
                    Operand::Constant(Constant::Function(*name))
                }
            }
            Expr::Binary { left, op, right } => {
                let left_op = self.lower_expr(*left);
                let right_op = self.lower_expr(*right);
                
                let temp = self.new_temp(Type::Unknown);
                self.push_statement(Statement::Assign(
                    Place::new(temp),
                    Rvalue::BinaryOp(op.clone(), left_op, right_op)
                ));
                Operand::Copy(Place::new(temp))
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

                // Check if it's a Class initialization
                if let Expr::Identifier(name) = self.arena.get_expr(*callee) {
                    // We check if it's a known class from the environment
                    // Since environment isn't fully structured for MIR to check classes directly easily,
                    // we'll rely on the codegen to do it, OR we emit an Aggregate here.
                    // For now, let's treat capitalized identifiers as Classes
                    if name.chars().next().unwrap().is_uppercase() {
                        let temp = self.new_temp(Type::Unknown);
                        self.push_statement(Statement::Assign(
                            Place::new(temp),
                            Rvalue::Aggregate(AggregateKind::Class(*name), arg_ops)
                        ));
                        return Operand::Copy(Place::new(temp));
                    }
                }
                
                let callee_op = self.lower_expr(*callee);
                
                let temp = self.new_temp(Type::Unknown);
                let next_block = self.new_block();
                
                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                    func: callee_op,
                    args: arg_ops,
                    destination: Place::new(temp),
                    target: Some(next_block),
                    cleanup: None,
                });
                
                self.current_block = next_block;
                Operand::Copy(Place::new(temp))
            }
            Expr::Unwrap(inner) => {
                let inner_op = self.lower_expr(*inner);
                let is_null_temp = self.new_temp(Type::Bool);
                self.push_statement(Statement::Assign(
                    Place::new(is_null_temp),
                    Rvalue::BinaryOp(BinaryOp::Eq, inner_op.clone(), Operand::Constant(Constant::Null))
                ));
                
                let panic_block = self.new_block();
                let continue_block = self.new_block();
                
                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::SwitchInt {
                    discr: Operand::Copy(Place::new(is_null_temp)),
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
                    Place::new(is_null_temp),
                    Rvalue::BinaryOp(BinaryOp::Eq, left_op.clone(), Operand::Constant(Constant::Null))
                ));
                
                let right_block = self.new_block();
                let continue_block = self.new_block();
                
                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::SwitchInt {
                    discr: Operand::Copy(Place::new(is_null_temp)),
                    targets: SwitchTargets::new(vec![0], vec![continue_block, right_block]),
                });
                
                let result_temp = self.new_temp(Type::Unknown);
                let merge_block = self.new_block();

                self.current_block = right_block;
                let right_op = self.lower_expr(*right);
                self.push_statement(Statement::Assign(Place::new(result_temp), Rvalue::Use(right_op)));
                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Goto { target: merge_block });
                
                self.current_block = continue_block;
                self.push_statement(Statement::Assign(Place::new(result_temp), Rvalue::Use(left_op)));
                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Goto { target: merge_block });
                
                self.current_block = merge_block;
                Operand::Copy(Place::new(result_temp))
            }
            Expr::Try(inner) => {
                let inner_op = self.lower_expr(*inner);
                let is_err_temp = self.new_temp(Type::Bool);
                let next_block = self.new_block();
                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                    func: Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_is_err"))),
                    args: vec![inner_op.clone()],
                    destination: Place::new(is_err_temp),
                    target: Some(next_block),
                    cleanup: None,
                });
                self.current_block = next_block;

                let err_block = self.new_block();
                let continue_block = self.new_block();

                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::SwitchInt {
                    discr: Operand::Copy(Place::new(is_err_temp)),
                    targets: SwitchTargets::new(vec![0], vec![continue_block, err_block]),
                });

                self.current_block = err_block;
                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Return);

                self.current_block = continue_block;
                inner_op
            }
            Expr::Await(inner) => {
                let inner_op = self.lower_expr(*inner);
                let temp = self.new_temp(Type::Unknown);
                let next_block = self.new_block();
                self.body.basic_blocks[self.current_block.0].terminator = Some(Terminator::Call {
                    func: Operand::Constant(Constant::Function(ustr::Ustr::from("__pace_promise_await"))),
                    args: vec![inner_op],
                    destination: Place::new(temp),
                    target: Some(next_block),
                    cleanup: None,
                });
                self.current_block = next_block;
                Operand::Copy(Place::new(temp))
            }
            Expr::Closure { params: _, return_type: _, body: _ } => {
                let closure_name = ustr::Ustr::from(&format!("closure_{}", self.body.basic_blocks.len()));
                let temp = self.new_temp(Type::Unknown);
                // Lower to an aggregate containing the environment (currently empty for simplicity without a full capture pass)
                self.push_statement(Statement::Assign(
                    Place::new(temp),
                    Rvalue::Aggregate(AggregateKind::Closure(closure_name), vec![])
                ));
                Operand::Copy(Place::new(temp))
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
                    Some(Place::new(local))
                } else {
                    None
                }
            }
            Expr::MemberAccess { object, property, computed_class, .. } => {
                let obj_op = self.lower_expr(*object);
                match obj_op {
                    Operand::Copy(mut place) | Operand::Move(mut place) => {
                        let class_name = computed_class.unwrap_or(ustr::Ustr::from("Unknown"));
                        place.projection.push(ProjectionElem::Field(*property, class_name));
                        Some(place)
                    }
                    _ => None
                }
            }
            _ => None,
        }
    }
}
