use std::collections::HashMap;

use ast::{BinaryOp, UnaryOp};
use cranelift_codegen::ir::{self, Block, InstBuilder, Value as CraneliftValue, types};
use cranelift_frontend::{FunctionBuilder, Variable};
use cranelift_module::{DataId, FuncId, Module};
use cranelift_object::ObjectModule;
use mir::{BlockId, Function, Inst, Place, RValue, Terminator, Value};

pub struct Translator<'a, 'b> {
    builder: &'a mut FunctionBuilder<'b>,
    module: &'a mut ObjectModule,
    program: &'a mir::Program,
    func_ids: &'a HashMap<String, FuncId>,
    class_metadata_ids: &'a HashMap<String, DataId>,
    enum_metadata_ids: &'a HashMap<(String, usize), DataId>,
    variables: HashMap<String, Variable>,
    temporaries: HashMap<usize, Variable>,
    blocks: HashMap<BlockId, Block>,
}

impl<'a, 'b> Translator<'a, 'b> {
    pub fn new(
        builder: &'a mut FunctionBuilder<'b>,
        module: &'a mut ObjectModule,
        program: &'a mir::Program,
        func_ids: &'a HashMap<String, FuncId>,
        class_metadata_ids: &'a HashMap<String, DataId>,
        enum_metadata_ids: &'a HashMap<(String, usize), DataId>,
    ) -> Self {
        Self {
            builder,
            module,
            program,
            func_ids,
            class_metadata_ids,
            enum_metadata_ids,
            variables: HashMap::new(),
            temporaries: HashMap::new(),
            blocks: HashMap::new(),
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
            self.builder
                .append_block_params_for_function_params(cl_entry);
            self.builder.switch_to_block(cl_entry);

            let block_params = self.builder.block_params(cl_entry).to_vec();
            for (i, param_name) in function.parameters.iter().enumerate() {
                let var = self.get_or_create_var(param_name);
                self.builder.def_var(var, block_params[i]);
            }

            // Pre-allocate stack slots for all struct variables and temporaries
            for (place, struct_name) in &function.struct_places {
                let class_def = self.program.classes.get(struct_name).unwrap();
                let total_size = class_def.fields.len() as u32 * 8;
                let ss = self.builder.create_sized_stack_slot(cranelift_codegen::ir::StackSlotData::new(
                    cranelift_codegen::ir::StackSlotKind::ExplicitSlot,
                    total_size,
                    3, // 8-byte alignment
                ));
                let ptr = self.builder.ins().stack_addr(types::I64, ss, 0);
                
                let var = match place {
                    mir::Place::Var(name) => self.get_or_create_var(name),
                    mir::Place::Temp(id) => self.get_or_create_temp(*id),
                };
                self.builder.def_var(var, ptr);
            }

            let zero_val = self.builder.ins().iconst(types::I64, 0);
            let mut defined_vars = std::collections::HashSet::new();
            
            for param_name in &function.parameters {
                defined_vars.insert(mir::Place::Var(param_name.clone()));
            }
            for (place, _) in &function.struct_places {
                defined_vars.insert(place.clone());
            }

            for block in &function.blocks {
                for inst in &block.instructions {
                    if let Inst::Assign(place, _) = inst {
                        if !defined_vars.contains(place) {
                            defined_vars.insert(place.clone());
                            let var = match place {
                                mir::Place::Var(name) => self.get_or_create_var(name),
                                mir::Place::Temp(id) => self.get_or_create_temp(*id),
                            };
                            self.builder.def_var(var, zero_val);
                        }
                    }
                }
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

    pub(super) fn get_or_create_var(&mut self, name: &str) -> Variable {
        if let Some(var) = self.variables.get(name) {
            *var
        } else {
            let var = self.builder.declare_var(types::I64);
            self.variables.insert(name.to_string(), var);
            var
        }
    }

    pub(super) fn get_or_create_temp(&mut self, id: usize) -> Variable {
        if let Some(var) = self.temporaries.get(&id) {
            *var
        } else {
            let var = self.builder.declare_var(types::I64);
            self.temporaries.insert(id, var);
            var
        }
    }

    pub(super) fn get_place_var(&mut self, place: &Place) -> Variable {
        match place {
            Place::Var(name) => self.get_or_create_var(name),
            Place::Temp(id) => self.get_or_create_temp(*id),
        }
    }

    pub(super) fn emit_panic_if(&mut self, condition: CraneliftValue, code: i64) {
        let panic_block = self.builder.create_block();
        let cont_block = self.builder.create_block();
        self.builder
            .ins()
            .brif(condition, panic_block, &[], cont_block, &[]);

        self.builder.switch_to_block(panic_block);
        let panic_func = self
            .func_ids
            .get("pacePanic")
            .expect("pacePanic not declared");
        let local_panic = self
            .module
            .declare_func_in_func(*panic_func, self.builder.func);
        let code_val = self.builder.ins().iconst(types::I64, code);
        self.builder.ins().call(local_panic, &[code_val]);
        self.builder
            .ins()
            .trap(ir::TrapCode::unwrap_user(code as u8));

        self.builder.switch_to_block(cont_block);
    }
    }
mod value;
mod inst;
mod terminator;
