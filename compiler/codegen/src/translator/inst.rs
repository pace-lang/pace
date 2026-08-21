use super::Translator;
use cranelift_codegen::ir::{self, InstBuilder, types};
use mir::{Inst, RValue, Value};
use ast::{BinaryOp, UnaryOp};

use cranelift_module::Module;

impl<'a, 'b> Translator<'a, 'b> {

    pub(super) fn translate_inst(&mut self, inst: &Inst) -> Result<(), String> {
        match inst {
            Inst::Assign(place, rvalue) => {
                let cl_val = match rvalue {
                    RValue::Use(val) => self.translate_value(val)?,
                    RValue::BinaryOp(op, left, right) => {
                        let cl_left = self.translate_value(left)?;
                        let cl_right = self.translate_value(right)?;

                        match op {
                            BinaryOp::Add => self.builder.ins().iadd(cl_left, cl_right),
                            BinaryOp::Subtract => self.builder.ins().isub(cl_left, cl_right),
                            BinaryOp::Multiply => self.builder.ins().imul(cl_left, cl_right),
                            BinaryOp::Divide => self.builder.ins().sdiv(cl_left, cl_right),
                            BinaryOp::Modulo => self.builder.ins().srem(cl_left, cl_right),
                            BinaryOp::Equal => {
                                let c = self.builder.ins().icmp(
                                    ir::condcodes::IntCC::Equal,
                                    cl_left,
                                    cl_right,
                                );
                                self.builder.ins().uextend(types::I64, c)
                            }
                            BinaryOp::NotEqual => {
                                let c = self.builder.ins().icmp(
                                    ir::condcodes::IntCC::NotEqual,
                                    cl_left,
                                    cl_right,
                                );
                                self.builder.ins().uextend(types::I64, c)
                            }
                            BinaryOp::Less => {
                                let c = self.builder.ins().icmp(
                                    ir::condcodes::IntCC::SignedLessThan,
                                    cl_left,
                                    cl_right,
                                );
                                self.builder.ins().uextend(types::I64, c)
                            }
                            BinaryOp::LessEqual => {
                                let c = self.builder.ins().icmp(
                                    ir::condcodes::IntCC::SignedLessThanOrEqual,
                                    cl_left,
                                    cl_right,
                                );
                                self.builder.ins().uextend(types::I64, c)
                            }
                            BinaryOp::Greater => {
                                let c = self.builder.ins().icmp(
                                    ir::condcodes::IntCC::SignedGreaterThan,
                                    cl_left,
                                    cl_right,
                                );
                                self.builder.ins().uextend(types::I64, c)
                            }
                            BinaryOp::GreaterEqual => {
                                let c = self.builder.ins().icmp(
                                    ir::condcodes::IntCC::SignedGreaterThanOrEqual,
                                    cl_left,
                                    cl_right,
                                );
                                self.builder.ins().uextend(types::I64, c)
                            }
                        }
                    }
                    RValue::UnaryOp(op, right) => {
                        if let Value::Float(f) = right {
                            if *op == UnaryOp::Negate {
                                let f_val = self.builder.ins().f64const(-*f);
                                self.builder.ins().bitcast(
                                    types::I64,
                                    cranelift_codegen::ir::MemFlagsData::new(),
                                    f_val,
                                )
                            } else {
                                let cl_right = self.translate_value(right)?;
                                match op {
                                    UnaryOp::Negate => self.builder.ins().ineg(cl_right),
                                }
                            }
                        } else {
                            let cl_right = self.translate_value(right)?;
                            match op {
                                UnaryOp::Negate => self.builder.ins().ineg(cl_right),
                            }
                        }
                    }
                    RValue::Call(func_name, args) => {
                        let target_func_name = func_name.as_str();

                        if target_func_name == "paceNullPointer" {
                            self.builder.ins().iconst(types::I64, 0)
                        } else if target_func_name == "ptrReadByte" {
                            let ptr_val = self.translate_value(&args[0])?;
                            let index_val = self.translate_value(&args[1])?;
                            let addr = self.builder.ins().iadd(ptr_val, index_val);
                            let val = self.builder.ins().load(types::I8, cranelift_codegen::ir::MemFlagsData::new(), addr, 0);
                            self.builder.ins().uextend(types::I64, val)
                        } else if target_func_name == "ptrWriteByte" {
                            let ptr_val = self.translate_value(&args[0])?;
                            let index_val = self.translate_value(&args[1])?;
                            let val = self.translate_value(&args[2])?;
                            let val_i8 = self.builder.ins().ireduce(types::I8, val);
                            let addr = self.builder.ins().iadd(ptr_val, index_val);
                            self.builder.ins().store(cranelift_codegen::ir::MemFlagsData::new(), val_i8, addr, 0);
                            self.builder.ins().iconst(types::I64, 0)
                        } else if target_func_name == "bitwiseAnd" {
                            let a = self.translate_value(&args[0])?;
                            let b = self.translate_value(&args[1])?;
                            self.builder.ins().band(a, b)
                        } else if target_func_name == "bitwiseOr" {
                            let a = self.translate_value(&args[0])?;
                            let b = self.translate_value(&args[1])?;
                            self.builder.ins().bor(a, b)
                        } else if target_func_name == "bitwiseXor" {
                            let a = self.translate_value(&args[0])?;
                            let b = self.translate_value(&args[1])?;
                            self.builder.ins().bxor(a, b)
                        } else if target_func_name == "bitwiseNot" {
                            let a = self.translate_value(&args[0])?;
                            self.builder.ins().bnot(a)
                        } else if target_func_name == "bitwiseShl" {
                            let a = self.translate_value(&args[0])?;
                            let b = self.translate_value(&args[1])?;
                            self.builder.ins().ishl(a, b)
                        } else if target_func_name == "bitwiseShr" {
                            let a = self.translate_value(&args[0])?;
                            let b = self.translate_value(&args[1])?;
                            self.builder.ins().sshr(a, b)
                        } else if target_func_name.starts_with("ptrRead_") {
                            let type_name = &target_func_name[8..];
                            let ptr_val = self.translate_value(&args[0])?;
                            let index_val = self.translate_value(&args[1])?;
                            
                            let mut is_struct = false;
                            let mut size_bytes = 8;
                            
                            if let Some(class_def) = self.program.classes.get(type_name) {
                                if class_def.is_struct {
                                    is_struct = true;
                                    size_bytes = class_def.fields.len() as u32 * 8;
                                }
                            }
                            
                            let byte_offset = self.builder.ins().imul_imm_s(index_val, size_bytes as i64);
                            let addr = self.builder.ins().iadd(ptr_val, byte_offset);
                            
                            if is_struct {
                                let ss = self.builder.create_sized_stack_slot(ir::StackSlotData::new(
                                    ir::StackSlotKind::ExplicitSlot,
                                    size_bytes,
                                    4,
                                ));
                                let dest_addr = self.builder.ins().stack_addr(types::I64, ss, 0);
                                let size_val = self.builder.ins().iconst(types::I64, size_bytes as i64);
                                self.builder.call_memcpy(self.module.target_config(), dest_addr, addr, size_val);
                                dest_addr
                            } else {
                                self.builder.ins().load(types::I64, cranelift_codegen::ir::MemFlagsData::new(), addr, 0)
                            }
                        } else if target_func_name.starts_with("ptrWrite_") {
                            let type_name = &target_func_name[9..];
                            let ptr_val = self.translate_value(&args[0])?;
                            let index_val = self.translate_value(&args[1])?;
                            let val = self.translate_value(&args[2])?;
                            
                            let mut is_struct = false;
                            let mut size_bytes = 8;
                            
                            if let Some(class_def) = self.program.classes.get(type_name) {
                                if class_def.is_struct {
                                    is_struct = true;
                                    size_bytes = class_def.fields.len() as u32 * 8;
                                }
                            }
                            
                            let byte_offset = self.builder.ins().imul_imm_s(index_val, size_bytes as i64);
                            let addr = self.builder.ins().iadd(ptr_val, byte_offset);
                            
                            if is_struct {
                                let size_val = self.builder.ins().iconst(types::I64, size_bytes as i64);
                                self.builder.call_memcpy(self.module.target_config(), addr, val, size_val);
                            } else {
                                self.builder.ins().store(cranelift_codegen::ir::MemFlagsData::new(), val, addr, 0);
                            }
                            self.builder.ins().iconst(types::I64, 0)
                        } else if target_func_name.starts_with("paceRetainRef_") {
                            let type_name = &target_func_name[14..];
                            let val = self.translate_value(&args[0])?;
                            
                            let mut is_ref = false;
                            if let Some(class_def) = self.program.classes.get(type_name) {
                                if !class_def.is_struct {
                                    is_ref = true;
                                }
                            } else if self.program.enums.contains_key(type_name) {
                                is_ref = true;
                            } else if type_name == "String" || type_name.starts_with("Array_") {
                                is_ref = true;
                            }
                            
                            if is_ref {
                                let retain_func = self.func_ids.get("pace_retain").unwrap();
                                let local_retain = self.module.declare_func_in_func(*retain_func, self.builder.func);
                                self.builder.ins().call(local_retain, &[val]);
                            }
                            self.builder.ins().iconst(types::I64, 0)
                        } else if target_func_name.starts_with("paceReleaseRef_") {
                            let type_name = &target_func_name[15..];
                            let val = self.translate_value(&args[0])?;
                            
                            let mut is_ref = false;
                            if let Some(class_def) = self.program.classes.get(type_name) {
                                if !class_def.is_struct {
                                    is_ref = true;
                                }
                            } else if self.program.enums.contains_key(type_name) {
                                is_ref = true;
                            } else if type_name == "String" || type_name.starts_with("Array_") {
                                is_ref = true;
                            }
                            
                            if is_ref {
                                let release_func = self.func_ids.get("pace_release").unwrap();
                                let local_release = self.module.declare_func_in_func(*release_func, self.builder.func);
                                self.builder.ins().call(local_release, &[val]);
                            }
                            self.builder.ins().iconst(types::I64, 0)
                        } else if target_func_name.starts_with("sizeof_") {
                            let type_name = &target_func_name[7..];
                            let mut size_bytes = 8;
                            if let Some(class_def) = self.program.classes.get(type_name) {
                                if class_def.is_struct {
                                    size_bytes = class_def.fields.len() as u32 * 8;
                                }
                            }
                            self.builder.ins().iconst(types::I64, size_bytes as i64)
                        } else if target_func_name == "hash_Int" || target_func_name == "hash_Boolean" {
                            // Simple identity hash for primitives
                            self.translate_value(&args[0])?
                        } else if target_func_name == "equals_Int" || target_func_name == "equals_Boolean" {
                            // Native equality for primitives
                            let left = self.translate_value(&args[0])?;
                            let right = self.translate_value(&args[1])?;
                            let c = self.builder.ins().icmp(ir::condcodes::IntCC::Equal, left, right);
                            self.builder.ins().uextend(types::I64, c)
                        } else {
                            let func_id = self
                                .func_ids
                                .get(target_func_name)
                                .ok_or_else(|| format!("Function {} not found", target_func_name))?;
                            let local_callee = self
                                .module
                                .declare_func_in_func(*func_id, self.builder.func);

                        let mut arg_vals = Vec::new();

                        if let Some(foreign_func) = self.program.foreign_functions.get(func_name) {
                            for (i, arg) in args.iter().enumerate() {
                                let mut val = self.translate_value(arg)?;
                                if let Some(abi_ty) = foreign_func.param_types.get(i) {
                                    match abi_ty {
                                        mir::ForeignAbiType::I8 => {
                                            val = self.builder.ins().ireduce(types::I8, val)
                                        }
                                        mir::ForeignAbiType::I16 => {
                                            val = self.builder.ins().ireduce(types::I16, val)
                                        }
                                        mir::ForeignAbiType::I32 => {
                                            val = self.builder.ins().ireduce(types::I32, val)
                                        }
                                        mir::ForeignAbiType::F64 => {
                                            val = self.builder.ins().bitcast(
                                                types::F64,
                                                cranelift_codegen::ir::MemFlagsData::new(),
                                                val,
                                            );
                                        }
                                        _ => {}
                                    }
                                }
                                arg_vals.push(val);
                            }
                        } else {
                            for arg in args {
                                arg_vals.push(self.translate_value(arg)?);
                            }
                        }

                        let call_inst = self.builder.ins().call(local_callee, &arg_vals);
                        let results = self.builder.inst_results(call_inst);
                        let mut result_val = if results.is_empty() {
                            self.builder.ins().iconst(types::I64, 0)
                        } else {
                            results[0]
                        };

                        if let Some(foreign_func) = self.program.foreign_functions.get(func_name)
                            && let Some(abi_ty) = &foreign_func.return_type
                        {
                            match abi_ty {
                                mir::ForeignAbiType::I8
                                | mir::ForeignAbiType::I16
                                | mir::ForeignAbiType::I32 => {
                                    result_val = self.builder.ins().sextend(types::I64, result_val);
                                }
                                mir::ForeignAbiType::F32 => {
                                    let bitcast = self.builder.ins().bitcast(
                                        types::I32,
                                        cranelift_codegen::ir::MemFlagsData::new(),
                                        result_val,
                                    );
                                    result_val = self.builder.ins().uextend(types::I64, bitcast);
                                }
                                mir::ForeignAbiType::F64 => {
                                    result_val = self.builder.ins().bitcast(
                                        types::I64,
                                        cranelift_codegen::ir::MemFlagsData::new(),
                                        result_val,
                                    );
                                }
                                _ => {}
                            }
                        }

                        if target_func_name == "Option_Point_unwrap" {
                            let debug_func = self.func_ids.get("debug_ptr").unwrap();
                            let local_debug = self.module.declare_func_in_func(*debug_func, self.builder.func);
                            self.builder.ins().call(local_debug, &[result_val]);
                        }
                        result_val
                        }
                    }
                    RValue::AllocateObject(class_name) => {
                        let class_def = self.program.classes.get(class_name)
                            .ok_or_else(|| format!("Class {} not found", class_name))?;
                        let total_size = 24 + (class_def.fields.len() as i64 * 8);

                        let alloc_func = self
                            .func_ids
                            .get("pace_alloc")
                            .expect("pace_alloc not declared");
                        let local_alloc = self
                            .module
                            .declare_func_in_func(*alloc_func, self.builder.func);

                        let metadata_id = *self.class_metadata_ids.get(class_name).unwrap();
                        let local_metadata_id = self
                            .module
                            .declare_data_in_func(metadata_id, self.builder.func);
                        let metadata_ptr = self
                            .builder
                            .ins()
                            .symbol_value(types::I64, local_metadata_id);

                        let size_val = self.builder.ins().iconst(types::I64, total_size);
                        let call_inst = self
                            .builder
                            .ins()
                            .call(local_alloc, &[size_val, metadata_ptr]);
                        let obj_ptr = self.builder.inst_results(call_inst)[0];

                        if class_def.is_actor {
                            let mailbox_create = self.func_ids.get("pace_actor_mailbox_create").unwrap();
                            let local_mailbox_create = self.module.declare_func_in_func(*mailbox_create, self.builder.func);
                            let mailbox_call = self.builder.ins().call(local_mailbox_create, &[obj_ptr]);
                            let mailbox_ptr = self.builder.inst_results(mailbox_call)[0];
                            
                            let mb_idx = class_def.fields.iter().position(|f| f == "__mailbox").unwrap();
                            let mb_offset = 24 + (mb_idx as i32 * 8);
                            self.builder.ins().store(cranelift_codegen::ir::MemFlagsData::new(), mailbox_ptr, obj_ptr, mb_offset);
                        }

                        obj_ptr
                    }
                    RValue::AllocateTask(poll_name) => {
                        let class_name = "Task";
                        let class_def = self.program.classes.get(class_name)
                            .ok_or_else(|| format!("Class {} not found", class_name))?;
                        let total_size = 24 + (class_def.fields.len() as i64 * 8);

                        let alloc_func = self
                            .func_ids
                            .get("pace_alloc")
                            .expect("pace_alloc not declared");
                        let local_alloc = self
                            .module
                            .declare_func_in_func(*alloc_func, self.builder.func);

                        let metadata_id = *self.class_metadata_ids.get(class_name).unwrap();
                        let local_metadata_id = self
                            .module
                            .declare_data_in_func(metadata_id, self.builder.func);
                        let metadata_ptr = self
                            .builder
                            .ins()
                            .symbol_value(types::I64, local_metadata_id);
                        let size_val = self.builder.ins().iconst(types::I64, total_size);

                        let call_inst = self
                            .builder
                            .ins()
                            .call(local_alloc, &[size_val, metadata_ptr]);
                        let obj_ptr = self.builder.inst_results(call_inst)[0];

                        // Get the poll_fn function pointer
                        let poll_func_id = self.func_ids.get(poll_name).ok_or_else(|| format!("Poll func {} not found", poll_name))?;
                        let local_poll = self.module.declare_func_in_func(*poll_func_id, self.builder.func);
                        let poll_ptr = self.builder.ins().func_addr(types::I64, local_poll);

                        // Store at offset 40 (poll_fn)
                        let poll_offset = self.builder.ins().iadd_imm_s(obj_ptr, 40);
                        self.builder.ins().store(ir::MemFlagsData::new(), poll_ptr, poll_offset, 0);

                        obj_ptr
                    }
                    RValue::AllocateStruct(struct_name) => {
                        let struct_def = self.program.classes.get(struct_name)
                            .ok_or_else(|| format!("Struct {} not found", struct_name))?;

                        let total_size = struct_def.fields.len() as u32 * 8;

                        let ss = self.builder.create_sized_stack_slot(ir::StackSlotData::new(
                            ir::StackSlotKind::ExplicitSlot,
                            total_size,
                            4,
                        ));
                        self.builder.ins().stack_addr(types::I64, ss, 0)
                    }
                    RValue::GetProperty(obj_val, prop_name, class_name) => {
                        let cl_obj = self.translate_value(obj_val)?;

                        let class_def = self.program.classes.get(class_name)
                            .ok_or_else(|| format!("Class {} not found", class_name))?;
                            
                        let idx = class_def.fields.iter().position(|f| f == prop_name)
                            .ok_or_else(|| format!("Property {} not found in class {}", prop_name, class_name))?;
                            
                        let offset = if class_def.is_struct {
                            idx as i32 * 8
                        } else {
                            24 + (idx as i32 * 8)
                        };

                        self.builder
                            .ins()
                            .load(types::I64, ir::MemFlagsData::new(), cl_obj, offset)
                    }
                    RValue::GetStaticProperty(class_name, prop_name) => {
                        let data_id = self.module.declare_data(
                            &format!("_pace_static_{}_{}", class_name, prop_name),
                            cranelift_module::Linkage::Export,
                            true,
                            false,
                        ).unwrap();
                        let local_data = self.module.declare_data_in_func(data_id, self.builder.func);
                        let ptr = self.builder.ins().symbol_value(types::I64, local_data);
                        self.builder.ins().load(types::I64, ir::MemFlagsData::new(), ptr, 0)
                    }
                    RValue::ForceUnwrap(inner) => {
                        let cl_val = self.translate_value(inner)?;
                        let is_null =
                            self.builder
                                .ins()
                                .icmp_imm_u(ir::condcodes::IntCC::Equal, cl_val, 0);
                        self.emit_panic_if(is_null, 1);
                        cl_val
                    }
                    RValue::Array(elements, is_ref) => {
                        let total_size = 32 + (elements.len() as i64 * 8);
                        let alloc_func = self
                            .func_ids
                            .get("pace_alloc")
                            .expect("pace_alloc not declared");
                        let local_alloc = self
                            .module
                            .declare_func_in_func(*alloc_func, self.builder.func);

                        let metadata_val = if *is_ref { -1i64 } else { -2i64 };
                        let metadata_ptr = self.builder.ins().iconst(types::I64, metadata_val);
                        let size_val = self.builder.ins().iconst(types::I64, total_size);
                        let call_inst = self
                            .builder
                            .ins()
                            .call(local_alloc, &[size_val, metadata_ptr]);
                        let array_ptr = self.builder.inst_results(call_inst)[0];

                        let len_val = self.builder.ins().iconst(types::I64, elements.len() as i64);
                        self.builder
                            .ins()
                            .store(ir::MemFlagsData::new(), len_val, array_ptr, 24);

                        for (i, elem) in elements.iter().enumerate() {
                            let cl_elem = self.translate_value(elem)?;
                            let offset = 32 + (i as i32 * 8);
                            self.builder.ins().store(
                                ir::MemFlagsData::new(),
                                cl_elem,
                                array_ptr,
                                offset,
                            );
                        }
                        array_ptr
                    }
                    RValue::ArrayRepeat(val, count, is_ref) => {
                        let cl_val = self.translate_value(val)?;
                        let cl_count = self.translate_value(count)?;

                        let alloc_repeat_func = self
                            .func_ids
                            .get("pace_alloc_array_repeat")
                            .expect("pace_alloc_array_repeat not declared");
                        let local_alloc_repeat = self
                            .module
                            .declare_func_in_func(*alloc_repeat_func, self.builder.func);

                        let metadata_val = if *is_ref { -1i64 } else { -2i64 };
                        let metadata_ptr = self.builder.ins().iconst(types::I64, metadata_val);

                        let call_inst = self
                            .builder
                            .ins()
                            .call(local_alloc_repeat, &[cl_count, cl_val, metadata_ptr]);
                        self.builder.inst_results(call_inst)[0]
                    }
                    RValue::ArrayLength(array) => {
                        let cl_array = self.translate_value(array)?;
                        self.builder
                            .ins()
                            .load(types::I64, ir::MemFlagsData::new(), cl_array, 24)
                    }
                    RValue::IndexGet(array, index) => {
                        let cl_array = self.translate_value(array)?;
                        let cl_index = self.translate_value(index)?;

                        // Bounds checking
                        let len_val = self.builder.ins().load(
                            types::I64,
                            ir::MemFlagsData::new(),
                            cl_array,
                            24,
                        );
                        let is_neg = self.builder.ins().icmp_imm_u(
                            ir::condcodes::IntCC::SignedLessThan,
                            cl_index,
                            0,
                        );
                        let is_gte = self.builder.ins().icmp(
                            ir::condcodes::IntCC::SignedGreaterThanOrEqual,
                            cl_index,
                            len_val,
                        );
                        let out_of_bounds = self.builder.ins().bor(is_neg, is_gte);
                        self.emit_panic_if(out_of_bounds, 2);

                        let byte_offset = self.builder.ins().imul_imm_s(cl_index, 8);
                        let base_offset = self.builder.ins().iadd_imm_s(cl_array, 32);
                        let element_ptr = self.builder.ins().iadd(base_offset, byte_offset);

                        self.builder
                            .ins()
                            .load(types::I64, ir::MemFlagsData::new(), element_ptr, 0)
                    }
                    RValue::ConstructVariant(_name, variant_idx, payloads) => {
                        let total_size = 24 + 8 + (payloads.len() as i64 * 8);

                        let alloc_func = self
                            .func_ids
                            .get("pace_alloc")
                            .expect("pace_alloc not declared");
                        let local_alloc = self
                            .module
                            .declare_func_in_func(*alloc_func, self.builder.func);

                        let metadata_id = *self
                            .enum_metadata_ids
                            .get(&(_name.clone(), *variant_idx))
                            .unwrap();
                        let local_metadata_id = self
                            .module
                            .declare_data_in_func(metadata_id, self.builder.func);
                        let metadata_ptr = self
                            .builder
                            .ins()
                            .symbol_value(types::I64, local_metadata_id);
                        let size_val = self.builder.ins().iconst(types::I64, total_size);

                        let call_inst = self
                            .builder
                            .ins()
                            .call(local_alloc, &[size_val, metadata_ptr]);
                        let obj_ptr = self.builder.inst_results(call_inst)[0];

                        let cl_tag = self.builder.ins().iconst(types::I64, *variant_idx as i64);
                        self.builder
                            .ins()
                            .store(ir::MemFlagsData::new(), cl_tag, obj_ptr, 24);

                        let enum_def = self.program.enums.get(_name).unwrap();
                        let variant_def = &enum_def.variants[*variant_idx];

                        for (i, p) in payloads.iter().enumerate() {
                            let cl_p = self.translate_value(p)?;
                            let offset = 32 + (i as i32 * 8);

                            if let Some(struct_name) = variant_def.struct_payloads.get(&i) {
                                let class_def = self.program.classes.get(struct_name).unwrap();
                                let struct_size = class_def.fields.len() as u32 * 8;

                                let struct_metadata_id = *self.class_metadata_ids.get(struct_name).unwrap();
                                let local_struct_metadata_id = self.module.declare_data_in_func(struct_metadata_id, self.builder.func);
                                let struct_metadata_ptr = self.builder.ins().symbol_value(types::I64, local_struct_metadata_id);
                                let struct_size_val = self.builder.ins().iconst(types::I64, struct_size as i64);
                                let alloc_size_val = self.builder.ins().iconst(types::I64, (struct_size + 32) as i64);

                                let struct_alloc_call = self.builder.ins().call(local_alloc, &[alloc_size_val, struct_metadata_ptr]);
                                let struct_obj_ptr = self.builder.inst_results(struct_alloc_call)[0];
                                let struct_payload_ptr = self.builder.ins().iadd_imm_s(struct_obj_ptr, 32);
                                self.builder.call_memcpy(self.module.target_config(), struct_payload_ptr, cl_p, struct_size_val);

                                self.builder.ins().store(
                                    ir::MemFlagsData::new(),
                                    struct_obj_ptr,
                                    obj_ptr,
                                    offset,
                                );
                            } else {
                                self.builder.ins().store(
                                    ir::MemFlagsData::new(),
                                    cl_p,
                                    obj_ptr,
                                    offset,
                                );
                            }
                        }

                        obj_ptr
                    }
                    RValue::ExtractPayload(enum_name, val, _variant_idx, field_idx, _is_ref) => {
                        let obj_ptr = self.translate_value(val)?;
                        let offset = 32 + (*field_idx as i32 * 8);
                        let extracted_val = self.builder.ins().load(
                            types::I64,
                            ir::MemFlagsData::new(),
                            obj_ptr,
                            offset,
                        );

                        let enum_def = match self.program.enums.get(enum_name) {
                            Some(d) => d,
                            None => return Err(format!("Enum {} not found in program.enums!", enum_name)),
                        };
                        let variant_def = &enum_def.variants[*_variant_idx];

                        if let Some(_struct_name) = variant_def.struct_payloads.get(field_idx) {
                            let extracted_payload_ptr = self.builder.ins().iadd_imm_s(extracted_val, 32);
                            extracted_payload_ptr
                        } else {
                            extracted_val
                        }
                    }
                    RValue::GetVariantTag(val) => {
                        let obj_ptr = self.translate_value(val)?;
                        self.builder
                            .ins()
                            .load(types::I64, ir::MemFlagsData::new(), obj_ptr, 24)
                    }
                    RValue::ActorMailboxPush(obj, method, args) => {
                        // 1. Call the method synchronously to obtain the Task object
                        let target_func_name = method.as_str();
                        let func_id = self.func_ids.get(target_func_name)
                            .ok_or_else(|| format!("Function {} not found for ActorMailboxPush", target_func_name))?;
                        let local_callee = self
                            .module
                            .declare_func_in_func(*func_id, self.builder.func);

                        let mut arg_vals = Vec::new();
                        for arg in args {
                            arg_vals.push(self.translate_value(arg)?);
                        }

                        let call_inst = self.builder.ins().call(local_callee, &arg_vals);
                        let task_ptr = self.builder.inst_results(call_inst)[0];

                        // 2. Extract __mailbox pointer from obj
                        let cl_obj = self.translate_value(obj)?;
                        // We need the class definition to find the offset
                        // Wait, we don't know the exact class name here easily if it's dynamic...
                        // But wait! Actor classes are statically typed at the call site.
                        // Wait, the RValue::ActorMailboxPush does not have the class_name!
                        // Let's look at get_struct_name or we can just extract it from method name?
                        // method name is like "MyActor::my_method". So class name is before "::".
                        let class_name = target_func_name.split("::").next()
                            .ok_or_else(|| format!("Invalid method name format for ActorMailboxPush: {}", target_func_name))?;
                        let class_def = self.program.classes.get(class_name)
                            .ok_or_else(|| format!("Class {} not found for ActorMailboxPush", class_name))?;
                        let mb_idx = class_def.fields.iter().position(|f| f == "__mailbox")
                            .ok_or_else(|| format!("Actor class {} is missing the __mailbox field", class_name))?;
                        let mb_offset = 24 + (mb_idx as i32 * 8);
                        
                        let mailbox_ptr = self.builder.ins().load(types::I64, cranelift_codegen::ir::MemFlagsData::new(), cl_obj, mb_offset);

                        // 3. Call pace_actor_mailbox_push(mailbox, task_ptr)
                        let push_func = self.func_ids.get("pace_actor_mailbox_push").unwrap();
                        let local_push = self.module.declare_func_in_func(*push_func, self.builder.func);
                        self.builder.ins().call(local_push, &[mailbox_ptr, task_ptr]);

                        // 4. Return the Task object
                        task_ptr
                    }
                    RValue::MethodCall(_, _, _) => {
                        return Err(
                            "Dynamic method calls not supported (Statically dispatched instead)"
                                .to_string(),
                        );
                    }
                    RValue::WeakUpgrade(inner) => {
                        let cl_val = self.translate_value(inner)?;
                        let weak_upgrade_func = self.func_ids.get("pace_weak_upgrade").unwrap();
                        let local_weak_upgrade = self
                            .module
                            .declare_func_in_func(*weak_upgrade_func, self.builder.func);
                        let call_inst = self.builder.ins().call(local_weak_upgrade, &[cl_val]);
                        self.builder.inst_results(call_inst)[0]
                    }
                    RValue::Spawn(task_val) => {
                        let cl_task = self.translate_value(task_val)?;

                        let spawn_func = self.func_ids.get("paceSpawnTask").unwrap();
                        let local_spawn = self.module.declare_func_in_func(*spawn_func, self.builder.func);
                        
                        self.builder.ins().call(local_spawn, &[cl_task]);
                        self.builder.ins().iconst(types::I64, 0) // Unit
                    }
                    RValue::GetTaskResult(task_val) => {
                        let cl_task = self.translate_value(task_val)?;
                        // Context is at offset 32
                        let ctx_offset = self.builder.ins().iadd_imm_s(cl_task, 32);
                        let ctx_ptr = self.builder.ins().load(types::I64, ir::MemFlagsData::new(), ctx_offset, 0);
                        // Result is at offset 32 in Context (state is at 24, result is at 32)
                        let result_offset = self.builder.ins().iadd_imm_s(ctx_ptr, 32);
                        self.builder.ins().load(types::I64, ir::MemFlagsData::new(), result_offset, 0)
                    }
                    RValue::Await(_) => {
                        unimplemented!("Await should be transformed away by MIR lowering or have specific implementations.")
                    }
                };

                let var = self.get_place_var(place);
                self.builder.def_var(var, cl_val);
                Ok(())
            }
            Inst::RegisterWaker(task_val, waker_val) => {
                let cl_task = self.translate_value(task_val)?;
                let cl_waker = self.translate_value(waker_val)?;
                let waker_offset = self.builder.ins().iadd_imm_s(cl_task, 48);
                self.builder.ins().store(ir::MemFlagsData::new(), cl_waker, waker_offset, 0);
                Ok(())
            }
            Inst::SetProperty(obj_val, prop_name, class_name, val_val, _is_ref) => {
                let cl_obj = self.translate_value(obj_val)?;
                let cl_val = self.translate_value(val_val)?;

                let class_def = self.program.classes.get(class_name)
                    .ok_or_else(|| format!("Class {} not found in program", class_name))?;
                let prop_idx = class_def.fields.iter().position(|f| f == prop_name)
                    .ok_or_else(|| format!("Property {} not found in class {}", prop_name, class_name))?;
                    
                let offset = if class_def.is_struct {
                    prop_idx as i32 * 8
                } else {
                    24 + (prop_idx as i32 * 8)
                };

                self.builder
                    .ins()
                    .store(ir::MemFlagsData::new(), cl_val, cl_obj, offset);
                Ok(())
            }
            Inst::SetStaticProperty(class_name, prop_name, value, _is_ref) => {
                let cl_val = self.translate_value(value)?;
                
                let data_id = self.module.declare_data(
                    &format!("_pace_static_{}_{}", class_name, prop_name),
                    cranelift_module::Linkage::Export,
                    true,
                    false,
                ).unwrap();
                let local_data = self.module.declare_data_in_func(data_id, self.builder.func);
                let ptr = self.builder.ins().symbol_value(types::I64, local_data);
                self.builder.ins().store(ir::MemFlagsData::new(), cl_val, ptr, 0);
                Ok(())
            }
            Inst::IndexSet(array, index, val) => {
                let cl_array = self.translate_value(array)?;
                let cl_index = self.translate_value(index)?;
                let cl_val = self.translate_value(val)?;

                // Bounds checking
                let len_val =
                    self.builder
                        .ins()
                        .load(types::I64, ir::MemFlagsData::new(), cl_array, 24);
                let is_neg = self.builder.ins().icmp_imm_u(
                    ir::condcodes::IntCC::SignedLessThan,
                    cl_index,
                    0,
                );
                let is_gte = self.builder.ins().icmp(
                    ir::condcodes::IntCC::SignedGreaterThanOrEqual,
                    cl_index,
                    len_val,
                );
                let out_of_bounds = self.builder.ins().bor(is_neg, is_gte);
                self.emit_panic_if(out_of_bounds, 2);

                let byte_offset = self.builder.ins().imul_imm_s(cl_index, 8);
                let base_offset = self.builder.ins().iadd_imm_s(cl_array, 32);
                let element_ptr = self.builder.ins().iadd(base_offset, byte_offset);

                self.builder
                    .ins()
                    .store(ir::MemFlagsData::new(), cl_val, element_ptr, 0);
                Ok(())
            }
            Inst::Retain(val) => {
                let cl_val = self.translate_value(val)?;
                let retain_func = self.func_ids.get("pace_retain").unwrap();
                let local_retain = self
                    .module
                    .declare_func_in_func(*retain_func, self.builder.func);
                self.builder.ins().call(local_retain, &[cl_val]);
                Ok(())
            }
            Inst::Release(val) => {
                let cl_val = self.translate_value(val)?;
                let release_func = self.func_ids.get("pace_release").unwrap();
                let local_release = self
                    .module
                    .declare_func_in_func(*release_func, self.builder.func);
                self.builder.ins().call(local_release, &[cl_val]);
                Ok(())
            }
            Inst::WeakRetain(val) => {
                let cl_val = self.translate_value(val)?;
                let retain_func = self.func_ids.get("pace_weak_retain").unwrap();
                let local_retain = self
                    .module
                    .declare_func_in_func(*retain_func, self.builder.func);
                self.builder.ins().call(local_retain, &[cl_val]);
                Ok(())
            }
            Inst::WeakRelease(val) => {
                let cl_val = self.translate_value(val)?;
                let release_func = self.func_ids.get("pace_weak_release").unwrap();
                let local_release = self
                    .module
                    .declare_func_in_func(*release_func, self.builder.func);
                self.builder.ins().call(local_release, &[cl_val]);
                Ok(())
            }
            Inst::MemCopy(dest, src, struct_name) => {
                let dest_ptr = self.translate_value(dest)?;
                let src_ptr = self.translate_value(src)?;
                let class_def = self.program.classes.get(struct_name).unwrap();
                let size = class_def.fields.len() as u32 * 8;
                
                let size_val = self.builder.ins().iconst(types::I64, size as i64);
                self.builder.call_memcpy(self.module.target_config(), dest_ptr, src_ptr, size_val);
                
                let retain_func = self.func_ids.get("pace_retain").unwrap();
                let local_retain = self.module.declare_func_in_func(*retain_func, self.builder.func);
                
                for (idx, field) in class_def.fields.iter().enumerate() {
                    if class_def.reference_fields.contains(field) {
                        let offset = idx as i32 * 8;
                        let field_ptr = self.builder.ins().load(types::I64, cranelift_codegen::ir::MemFlagsData::new(), dest_ptr, offset);
                        self.builder.ins().call(local_retain, &[field_ptr]);
                    }
                }
                Ok(())
            }
            Inst::DropStruct(ptr_val, struct_name) => {
                let struct_ptr = self.translate_value(ptr_val)?;
                let class_def = self.program.classes.get(struct_name).unwrap();
                
                let release_func = self.func_ids.get("pace_release").unwrap();
                let local_release = self.module.declare_func_in_func(*release_func, self.builder.func);
                
                for (idx, field) in class_def.fields.iter().enumerate() {
                    if class_def.reference_fields.contains(field) {
                        let offset = idx as i32 * 8;
                        let field_ptr = self.builder.ins().load(types::I64, cranelift_codegen::ir::MemFlagsData::new(), struct_ptr, offset);
                        self.builder.ins().call(local_release, &[field_ptr]);
                    }
                }
                Ok(())
            }
        }
    }
}
