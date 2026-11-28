mod emit;
mod types;

use pace_mir::MirProgram;
use pace_ty::Ty;

pub struct CGenerator {
    output: String,
    #[allow(clippy::type_complexity)]
    enum_defs: std::collections::HashMap<
        pace_hir::HirId,
        Vec<(String, Option<Vec<(String, pace_ty::Ty)>>)>,
    >,
}

impl Default for CGenerator {
    fn default() -> Self {
        Self::new()
    }
}

impl CGenerator {
    pub fn new() -> Self {
        Self {
            output: String::with_capacity(1024 * 1024),
            enum_defs: std::collections::HashMap::new(),
        }
    }

    pub fn generate(&mut self, program: &MirProgram) -> String {
        self.output.push_str("#include <stdio.h>\n");
        self.output.push_str("#include <stdlib.h>\n");
        self.output.push_str("#include <string.h>\n");
        self.output.push_str("#ifndef _WIN32\n");
        self.output.push_str("#include <pthread.h>\n");
        self.output.push_str("#endif\n");
        self.output.push_str("#include <math.h>\n#include <stdio.h>\n#include \"pace_runtime.h\"\n\nstatic inline void* pace_retain_ptr(void* p) { if (p) pace_retain(p); return p; }\n\n");

        self.output.push_str("typedef struct PaceClosure {\n");
        self.output.push_str("    void* func;\n");
        self.output.push_str("    void* env;\n");
        self.output.push_str("} PaceClosure;\n\n");

        self.output.push_str("typedef struct PaceOptInt { int has_value; long long value; } PaceOptInt;\n");
        self.output.push_str("typedef struct PaceOptFloat { int has_value; double value; } PaceOptFloat;\n");
        self.output.push_str("typedef struct PaceOptBool { int has_value; long long value; } PaceOptBool;\n");
        self.output.push_str("static inline PaceOptInt pace_opt_int_from_ptr(void* p) { return p ? (PaceOptInt){1, (long long)(intptr_t)p} : (PaceOptInt){0, 0}; }\n");
        self.output.push_str("static inline PaceOptFloat pace_opt_float_from_ptr(void* p) { return p ? (PaceOptFloat){1, *(double*)p} : (PaceOptFloat){0, 0}; }\n");
        self.output.push_str("static inline PaceOptBool pace_opt_bool_from_ptr(void* p) { return p ? (PaceOptBool){1, (long long)(intptr_t)p} : (PaceOptBool){0, 0}; }\n\n");

        for func in &program.functions {
            if let Some(env_layout) = &func.env_layout {
                self.output.push_str(&format!("struct __Env_{} {{\n", func.name));
                for (local, ty) in env_layout {
                    self.output.push_str(&format!("    {} _{};\n", self.emit_c_type(ty), local.0));
                }
                self.output.push_str("};\n\n");
            }
        }

        self.enum_defs = program.enum_defs.clone();

        for (id, fields) in &program.struct_defs {
            self.output.push_str(&format!("struct pace_{} {{\n", id.0));
            for pace_ty::ResolvedField {
                name: fname,
                ty: fty,
                ..
            } in fields
            {
                self.output
                    .push_str(&format!("    {} {};\n", self.emit_c_type(fty), fname));
            }
            self.output.push_str("};\n\n");
        }

        for (id, variants) in &program.enum_defs {
            self.output.push_str(&format!("struct pace_{} {{\n", id.0));
            self.output.push_str("    long long tag;\n");
            self.output.push_str("    union {\n");
            for (v_name, v_fields) in variants {
                if let Some(fields) = v_fields {
                    self.output.push_str("        struct {\n");
                    for (fname, fty) in fields {
                        self.output.push_str(&format!(
                            "            {} {};\n",
                            self.emit_c_type(fty),
                            fname
                        ));
                    }
                    self.output.push_str(&format!("        }} {};\n", v_name));
                }
            }
            self.output.push_str("    } payload;\n");
            self.output.push_str("};\n\n");
        }

        // Forward declarations of class structs
        for id in program.class_defs.keys() {
            self.output.push_str(&format!("struct pace_{};\n", id.0));
        }
        self.output.push('\n');

        for (id, methods) in &program.class_vtables {
            self.output
                .push_str(&format!("struct pace_{}_vtable {{\n", id.0));
            for (i, pace_ty::VTableEntry { ty, .. }) in methods.iter().enumerate() {
                if let Ty::Function(params, ret) = ty {
                    self.output
                        .push_str(&format!("    {} (*m{})(", self.emit_c_type(ret), i));
                    if params.is_empty() {
                        self.output.push_str("void");
                    } else {
                        for (j, p) in params.iter().enumerate() {
                            if j > 0 {
                                self.output.push_str(", ");
                            }
                            self.output.push_str(&self.emit_c_type(p));
                        }
                    }
                    self.output.push_str(");\n");
                }
            }
            self.output.push_str("};\n\n");
        }

        for (id, fields) in &program.class_defs {
            self.output.push_str(&format!("struct pace_{} {{\n", id.0));
            self.output
                .push_str(&format!("    struct pace_{}_vtable* vtable;\n", id.0));
            if program.actor_defs.contains(id) {
                self.output.push_str("    pthread_mutex_t __mailbox_mutex;\n");
                self.output.push_str("    void* __mailbox_head;\n");
                self.output.push_str("    void* __mailbox_tail;\n");
                self.output.push_str("    int __is_running;\n");
            }
            for pace_ty::ResolvedField {
                name: fname,
                ty: fty,
                ..
            } in fields
            {
                self.output
                    .push_str(&format!("    {} {};\n", self.emit_c_type(fty), fname));
            }
            self.output.push_str("};\n\n");

            self.output
                .push_str(&format!("void pace_{}_deinit(void* ptr) {{\n", id.0));
            self.output.push_str(&format!(
                "    struct pace_{}* self = (struct pace_{}*)ptr;\n",
                id.0, id.0
            ));
            for pace_ty::ResolvedField {
                name: fname,
                ty: fty,
                ..
            } in fields
            {
                if matches!(fty, Ty::Class(_)) || matches!(fty, Ty::String) {
                    self.output
                        .push_str(&format!("    pace_release(self->{});\n", fname));
                }
            }
            self.output.push_str("}\n\n");
        }

        for (name, ty) in &program.global_vars {
            let c_ty = self.emit_c_type(ty);
            self.output.push_str(&format!("{} {};\n", c_ty, name));
        }
        self.output.push('\n');

        // Forward declare functions
        for func in &program.functions {
            let func_name = if func.name == "main" {
                "pace_main"
            } else {
                &func.name
            };
            self.output.push_str(&format!(
                "{} {}(",
                self.emit_c_type(&func.return_type),
                func_name
            ));
            if func.env_layout.is_some() {
                self.output.push_str("void* __env");
                if !func.params.is_empty() {
                    self.output.push_str(", ");
                }
            } else if func.params.is_empty() {
                self.output.push_str("void");
            }
            
            for (i, p) in func.params.iter().enumerate() {
                if i > 0 {
                    self.output.push_str(", ");
                }
                let c_ty = self.emit_c_type(&func.body.locals[p.0 as usize]);
                self.output.push_str(&format!("{} _{}", c_ty, i));
            }
            self.output.push_str(");\n");
        }
        self.output.push('\n');

        // V-Table globals
        for (id, methods) in &program.class_vtables {
            self.output.push_str(&format!(
                "struct pace_{}_vtable pace_{}_vtable_inst = {{\n",
                id.0, id.0
            ));
            for pace_ty::VTableEntry {
                mangled_name: func_name,
                ..
            } in methods
            {
                self.output.push_str(&format!("    {},\n", func_name));
            }
            self.output.push_str("};\n\n");
        }

        for func in &program.functions {
            self.generate_function(func);
            self.output.push('\n');
        }

        // Generate pace_init for top-level code (like static initializers)
        self.output.push_str("void pace_init() {\n");
        for (i, ty) in program.main_body.locals.iter().enumerate() {
            let c_ty = self.emit_c_type(ty);
            if c_ty != "void" {
                let def_val = self.emit_c_default_val(ty);
                self.output
                    .push_str(&format!("    {} _{} = {};\n", c_ty, i, def_val));
            }
        }
        self.output.push('\n');
        for (i, block) in program.main_body.blocks.iter().enumerate() {
            self.output.push_str(&format!("init_bb_{}:\n", i));
            self.generate_block(block, &program.main_body.locals);
        }
        self.output.push_str("}\n\n");

        self.output.push_str("int main(int argc, char** argv) {\n");
        self.output.push_str("    pace_set_args(argc, argv);\n");
        self.output.push_str("    pace_thread_pool_init(4);\n");
        self.output.push_str("    pace_event_loop_init();\n");
        self.output.push_str("    pace_init();\n");
        let main_func = program.functions.iter().find(|f| f.name == "main");
        if let Some(m) = main_func {
            self.output.push_str("    pace_main();\n");
            if m.is_async {
                self.output.push_str("    pace_event_loop_run();\n");
            }
        }
        self.output.push_str("    pace_thread_pool_shutdown();\n");
        self.output.push_str("    return 0;\n");
        self.output.push_str("}\n");
        self.output.clone()
    }

}
