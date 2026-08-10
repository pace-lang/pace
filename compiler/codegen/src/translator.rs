use std::collections::HashMap;

use cranelift_codegen::ir::{self, types, InstBuilder, Value as CraneliftValue, Block};
use cranelift_frontend::{FunctionBuilder, Variable};
use cranelift_module::{Module, FuncId, DataId};
use cranelift_object::ObjectModule;
use mir::{BlockId, Function, Inst, Place, RValue, Terminator, Value};
use ast::{BinaryOp, UnaryOp};

pub struct Translator<'a, 'b> {
    builder: &'a mut FunctionBuilder<'b>,
    module: &'a mut ObjectModule,
    program: &'a mir::Program,
    func_ids: &'a HashMap<String, FuncId>,
    class_metadata_ids: &'a HashMap<String, DataId>,
    variables: HashMap<String, Variable>,
    temporaries: HashMap<usize, Variable>,
    blocks: HashMap<BlockId, Block>,
    next_var_id: usize,
}

impl<'a, 'b> Translator<'a, 'b> {
    pub fn new(builder: &'a mut FunctionBuilder<'b>, module: &'a mut ObjectModule, program: &'a mir::Program, func_ids: &'a HashMap<String, FuncId>, class_metadata_ids: &'a HashMap<String, DataId>) -> Self {
        Self {
            builder,
            module,
            program,
            func_ids,
            class_metadata_ids,
            variables: HashMap::new(),
            temporaries: HashMap::new(),
            blocks: HashMap::new(),
            next_var_id: 0,
        }
    }

    pub fn translate(&mut self, function: &Function) -> Result<(), String> {
        // Pre-create all basic blocks
        for block in &function.blocks {
            let cl_block = self.builder.create_block();
            self.blocks.insert(block.id, cl_block);
        }

        // Setup entry block and its parameters
        if let Some(entry_block) = function.blocks.first() {
            let cl_entry = *self.blocks.get(&entry_block.id).unwrap();
            self.builder.append_block_params_for_function_params(cl_entry);
            self.builder.switch_to_block(cl_entry);

            let block_params = self.builder.block_params(cl_entry).to_vec();
            for (i, param_name) in function.parameters.iter().enumerate() {
                let var = self.get_or_create_var(param_name);
                self.builder.def_var(var, block_params[i]);
            }
        }

        // Translate each block
        for (i, block) in function.blocks.iter().enumerate() {
            let cl_block = *self.blocks.get(&block.id).unwrap();
            
            if i != 0 {
                self.builder.switch_to_block(cl_block);
            }
            
            for inst in &block.instructions {
                self.translate_inst(inst)?;
            }

            if let Some(terminator) = &block.terminator {
                self.translate_terminator(terminator)?;
            }
        }
        
        self.builder.seal_all_blocks();
        Ok(())
    }

    fn get_or_create_var(&mut self, name: &str) -> Variable {
        if let Some(var) = self.variables.get(name) {
            *var
        } else {
            let var = self.builder.declare_var(types::I64);
            self.variables.insert(name.to_string(), var);
            var
        }
    }

    fn get_or_create_temp(&mut self, id: usize) -> Variable {
        if let Some(var) = self.temporaries.get(&id) {
            *var
        } else {
            let var = self.builder.declare_var(types::I64);
            self.temporaries.insert(id, var);
            var
        }
    }

    fn get_place_var(&mut self, place: &Place) -> Variable {
        match place {
            Place::Var(name) => self.get_or_create_var(name),
            Place::Temp(id) => self.get_or_create_temp(*id),
        }
    }

    fn translate_value(&mut self, value: &Value) -> Result<CraneliftValue, String> {
        match value {
            Value::Int(i) => Ok(self.builder.ins().iconst(types::I64, *i)),
            Value::Float(f) => Ok(self.builder.ins().f64const(*f)),
            Value::Boolean(b) => Ok(self.builder.ins().iconst(types::I8, if *b { 1 } else { 0 })),
            Value::Place(place) => {
                let var = self.get_place_var(place);
                Ok(self.builder.use_var(var))
            }
            Value::Void | Value::Null => Ok(self.builder.ins().iconst(types::I64, 0)),
            _ => Err("Value variant not supported in M1".to_string()),
        }
    }

    fn translate_inst(&mut self, inst: &Inst) -> Result<(), String> {
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
                            BinaryOp::Equal => {
                                let c = self.builder.ins().icmp(ir::condcodes::IntCC::Equal, cl_left, cl_right);
                                self.builder.ins().uextend(types::I64, c)
                            }
                            BinaryOp::NotEqual => {
                                let c = self.builder.ins().icmp(ir::condcodes::IntCC::NotEqual, cl_left, cl_right);
                                self.builder.ins().uextend(types::I64, c)
                            }
                            BinaryOp::Less => {
                                let c = self.builder.ins().icmp(ir::condcodes::IntCC::SignedLessThan, cl_left, cl_right);
                                self.builder.ins().uextend(types::I64, c)
                            }
                            BinaryOp::LessEqual => {
                                let c = self.builder.ins().icmp(ir::condcodes::IntCC::SignedLessThanOrEqual, cl_left, cl_right);
                                self.builder.ins().uextend(types::I64, c)
                            }
                            BinaryOp::Greater => {
                                let c = self.builder.ins().icmp(ir::condcodes::IntCC::SignedGreaterThan, cl_left, cl_right);
                                self.builder.ins().uextend(types::I64, c)
                            }
                            BinaryOp::GreaterEqual => {
                                let c = self.builder.ins().icmp(ir::condcodes::IntCC::SignedGreaterThanOrEqual, cl_left, cl_right);
                                self.builder.ins().uextend(types::I64, c)
                            }
                        }
                    }
                    RValue::UnaryOp(op, right) => {
                        let cl_right = self.translate_value(right)?;
                        match op {
                            UnaryOp::Negate => self.builder.ins().ineg(cl_right),
                        }
                    }
                    RValue::Call(func_name, args) => {
                        let target_func_name = if func_name == "print" {
                            "pace_print"
                        } else {
                            func_name.as_str()
                        };
                        let func_id = self.func_ids.get(target_func_name)
                            .ok_or_else(|| format!("Function {} not found", target_func_name))?;
                        let local_callee = self.module.declare_func_in_func(*func_id, self.builder.func);
                        
                        let mut arg_vals = Vec::new();
                        for arg in args {
                            arg_vals.push(self.translate_value(arg)?);
                        }
                        
                        let call_inst = self.builder.ins().call(local_callee, &arg_vals);
                        let results = self.builder.inst_results(call_inst);
                        results[0]
                    }
                    RValue::AllocateObject(class_name) => {
                        let class_def = self.program.classes.get(class_name)
                            .unwrap_or_else(|| panic!("Class {} not found", class_name));
                        let total_size = 24 + (class_def.fields.len() as i64 * 8);
                        
                        let alloc_func = self.func_ids.get("pace_alloc")
                            .expect("pace_alloc not declared");
                        let local_alloc = self.module.declare_func_in_func(*alloc_func, self.builder.func);
                        
                        let metadata_id = *self.class_metadata_ids.get(class_name).unwrap();
                        let local_metadata_id = self.module.declare_data_in_func(metadata_id, self.builder.func);
                        let metadata_ptr = self.builder.ins().symbol_value(types::I64, local_metadata_id);
                        
                        let size_val = self.builder.ins().iconst(types::I64, total_size);
                        let call_inst = self.builder.ins().call(local_alloc, &[size_val, metadata_ptr]);
                        self.builder.inst_results(call_inst)[0]
                    }
                    RValue::GetProperty(obj_val, prop_name) => {
                        let cl_obj = self.translate_value(obj_val)?;
                        
                        // Find field offset by searching all classes
                        let mut offset = None;
                        for class_def in self.program.classes.values() {
                            if let Some(idx) = class_def.fields.iter().position(|f| f == prop_name) {
                                offset = Some(24 + (idx as i32 * 8));
                                break;
                            }
                        }
                        let offset = offset.unwrap_or_else(|| panic!("Property {} not found", prop_name));
                        
                        self.builder.ins().load(types::I64, ir::MemFlagsData::new(), cl_obj, offset)
                    }
                    RValue::ForceUnwrap(inner) => {
                        let cl_val = self.translate_value(inner)?;
                        let is_null = self.builder.ins().icmp_imm_u(ir::condcodes::IntCC::Equal, cl_val, 0);
                        self.builder.ins().trapnz(is_null, ir::TrapCode::unwrap_user(1)); // Panic on null unwrap
                        cl_val
                    }
                    RValue::Array(elements, is_ref) => {
                        let total_size = 24 + (elements.len() as i64 * 8);
                        let alloc_func = self.func_ids.get("pace_alloc").expect("pace_alloc not declared");
                        let local_alloc = self.module.declare_func_in_func(*alloc_func, self.builder.func);
                        
                        let metadata_val = if *is_ref { -1i64 } else { -2i64 };
                        let metadata_ptr = self.builder.ins().iconst(types::I64, metadata_val);
                        let size_val = self.builder.ins().iconst(types::I64, total_size);
                        let call_inst = self.builder.ins().call(local_alloc, &[size_val, metadata_ptr]);
                        let array_ptr = self.builder.inst_results(call_inst)[0];
                        
                        let len_val = self.builder.ins().iconst(types::I64, elements.len() as i64);
                        self.builder.ins().store(ir::MemFlagsData::new(), len_val, array_ptr, 24);
                        
                        for (i, elem) in elements.iter().enumerate() {
                            let cl_elem = self.translate_value(elem)?;
                            let offset = 32 + (i as i32 * 8);
                            self.builder.ins().store(ir::MemFlagsData::new(), cl_elem, array_ptr, offset);
                        }
                        array_ptr
                    }
                    RValue::ArrayRepeat(val, count, is_ref) => {
                        let cl_val = self.translate_value(val)?;
                        let cl_count = self.translate_value(count)?;
                        
                        let alloc_repeat_func = self.func_ids.get("pace_alloc_array_repeat").expect("pace_alloc_array_repeat not declared");
                        let local_alloc_repeat = self.module.declare_func_in_func(*alloc_repeat_func, self.builder.func);
                        
                        let metadata_val = if *is_ref { -1i64 } else { -2i64 };
                        let metadata_ptr = self.builder.ins().iconst(types::I64, metadata_val);
                        
                        let call_inst = self.builder.ins().call(local_alloc_repeat, &[cl_count, cl_val, metadata_ptr]);
                        self.builder.inst_results(call_inst)[0]
                    }
                    RValue::IndexGet(array, index) => {
                        let cl_array = self.translate_value(array)?;
                        let cl_index = self.translate_value(index)?;
                        
                        // Bounds checking
                        let len_val = self.builder.ins().load(types::I64, ir::MemFlagsData::new(), cl_array, 24);
                        let is_neg = self.builder.ins().icmp_imm_u(ir::condcodes::IntCC::SignedLessThan, cl_index, 0);
                        let is_gte = self.builder.ins().icmp(ir::condcodes::IntCC::SignedGreaterThanOrEqual, cl_index, len_val);
                        let out_of_bounds = self.builder.ins().bor(is_neg, is_gte);
                        self.builder.ins().trapnz(out_of_bounds, ir::TrapCode::unwrap_user(2)); // Panic out of bounds
                        
                        let byte_offset = self.builder.ins().imul_imm(cl_index, 8);
                        let base_offset = self.builder.ins().iadd_imm(cl_array, 32);
                        let element_ptr = self.builder.ins().iadd(base_offset, byte_offset);
                        
                        self.builder.ins().load(types::I64, ir::MemFlagsData::new(), element_ptr, 0)
                    }
                    _ => return Err("RValue variant not supported in M1-M9".to_string()),
                };
                
                let var = self.get_place_var(place);
                self.builder.def_var(var, cl_val);
                Ok(())
            }
            Inst::SetProperty(obj_val, prop_name, val_val) => {
                let cl_obj = self.translate_value(obj_val)?;
                let cl_val = self.translate_value(val_val)?;
                
                // Find field offset
                let mut offset = None;
                for class_def in self.program.classes.values() {
                    if let Some(idx) = class_def.fields.iter().position(|f| f == prop_name) {
                        offset = Some(24 + (idx as i32 * 8));
                        break;
                    }
                }
                let offset = offset.unwrap_or_else(|| panic!("Property {} not found", prop_name));
                
                self.builder.ins().store(ir::MemFlagsData::new(), cl_val, cl_obj, offset);
                Ok(())
            }
            Inst::IndexSet(array, index, val) => {
                let cl_array = self.translate_value(array)?;
                let cl_index = self.translate_value(index)?;
                let cl_val = self.translate_value(val)?;
                
                // Bounds checking
                let len_val = self.builder.ins().load(types::I64, ir::MemFlagsData::new(), cl_array, 24);
                let is_neg = self.builder.ins().icmp_imm_u(ir::condcodes::IntCC::SignedLessThan, cl_index, 0);
                let is_gte = self.builder.ins().icmp(ir::condcodes::IntCC::SignedGreaterThanOrEqual, cl_index, len_val);
                let out_of_bounds = self.builder.ins().bor(is_neg, is_gte);
                self.builder.ins().trapnz(out_of_bounds, ir::TrapCode::unwrap_user(2)); // Panic out of bounds
                
                let byte_offset = self.builder.ins().imul_imm(cl_index, 8);
                let base_offset = self.builder.ins().iadd_imm(cl_array, 32);
                let element_ptr = self.builder.ins().iadd(base_offset, byte_offset);
                
                self.builder.ins().store(ir::MemFlagsData::new(), cl_val, element_ptr, 0);
                Ok(())
            }
            Inst::Retain(val) => {
                let cl_val = self.translate_value(val)?;
                let retain_func = self.func_ids.get("pace_retain").unwrap();
                let local_retain = self.module.declare_func_in_func(*retain_func, self.builder.func);
                self.builder.ins().call(local_retain, &[cl_val]);
                Ok(())
            }
            Inst::Release(val) => {
                let cl_val = self.translate_value(val)?;
                let release_func = self.func_ids.get("pace_release").unwrap();
                let local_release = self.module.declare_func_in_func(*release_func, self.builder.func);
                self.builder.ins().call(local_release, &[cl_val]);
                Ok(())
            }
            Inst::WeakRetain(val) => {
                let cl_val = self.translate_value(val)?;
                let retain_func = self.func_ids.get("pace_weak_retain").unwrap();
                let local_retain = self.module.declare_func_in_func(*retain_func, self.builder.func);
                self.builder.ins().call(local_retain, &[cl_val]);
                Ok(())
            }
            Inst::WeakRelease(val) => {
                let cl_val = self.translate_value(val)?;
                let release_func = self.func_ids.get("pace_weak_release").unwrap();
                let local_release = self.module.declare_func_in_func(*release_func, self.builder.func);
                self.builder.ins().call(local_release, &[cl_val]);
                Ok(())
            }
            _ => Err("Inst variant not supported in M1-M5".to_string()),
        }
    }

    fn translate_terminator(&mut self, terminator: &Terminator) -> Result<(), String> {
        match terminator {
            Terminator::Jump(block_id) => {
                let cl_block = *self.blocks.get(block_id).unwrap();
                self.builder.ins().jump(cl_block, &[]);
            }
            Terminator::Branch { cond, then_block, else_block } => {
                let cl_cond = self.translate_value(cond)?;
                // Cranelift uses 0 for false and non-zero for true
                let cl_then = *self.blocks.get(then_block).unwrap();
                let cl_else = *self.blocks.get(else_block).unwrap();
                self.builder.ins().brif(cl_cond, cl_then, &[], cl_else, &[]);
            }
            Terminator::Return(opt_val) => {
                let cl_val = if let Some(val) = opt_val {
                    self.translate_value(val)?
                } else {
                    self.builder.ins().iconst(types::I64, 0)
                };
                self.builder.ins().return_(&[cl_val]);
            }
        }
        Ok(())
    }
}
