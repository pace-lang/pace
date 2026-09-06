use crate::context::CodegenContext;
use cranelift::prelude::*;
use cranelift_module::{Linkage, Module};
use pace_mir::{Constant, MirBody, MirProgram, Operand, Rvalue, Statement, Terminator};

pub fn compile_mir_program<M: Module>(
    context: &mut CodegenContext<M>,
    builder_context: &mut cranelift::prelude::FunctionBuilderContext,
    ctx: &mut cranelift::codegen::Context,
    program: &MirProgram,
) -> Result<(), crate::CodegenError> {
    
    // Pass 1: Declare all functions
    for (name, body) in &program.functions {
        if context.funcs.contains_key(name) {
            continue;
        }
        let mut sig = context.module.make_signature();
        sig.call_conv = if body.is_extern || name.as_str() == "main" {
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
            Linkage::Export
        } else { 
            Linkage::Local 
        };
        let id = context
            .module
            .declare_function(name.as_str(), linkage, &sig)
            .map_err(|e| crate::CodegenError {
                message: e.to_string(),
            })?;
        context.funcs.insert(*name, id);
    }
    
    // Pass 1.5: Define VTables
    let mut trap_sig = context.module.make_signature();
    trap_sig.call_conv = cranelift::prelude::isa::CallConv::Fast;
    let trap_id = match context.funcs.get(&ustr::Ustr::from("__pace_trap")) {
        Some(id) => *id,
        None => {
            let id = context.module.declare_function("__pace_trap", Linkage::Import, &trap_sig).unwrap();
            context.funcs.insert(ustr::Ustr::from("__pace_trap"), id);
            id
        }
    };

    for (class_name, vtable) in &program.vtables {
        let vtable_name = format!("{}_vtable", class_name.as_str());
        let data_id = context
            .module
            .declare_data(&vtable_name, Linkage::Export, true, false)
            .map_err(|e| crate::CodegenError { message: e.to_string() })?;
        context.vtables.insert(*class_name, data_id);

        let mut data_ctx = cranelift_module::DataDescription::new();
        // Allocate space for the vtable (8 bytes per entry)
        data_ctx.define(Box::from(vec![0; vtable.len() * 8]));

        for (index, method_opt) in vtable.iter().enumerate() {
            let func_id = if let Some(method_name) = method_opt {
                *context.funcs.get(method_name).unwrap_or(&trap_id)
            } else {
                trap_id
            };
            
            let func_ref = context.module.declare_func_in_data(func_id, &mut data_ctx);
            data_ctx.write_function_addr((index * 8) as u32, func_ref);
        }

        context
            .module
            .define_data(data_id, &data_ctx)
            .map_err(|e| crate::CodegenError { message: e.to_string() })?;
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
) -> Result<(), crate::CodegenError> {
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
    for decl in &body.local_decls {
        let cl_type = match decl.ty {
            pace_ty::Type::Float => types::F64,
            _ => types::I64,
        };
        let var = builder.declare_var(cl_type);
        locals.push(var);
    }

    builder.append_block_params_for_function_params(blocks[0]);
    builder.switch_to_block(blocks[0]);
    
    // Initialize return value (Local 0) with 0 by default, for void functions
    let _ret_type = match body.local_decls[0].ty {
        pace_ty::Type::Float => types::F64,
        _ => types::I64,
    };
    let default_ret = match body.local_decls[0].ty {
        pace_ty::Type::Float => builder.ins().f64const(0.0),
        _ => builder.ins().iconst(types::I64, 0),
    };
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
                        
                        if func_name.as_str() == "__pace_hash" && arg_vals.len() == 1 {
                            let mut x = arg_vals[0];
                            let thirty = builder.ins().iconst(cranelift::prelude::types::I64, 30);
                            let x_shifted = builder.ins().ushr(x, thirty);
                            x = builder.ins().bxor(x, x_shifted);
                            let m1 = builder.ins().iconst(cranelift::prelude::types::I64, 0xbf58476d1ce4e5b9u64 as i64);
                            x = builder.ins().imul(x, m1);
                            let twenty_seven = builder.ins().iconst(cranelift::prelude::types::I64, 27);
                            let x_shifted2 = builder.ins().ushr(x, twenty_seven);
                            x = builder.ins().bxor(x, x_shifted2);
                            let m2 = builder.ins().iconst(cranelift::prelude::types::I64, 0x94d049bb133111ebu64 as i64);
                            x = builder.ins().imul(x, m2);
                            let thirty_one = builder.ins().iconst(cranelift::prelude::types::I64, 31);
                            let x_shifted3 = builder.ins().ushr(x, thirty_one);
                            let result_val = builder.ins().bxor(x, x_shifted3);
                            
                            store_to_place(&mut builder, context, destination, &locals, result_val)?;
                            
                            if let Some(next_block) = target {
                                builder.ins().jump(blocks[next_block.index()], &[]);
                            } else {
                                builder.ins().trap(cranelift::prelude::TrapCode::user(1).unwrap());
                            }
                        } else {
                            let inst = builder.ins().call(callee_ref, &arg_vals);
                            let results = builder.inst_results(inst);
                            let result_val = if results.is_empty() {
                                builder.ins().iconst(cranelift::prelude::types::I64, 0)
                            } else {
                                results[0]
                            };
                            store_to_place(&mut builder, context, destination, &locals, result_val)?;
                            
                            if let Some(next_block) = target {
                                builder.ins().jump(blocks[next_block.index()], &[]);
                            } else {
                                builder.ins().trap(cranelift::prelude::TrapCode::user(1).unwrap());
                            }
                        }
                    } else {
                        // Indirect call (e.g., Closures or function pointers)
                        // A closure evaluates to a pointer (env_ptr). 
                        // Offset 0 (8 bytes) is the RC, Offset 8 (8 bytes) is the function pointer.
                        let env_ptr = translate_operand(&mut builder, context, func, &locals)?;
                        let ptr_ty = context.module.target_config().pointer_type();
                        
                        let func_ptr = builder.ins().load(
                            ptr_ty,
                            cranelift::prelude::MemFlagsData::new(),
                            env_ptr,
                            8, // Load function pointer from offset 8
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
                        store_to_place(&mut builder, context, destination, &locals, result_val)?;
                        
                        if let Some(next_block) = target {
                            builder.ins().jump(blocks[next_block.index()], &[]);
                        } else {
                            builder.ins().trap(cranelift::prelude::TrapCode::user(1).unwrap());
                        }
                    }
                }
                Terminator::InterfaceCall { obj, method_index, args, destination, target, cleanup: _ } => {
                    let obj_ptr = translate_operand(&mut builder, context, obj, &locals)?;
                    let ptr_ty = context.module.target_config().pointer_type();
                    
                    // Load vtable pointer from offset 8
                    let vtable_ptr = builder.ins().load(
                        ptr_ty,
                        cranelift::prelude::MemFlagsData::new(),
                        obj_ptr,
                        8,
                    );
                    
                    // Load function pointer from vtable_ptr + method_index * 8
                    let func_ptr = builder.ins().load(
                        ptr_ty,
                        cranelift::prelude::MemFlagsData::new(),
                        vtable_ptr,
                        (*method_index * 8) as i32,
                    );
                    
                    let mut sig = context.module.make_signature();
                    sig.call_conv = cranelift::prelude::isa::CallConv::Fast;
                    sig.returns.push(cranelift::prelude::AbiParam::new(cranelift::prelude::types::I64));
                    for _ in 0..args.len() {
                        sig.params.push(cranelift::prelude::AbiParam::new(cranelift::prelude::types::I64));
                    }
                    let sig_ref = builder.import_signature(sig);
                    
                    let mut arg_vals = Vec::new();
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
                    store_to_place(&mut builder, context, destination, &locals, result_val)?;
                    
                    if let Some(next_block) = target {
                        builder.ins().jump(blocks[next_block.index()], &[]);
                    } else {
                        builder.ins().trap(cranelift::prelude::TrapCode::user(1).unwrap());
                    }
                }
                Terminator::Unreachable => {
                    builder.ins().trap(cranelift::prelude::TrapCode::user(0).unwrap());
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
) -> Result<Value, crate::CodegenError> {
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

        Rvalue::Aggregate(pace_mir::AggregateKind::StackClass(class_name, class_size), operands) => {
            let slot = builder.create_sized_stack_slot(cranelift::prelude::StackSlotData::new(
                cranelift::prelude::StackSlotKind::ExplicitSlot,
                *class_size as u32,
                3, // 8-byte alignment
            ));
            let obj_ptr = builder.ins().stack_addr(context.module.target_config().pointer_type(), slot, 0);
            
            let immortal = builder.ins().iconst(types::I64, 4611686018427387904);
            builder.ins().store(cranelift::prelude::MemFlagsData::new(), immortal, obj_ptr, 0);
            
            // Store VTable pointer for stack classes too
            let vtable_val = if let Some(&data_id) = context.vtables.get(class_name) {
                let local_data_id = context.module.declare_data_in_func(data_id, builder.func);
                builder.ins().symbol_value(context.module.target_config().pointer_type(), local_data_id)
            } else {
                builder.ins().iconst(types::I64, 0)
            };
            builder.ins().store(cranelift::prelude::MemFlagsData::new(), vtable_val, obj_ptr, 8);
            
            for (i, op) in operands.iter().enumerate() {
                let val = translate_operand(builder, context, op, &locals)?;
                let offset = (16 + i * 8) as i32;
                builder.ins().store(cranelift::prelude::MemFlagsData::new(), val, obj_ptr, offset);
            }
            Ok(obj_ptr)
        }
        Rvalue::Aggregate(pace_mir::AggregateKind::Class(class_name, class_size), operands) => {
            let size_val = builder.ins().iconst(types::I64, *class_size as i64);

            let malloc_id = *context.funcs.get(&ustr::Ustr::from("__pace_malloc")).unwrap();
            let local_malloc = context.module.declare_func_in_func(malloc_id, builder.func);
            let call = builder.ins().call(local_malloc, &[size_val]);
            let obj_ptr = builder.inst_results(call)[0];

            let one = builder.ins().iconst(types::I64, 1);
            builder.ins().store(cranelift::prelude::MemFlagsData::new(), one, obj_ptr, 0);

            // Store VTable pointer
            let vtable_val = if let Some(&data_id) = context.vtables.get(class_name) {
                let local_data_id = context.module.declare_data_in_func(data_id, builder.func);
                builder.ins().symbol_value(context.module.target_config().pointer_type(), local_data_id)
            } else {
                builder.ins().iconst(types::I64, 0)
            };
            builder.ins().store(cranelift::prelude::MemFlagsData::new(), vtable_val, obj_ptr, 8);

            let mut offset = 16;
            for op in operands {
                let val = translate_operand(builder, context, op, locals)?;
                builder.ins().store(cranelift::prelude::MemFlagsData::new(), val, obj_ptr, offset as i32);
                offset += 8;
            }

            Ok(obj_ptr)
        }
        Rvalue::Aggregate(pace_mir::AggregateKind::Closure(closure_name), operands) => {
            // Allocate 16 bytes for the environment struct (8 bytes RC, 8 bytes Function Pointer, plus whatever env variables)
            let env_size = 16 + (operands.len() * 8); // Assuming all captured env variables are 8 bytes (pointers/i64)
            let size_val = builder.ins().iconst(types::I64, env_size as i64);

            let malloc_id = *context.funcs.get(&ustr::Ustr::from("__pace_malloc")).unwrap();
            let local_malloc = context.module.declare_func_in_func(malloc_id, builder.func);
            let call = builder.ins().call(local_malloc, &[size_val]);
            let env_ptr = builder.inst_results(call)[0];

            // Offset 0: ARC Reference Count = 1
            let one = builder.ins().iconst(types::I64, 1);
            builder.ins().store(cranelift::prelude::MemFlagsData::new(), one, env_ptr, 0);

            // Offset 8: Function Pointer
            let func_id = if let Some(id) = context.funcs.get(closure_name) {
                *id
            } else {
                let mut sig = context.module.make_signature();
                sig.call_conv = cranelift::prelude::isa::CallConv::Fast;
                let id = context.module.declare_function(closure_name.as_str(), cranelift_module::Linkage::Export, &sig).unwrap();
                context.funcs.insert(*closure_name, id);
                id
            };
            let local_func = context.module.declare_func_in_func(func_id, builder.func);
            let ptr_ty = context.module.target_config().pointer_type();
            let func_addr = builder.ins().func_addr(ptr_ty, local_func);
            builder.ins().store(cranelift::prelude::MemFlagsData::new(), func_addr, env_ptr, 8);

            let mut offset = 16;
            for op in operands {
                let val = translate_operand(builder, context, op, locals)?;
                builder.ins().store(cranelift::prelude::MemFlagsData::new(), val, env_ptr, offset as i32);
                offset += 8;
            }

            Ok(env_ptr)
        }
        _ => Ok(builder.ins().iconst(types::I64, 0)),
    }
}

fn translate_operand<M: Module>(
    builder: &mut FunctionBuilder,
    context: &mut CodegenContext<M>,
    operand: &Operand,
    locals: &[Variable],
) -> Result<Value, crate::CodegenError> {
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
            Constant::Float(f) => Ok(builder.ins().f64const(*f)),
            _ => Ok(builder.ins().iconst(types::I64, 0)),
        },
    }
}

fn translate_place<M: cranelift_module::Module>(
    builder: &mut FunctionBuilder,
    context: &mut CodegenContext<M>,
    place: &pace_mir::Place,
    locals: &[Variable],
) -> Result<Value, crate::CodegenError> {
    let mut val = match &place.base {
        pace_mir::PlaceBase::Local(local) => builder.use_var(locals[local.index()]),
        pace_mir::PlaceBase::Static(class_name, field) => {
            let name_str = format!("__pace_static_{}_{}", class_name.as_str(), field.as_str());
            let name = ustr::Ustr::from(&name_str);
            
            let data_id = if let Some(&id) = context.global_vars.get(&name) {
                id
            } else {
                let id = context
                    .module
                    .declare_data(name.as_str(), cranelift_module::Linkage::Export, true, false)
                    .unwrap();
                
                let mut data_ctx = cranelift_module::DataDescription::new();
                data_ctx.define_zeroinit(8);
                context.module.define_data(id, &data_ctx).unwrap();
                
                context.global_vars.insert(name, id);
                id
            };
            
            let local_data = context.module.declare_data_in_func(data_id, builder.func);
            let ptr_ty = context.module.target_config().pointer_type();
            let ptr = builder.ins().symbol_value(ptr_ty, local_data);
            builder.ins().load(cranelift::prelude::types::I64, cranelift::prelude::MemFlagsData::new(), ptr, 0)
        }
    };
    
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

fn store_to_place<M: cranelift_module::Module>(
    builder: &mut FunctionBuilder,
    context: &mut CodegenContext<M>,
    place: &pace_mir::Place,
    locals: &[Variable],
    value: Value,
) -> Result<(), crate::CodegenError> {
    if place.projection.is_empty() {
        match &place.base {
            pace_mir::PlaceBase::Local(local) => {
                builder.def_var(locals[local.index()], value);
            }
            pace_mir::PlaceBase::Static(class_name, field) => {
                let name_str = format!("__pace_static_{}_{}", class_name.as_str(), field.as_str());
                let name = ustr::Ustr::from(&name_str);
                
                let data_id = if let Some(&id) = context.global_vars.get(&name) {
                    id
                } else {
                    let id = context
                        .module
                        .declare_data(name.as_str(), cranelift_module::Linkage::Export, true, false)
                        .unwrap();
                    
                    let mut data_ctx = cranelift_module::DataDescription::new();
                    data_ctx.define_zeroinit(8);
                    context.module.define_data(id, &data_ctx).unwrap();
                    
                    context.global_vars.insert(name, id);
                    id
                };
                
                let local_data = context.module.declare_data_in_func(data_id, builder.func);
                let ptr_ty = context.module.target_config().pointer_type();
                let ptr = builder.ins().symbol_value(ptr_ty, local_data);
                builder.ins().store(cranelift::prelude::MemFlagsData::new(), value, ptr, 0);
            }
        }
    } else {
        let mut ptr = match &place.base {
            pace_mir::PlaceBase::Local(local) => builder.use_var(locals[local.index()]),
            pace_mir::PlaceBase::Static(class_name, field) => {
                let name_str = format!("__pace_static_{}_{}", class_name.as_str(), field.as_str());
                let name = ustr::Ustr::from(&name_str);
                
                let data_id = if let Some(&id) = context.global_vars.get(&name) {
                    id
                } else {
                    let id = context
                        .module
                        .declare_data(name.as_str(), cranelift_module::Linkage::Export, true, false)
                        .unwrap();
                    
                    let mut data_ctx = cranelift_module::DataDescription::new();
                    data_ctx.define_zeroinit(8);
                    context.module.define_data(id, &data_ctx).unwrap();
                    
                    context.global_vars.insert(name, id);
                    id
                };
                
                let local_data = context.module.declare_data_in_func(data_id, builder.func);
                let ptr_ty = context.module.target_config().pointer_type();
                let global_ptr = builder.ins().symbol_value(ptr_ty, local_data);
                builder.ins().load(cranelift::prelude::types::I64, cranelift::prelude::MemFlagsData::new(), global_ptr, 0)
            }
        };
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
