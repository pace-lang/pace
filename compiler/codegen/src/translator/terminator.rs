use super::Translator;
use cranelift_codegen::ir::{self, InstBuilder, types};
use mir::Terminator;

impl<'a, 'b> Translator<'a, 'b> {

    pub(super) fn translate_terminator(&mut self, terminator: &Terminator) -> Result<(), String> {
        match terminator {
            Terminator::Jump(block_id) => {
                let cl_block = *self.blocks.get(block_id).unwrap();
                self.builder.ins().jump(cl_block, &[]);
            }
            Terminator::Branch {
                cond,
                then_block,
                else_block,
            } => {
                let cl_cond = self.translate_value(cond)?;
                let cl_then = *self.blocks.get(then_block).unwrap();
                let cl_else = *self.blocks.get(else_block).unwrap();
                self.builder.ins().brif(cl_cond, cl_then, &[], cl_else, &[]);
            }
            Terminator::Switch {
                cond,
                cases,
                default,
            } => {
                let cond_val = self.translate_value(cond)?;

                for (i, (var_idx, block_id)) in cases.iter().enumerate() {
                    let target = *self.blocks.get(block_id).unwrap();
                    let var_val = self.builder.ins().iconst(types::I64, *var_idx as i64);
                    let is_eq =
                        self.builder
                            .ins()
                            .icmp(ir::condcodes::IntCC::Equal, cond_val, var_val);

                    if i == cases.len() - 1 && default.is_none() {
                        self.builder.ins().jump(target, &[]);
                    } else {
                        let next_block = self.builder.create_block();
                        self.builder.ins().brif(is_eq, target, &[], next_block, &[]);
                        self.builder.switch_to_block(next_block);
                    }
                }

                if let Some(def) = default {
                    let target = *self.blocks.get(def).unwrap();
                    self.builder.ins().jump(target, &[]);
                }
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
