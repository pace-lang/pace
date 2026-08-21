use super::Translator;
use cranelift_module::Module;
use cranelift_codegen::ir::{InstBuilder, Value as CraneliftValue, types};
use mir::Value;

impl<'a, 'b> Translator<'a, 'b> {

    pub(super) fn translate_value(&mut self, value: &Value) -> Result<CraneliftValue, String> {
        match value {
            Value::Int(i) => Ok(self.builder.ins().iconst(types::I64, *i)),
            Value::Float(f) => {
                let f_val = self.builder.ins().f64const(*f);
                Ok(self.builder.ins().bitcast(
                    types::I64,
                    cranelift_codegen::ir::MemFlagsData::new(),
                    f_val,
                ))
            }
            Value::Boolean(b) => Ok(self
                .builder
                .ins()
                .iconst(types::I64, if *b { 1 } else { 0 })),
            Value::Place(place) => {
                let var = self.get_place_var(place);
                Ok(self.builder.use_var(var))
            }
            Value::Void | Value::Null => Ok(self.builder.ins().iconst(types::I64, 0)),
            Value::String(s) => {
                use cranelift_module::DataDescription;
                let data_id = self
                    .module
                    .declare_data(
                        &format!(
                            "str_{}",
                            {
                                static STR_COUNTER: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
                                STR_COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
                            }
                        ),
                        cranelift_module::Linkage::Local,
                        false,
                        false,
                    )
                    .unwrap();
                let mut data_desc = DataDescription::new();
                data_desc.set_align(8);
                let mut bytes = Vec::new();

                // ARC Header for static string literal
                // strong_count: high value so it never hits 0 and triggers free() on .rodata
                bytes.extend_from_slice(&(0x7FFFFFFF_FFFFFFFF_u64.to_le_bytes()));
                // weak_count
                bytes.extend_from_slice(&(1_u64.to_le_bytes()));
                // metadata = !1 (-2: primitive array)
                bytes.extend_from_slice(&((!1_u64).to_le_bytes()));

                // Payload
                bytes.extend_from_slice(s.as_bytes());
                bytes.push(0); // Null terminator

                data_desc.define(bytes.into_boxed_slice());
                self.module.define_data(data_id, &data_desc).unwrap();
                let local_data = self.module.declare_data_in_func(data_id, self.builder.func);
                let base_ptr = self.builder.ins().symbol_value(types::I64, local_data);

                // Return pointer to the header (ARC requirement)
                Ok(base_ptr)
            }
            Value::Object(_) | Value::Array(_) | Value::EnumVariant(..) => Err(
                "Value::Object, Value::Array, and Value::EnumVariant are runtime-only variants"
                    .to_string(),
            ),
        }
    }
}
