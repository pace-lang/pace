use crate::context::CodegenContext;
use cranelift::prelude::*;
use cranelift_module::{Linkage, Module};
use pace_mir::{Constant, MirBody, MirProgram, Operand, Rvalue, Statement, Terminator};

pub fn compile_mir_program<M: Module>(
    context: &mut CodegenContext<M>,
    builder_context: &mut cranelift::prelude::FunctionBuilderContext,
    ctx: &mut cranelift::codegen::Context,
    program: &MirProgram,
) -> Result<(), crate::layouts::CodegenError> {
    
    // Pass 1: Declare all functions
    for (name, body) in &program.functions {
        if context.funcs.contains_key(name) {
            continue;
        }
        let mut sig = context.module.make_signature();
        sig.call_conv = if body.is_extern {
            cranelift::prelude::isa::CallConv::SystemV
        } else {
            cranelift::prelude::isa::CallConv::Fast
        };
        
        // Return type (always I64 for now)
        sig.returns.push(AbiParam::new(types::I64));
        
        // Arguments
        for _ in 0..body.arg_count {
            sig.params.push(AbiParam::new(types::I64));
        }

        let linkage = if body.is_extern { 
            Linkage::Import 
        } else if name.as_str() == "main" {
            Linkage::Local
        } else { 
            Linkage::Local 
        };
        let id = context
            .module
            .declare_function(name.as_str(), linkage, &sig)
            .map_err(|e| crate::layouts::CodegenError {
                message: e.to_string(),
            })?;
        
        context.funcs.insert(*name, id);
    }

    // Pass 2: Define all functions
    for (name, body) in &program.functions {
        if body.is_extern {
            continue;
        }
        let id = *context.funcs.get(name).unwrap();
        compile_mir_function(context, builder_context, ctx, body, id)?;
    }

    Ok(())
}

fn compile_mir_function<M: Module>(
    context: &mut CodegenContext<M>,
    builder_context: &mut cranelift::prelude::FunctionBuilderContext,
    ctx: &mut cranelift::codegen::Context,
    body: &MirBody,
    func_id: cranelift_module::FuncId,
) -> Result<(), crate::layouts::CodegenError> {
    ctx.clear();
    
    ctx.func.signature.returns.push(AbiParam::new(types::I64));
    for _ in 0..body.arg_count {
        ctx.func.signature.params.push(AbiParam::new(types::I64));
    }

    let mut builder = FunctionBuilder::new(&mut ctx.func, builder_context);
    
    // Create Cranelift blocks for all MIR BasicBlocks
    let mut blocks = Vec::with_capacity(body.basic_blocks.len());
    for _ in 0..body.basic_blocks.len() {
        blocks.push(builder.create_block());
    }
    
    // Create Cranelift Variables for all Locals
    let mut locals = Vec::with_capacity(body.local_decls.len());
    for _ in 0..body.local_decls.len() {
        let var = builder.declare_var(types::I64);
        locals.push(var);
    }

    builder.append_block_params_for_function_params(blocks[0]);
    builder.switch_to_block(blocks[0]);
    
    // Initialize return value (Local 0) with 0 by default, for void functions
    let default_ret = builder.ins().iconst(types::I64, 0);
    builder.def_var(locals[0], default_ret);
    
    // Map arguments to local variables
    for i in 0..body.arg_count {
        let param_val = builder.block_params(blocks[0])[i];
        builder.def_var(locals[i + 1], param_val); // Arg 0 is Local 1, etc.
    }

    // Translate blocks
    for (bb_idx, bb_data) in body.basic_blocks.iter().enumerate() {
        let current_block = blocks[bb_idx];
        
        if builder.current_block() != Some(current_block) {
            builder.switch_to_block(current_block);
        }

        // Statements
        for stmt in &bb_data.statements {
            match stmt {
                Statement::Assign(place, rvalue) => {
                    let val = translate_rvalue(&mut builder, context, rvalue, &locals)?;
                    store_to_place(&mut builder, context, place, &locals, val)?;
                }
                _ => {}
            }
        }

        // Terminator
        if let Some(terminator) = &bb_data.terminator {
            match terminator {
                Terminator::Goto { target } => {
                    builder.ins().jump(blocks[target.index()], &[]);
                }
                Terminator::Return => {
                    // Return value is always in Local 0
                    let ret_val = builder.use_var(locals[0]);
                    builder.ins().return_(&[ret_val]);
                }
                Terminator::SwitchInt { discr, targets } => {
                    let discr_val = translate_operand(&mut builder, context, discr, &locals)?;
                    
                    // A simple switch logic (if false, go to false target, else true target)
                    // Currently SwitchTargets only has 2 targets (false, true) in our MIR Builder
                    // We'll just branch if zero (false)
                    let false_block = blocks[targets.targets[0].index()];
                    let true_block = blocks[targets.targets[1].index()];
                    
                    builder.ins().brif(discr_val, true_block, &[], false_block, &[]);
                }
                Terminator::Call { func, args, destination, target, .. } => {
                    if let Operand::Constant(Constant::Function(func_name)) = func {
                        let callee_id = if let Some(callee_id) = context.funcs.get(func_name) {
                            *callee_id
                        } else {
                            // Declare missing extern function on the fly
                            let mut sig = context.module.make_signature();
                            sig.call_conv = cranelift::prelude::isa::CallConv::SystemV;
                            for _ in 0..args.len() {
                                sig.params.push(cranelift::prelude::AbiParam::new(cranelift::prelude::types::I64));
                            }
                            sig.returns.push(cranelift::prelude::AbiParam::new(cranelift::prelude::types::I64));
                            let id = context.module.declare_function(func_name.as_str(), cranelift_module::Linkage::Import, &sig).unwrap();
                            context.funcs.insert(*func_name, id);
                            id
                        };
                        
                        let callee_ref = context.module.declare_func_in_func(callee_id, builder.func);
                        let mut arg_vals = Vec::new();
                        for arg in args {
                            arg_vals.push(translate_operand(&mut builder, context, arg, &locals)?);
                        }
                        
                        let inst = builder.ins().call(callee_ref, &arg_vals);
                        let results = builder.inst_results(inst);
                        let result_val = if results.is_empty() {
                            builder.ins().iconst(cranelift::prelude::types::I64, 0)
                        } else {
                            results[0]
                        };
                        builder.def_var(locals[destination.local.index()], result_val);
                        
                        if let Some(next_block) = target {
                            builder.ins().jump(blocks[next_block.index()], &[]);
                        } else {
                            builder.ins().trap(cranelift::prelude::TrapCode::user(1).unwrap());
                        }
                    } else {
                        // Indirect call (e.g., Closures or function pointers)
                        // A closure evaluates to a pointer (env_ptr), the first 8 bytes of which is the function pointer.
                        let env_ptr = translate_operand(&mut builder, context, func, &locals)?;
                        let ptr_ty = context.module.target_config().pointer_type();
                        
                        let func_ptr = builder.ins().load(
                            ptr_ty,
                            cranelift::prelude::MemFlagsData::new(),
                            env_ptr,
                            0,
                        );

                        let mut sig = context.module.make_signature();
                        sig.call_conv = cranelift::prelude::isa::CallConv::Fast;
                        
                        // Indirect calls pass env_ptr as the first argument
                        sig.params.push(cranelift::prelude::AbiParam::new(ptr_ty));
                        for _ in args {
                            sig.params.push(cranelift::prelude::AbiParam::new(cranelift::prelude::types::I64));
                        }
                        sig.returns.push(cranelift::prelude::AbiParam::new(cranelift::prelude::types::I64));
                        
                        let sig_ref = builder.import_signature(sig);
                        
                        let mut arg_vals = vec![env_ptr];
                        for arg in args {
                            arg_vals.push(translate_operand(&mut builder, context, arg, &locals)?);
                        }
                        
                        let inst = builder.ins().call_indirect(sig_ref, func_ptr, &arg_vals);
                        
                        let results = builder.inst_results(inst);
                        let result_val = if results.is_empty() {
                            builder.ins().iconst(cranelift::prelude::types::I64, 0)
                        } else {
                            results[0]
                        };
                        builder.def_var(locals[destination.local.index()], result_val);
                        
                        if let Some(next_block) = target {
                            builder.ins().jump(blocks[next_block.index()], &[]);
                        } else {
                            builder.ins().trap(cranelift::prelude::TrapCode::user(1).unwrap());
                        }
                    }
                }
                Terminator::Unreachable => {
                    builder.ins().trap(cranelift::prelude::TrapCode::user(1).unwrap());
                }
            }
        } else {
            builder.ins().trap(cranelift::prelude::TrapCode::user(1).unwrap());
        }
    }

    builder.seal_all_blocks();
    builder.finalize(context.module.target_config());

    if let Err(e) = cranelift::codegen::verify_function(&ctx.func, &cranelift::codegen::settings::Flags::new(cranelift::codegen::settings::builder())) {
        panic!("Verifier error in {}: {:#?}", body.name, e);
    }

    // Compute CFG before defining the function, as Cranelift 0.135.0 has a bug where
    // Context::compile might use an invalid CFG for verification if it's not pre-computed.
    ctx.compute_cfg();
    ctx.compute_domtree();

    if let Err(e) = context.module.define_function(func_id, ctx) {
        panic!("define_function error in {}: {:#?}\nIR:\n{}", body.name, e, ctx.func.display());
    }
    
    Ok(())
}

fn translate_rvalue<M: Module>(
    builder: &mut FunctionBuilder,
    context: &mut CodegenContext<M>,
    rvalue: &Rvalue,
    locals: &[Variable],
) -> Result<Value, crate::layouts::CodegenError> {
    match rvalue {
        Rvalue::Use(operand) => translate_operand(builder, context, operand, locals),
        Rvalue::BinaryOp(op, left, right) => {
            let lhs = translate_operand(builder, context, left, locals)?;
            let rhs = translate_operand(builder, context, right, locals)?;
            
            use pace_ast::BinaryOp;
            let val = match op {
                BinaryOp::Add => builder.ins().iadd(lhs, rhs),
                BinaryOp::Sub => builder.ins().isub(lhs, rhs),
                BinaryOp::Mul => builder.ins().imul(lhs, rhs),
                BinaryOp::Div => builder.ins().sdiv(lhs, rhs),
                BinaryOp::Mod => builder.ins().srem(lhs, rhs),
                BinaryOp::Eq => {
                    let b = builder.ins().icmp(cranelift::codegen::ir::condcodes::IntCC::Equal, lhs, rhs);
                    builder.ins().uextend(types::I64, b)
                }
                BinaryOp::NotEq => {
                    let b = builder.ins().icmp(cranelift::codegen::ir::condcodes::IntCC::NotEqual, lhs, rhs);
                    builder.ins().uextend(types::I64, b)
                }
                BinaryOp::Less => {
                    let b = builder.ins().icmp(cranelift::codegen::ir::condcodes::IntCC::SignedLessThan, lhs, rhs);
                    builder.ins().uextend(types::I64, b)
                }
                BinaryOp::Greater => {
                    let b = builder.ins().icmp(cranelift::codegen::ir::condcodes::IntCC::SignedGreaterThan, lhs, rhs);
                    builder.ins().uextend(types::I64, b)
                }
                BinaryOp::LessEq => {
                    let b = builder.ins().icmp(cranelift::codegen::ir::condcodes::IntCC::SignedLessThanOrEqual, lhs, rhs);
                    builder.ins().uextend(types::I64, b)
                }
                BinaryOp::GreaterEq => {
                    let b = builder.ins().icmp(cranelift::codegen::ir::condcodes::IntCC::SignedGreaterThanOrEqual, lhs, rhs);
                    builder.ins().uextend(types::I64, b)
                }
                _ => builder.ins().iconst(types::I64, 0), // Fallback
            };
            Ok(val)
        }
        Rvalue::Aggregate(pace_mir::AggregateKind::Closure(closure_name), _) => {
            let size_val = builder.ins().iconst(types::I64, 16);
            let malloc_id = *context.funcs.get(&ustr::Ustr::from("__pace_malloc")).unwrap();
            let local_malloc = context.module.declare_func_in_func(malloc_id, builder.func);
            let call = builder.ins().call(local_malloc, &[size_val]);
            let env_ptr = builder.inst_results(call)[0];
            
            // Try to resolve the closure function or import it
            let func_id = if let Some(id) = context.funcs.get(closure_name) {
                *id
            } else {
                let mut sig = context.module.make_signature();
                sig.call_conv = cranelift::prelude::isa::CallConv::Fast;
                // Signature is unknown at this point, but we know it's Fast
                context.module.declare_function(closure_name.as_str(), cranelift_module::Linkage::Export, &sig).unwrap_or_else(|_| {
                    panic!("Failed to declare closure function {}", closure_name);
                })
            };
            
            let func_ref = context.module.declare_func_in_func(func_id, builder.func);
            let func_ptr = builder.ins().func_addr(context.module.target_config().pointer_type(), func_ref);
            
            builder.ins().store(cranelift::prelude::MemFlagsData::new(), func_ptr, env_ptr, 0);
            
            Ok(env_ptr)
        }
        Rvalue::Aggregate(pace_mir::AggregateKind::Class(class_name, class_size), operands) => {
            let size_val = builder.ins().iconst(types::I64, *class_size as i64);

            let malloc_id = *context.funcs.get(&ustr::Ustr::from("__pace_malloc")).unwrap();
            let local_malloc = context.module.declare_func_in_func(malloc_id, builder.func);
            let call = builder.ins().call(local_malloc, &[size_val]);
            let obj_ptr = builder.inst_results(call)[0];

            let one = builder.ins().iconst(types::I64, 1);
            builder.ins().store(cranelift::prelude::MemFlagsData::new(), one, obj_ptr, 0);

            // Null VTable pointer since MIR dynamic dispatch is not implemented
            let null_vtable = builder.ins().iconst(types::I64, 0);
            builder.ins().store(cranelift::prelude::MemFlagsData::new(), null_vtable, obj_ptr, 8);

            let mut offset = 16;
            for op in operands {
                let val = translate_operand(builder, context, op, locals)?;
                builder.ins().store(cranelift::prelude::MemFlagsData::new(), val, obj_ptr, offset as i32);
                offset += 8;
            }

            Ok(obj_ptr)
        }
        _ => Ok(builder.ins().iconst(types::I64, 0)),
    }
}

fn translate_operand<M: Module>(
    builder: &mut FunctionBuilder,
    context: &mut CodegenContext<M>,
    operand: &Operand,
    locals: &[Variable],
) -> Result<Value, crate::layouts::CodegenError> {
    match operand {
        Operand::Copy(place) | Operand::Move(place) => {
            translate_place(builder, context, place, locals)
        }
        Operand::Constant(c) => match c {
            Constant::Int(i) => Ok(builder.ins().iconst(types::I64, *i)),
            Constant::Bool(b) => Ok(builder.ins().iconst(types::I64, if *b { 1 } else { 0 })),
            Constant::String(s) => {
                let s_ustr = ustr::Ustr::from(s);
                let string_name = if let Some(name) = context.string_cache.get(&s_ustr) {
                    name.clone()
                } else {
                    let id = context.string_id;
                    context.string_id += 1;
                    let name = format!("__string_{}", id);
                    context.string_cache.insert(s_ustr.clone(), name.clone());

                    let mut data_ctx = cranelift_module::DataDescription::new();
                    let mut bytes = s.as_bytes().to_vec();
                    bytes.push(0); // Null terminator for CStr
                    data_ctx.define(bytes.into_boxed_slice());

                    let data_id = context
                        .module
                        .declare_data(&name, cranelift_module::Linkage::Local, true, false)
                        .unwrap();

                    context.module.define_data(data_id, &data_ctx).unwrap();
                    name
                };

                let data_id = context
                    .module
                    .declare_data(&string_name, cranelift_module::Linkage::Local, true, false)
                    .unwrap();

                let ptr_ty = context.module.target_config().pointer_type();
                let local_data = context
                    .module
                    .declare_data_in_func(data_id, builder.func);
                Ok(builder.ins().symbol_value(ptr_ty, local_data))
            }
            _ => Ok(builder.ins().iconst(types::I64, 0)),
        },
    }
}

fn translate_place<M: Module>(
    builder: &mut FunctionBuilder,
    context: &mut CodegenContext<M>,
    place: &pace_mir::Place,
    locals: &[Variable],
) -> Result<Value, crate::layouts::CodegenError> {
    let mut val = builder.use_var(locals[place.local.index()]);
    for proj in &place.projection {
        match proj {
            pace_mir::ProjectionElem::Field(_prop, _class_name, offset) => {
                val = builder.ins().load(types::I64, cranelift::prelude::MemFlagsData::new(), val, *offset as i32);
            }
            _ => {}
        }
    }
    Ok(val)
}

fn store_to_place<M: Module>(
    builder: &mut FunctionBuilder,
    context: &mut CodegenContext<M>,
    place: &pace_mir::Place,
    locals: &[Variable],
    value: Value,
) -> Result<(), crate::layouts::CodegenError> {
    if place.projection.is_empty() {
        builder.def_var(locals[place.local.index()], value);
    } else {
        let mut ptr = builder.use_var(locals[place.local.index()]);
        for (i, proj) in place.projection.iter().enumerate() {
            if i == place.projection.len() - 1 {
                if let pace_mir::ProjectionElem::Field(_prop, _class_name, offset) = proj {
                    builder.ins().store(cranelift::prelude::MemFlagsData::new(), value, ptr, *offset as i32);
                }
            } else {
                if let pace_mir::ProjectionElem::Field(_prop, _class_name, offset) = proj {
                    ptr = builder.ins().load(types::I64, cranelift::prelude::MemFlagsData::new(), ptr, *offset as i32);
                }
            }
        }
    }
    Ok(())
}
