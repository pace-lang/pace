use miette::{IntoDiagnostic, Report, Result};
use pace_ast::Stmt;

use miette::Diagnostic;
use thiserror::Error;

pub mod fold;
pub mod inline;
pub mod monomorphize;
pub mod resolve;
pub mod shake;

#[derive(Error, Diagnostic, Debug)]
#[error("Found multiple type errors")]
#[diagnostic(code(pace::multiple_type_errors))]
pub struct MultipleTypeErrors {
    #[related]
    pub errors: Vec<pace_ty::TypeError>,
}

pub struct Compiler {
    pub session: pace_session::Session,
}

impl Compiler {
    pub fn new(session: pace_session::Session) -> Self {
        // Ensure pace-runtime is linked in the driver for JIT
        let _ = pace_runtime::__pace_print_int as *const () as usize;
        let _ = pace_runtime::__pace_print_float as *const () as usize;
        let _ = pace_runtime::__pace_print_string as *const () as usize;
        let _ = pace_runtime::__pace_malloc as *const () as usize;
        let _ = pace_runtime::__pace_hash as *const () as usize;
        let _ = pace_runtime::__pace_time as *const () as usize;
        let _ = pace_runtime::__pace_get_year as *const () as usize;

        // Force linkage of FS and HTTP runtime functions
        let _ = pace_runtime::__pace_fs_write as *const () as usize;
        let _ = pace_runtime::__pace_fs_exists as *const () as usize;
        let _ = pace_runtime::__pace_fs_read as *const () as usize;
        let _ = pace_runtime::__pace_http_get as *const () as usize;

        // Force linkage of StringBuilder runtime functions
        let _ = pace_runtime::__pace_sb_new as *const () as usize;
        let _ = pace_runtime::__pace_sb_append as *const () as usize;
        let _ = pace_runtime::__pace_sb_build as *const () as usize;
        let _ = pace_runtime::__pace_sb_free as *const () as usize;
        Self { session }
    }

    fn load_file(
        &self,
        arena: &mut pace_ast::arena::AstArena,
        path: &std::path::Path,
        module_name: &str,
        visited: &mut std::collections::HashSet<std::path::PathBuf>,
        override_path: Option<&std::path::Path>,
        override_src: Option<&str>,
        sources: &mut std::collections::HashMap<ustr::Ustr, String>,
    ) -> Result<Vec<pace_ast::arena::StmtId>> {
        let path_buf = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if visited.contains(&path_buf) {
            return Ok(Vec::new()); // Already loaded, prevent cycles
        }
        visited.insert(path_buf.clone());

        let src = if let Some(op) = override_path {
            if path_buf == *op {
                override_src.unwrap_or("").to_string()
            } else {
                std::fs::read_to_string(&path_buf).into_diagnostic()?
            }
        } else {
            std::fs::read_to_string(&path_buf).into_diagnostic()?
        };

        sources.insert(ustr::Ustr::from(module_name), src.clone());

        let (mut ast, _comments) =
            match pace_parser::parse(arena, &src, &path.display().to_string()) {
                Ok(res) => res,
                Err(parse_errors) => {
                    return Err(Report::new(pace_errors::MultipleSyntaxErrors {
                        errors: parse_errors,
                    }));
                }
            };

        // Auto-inject pace:prelude if not the core or prelude library itself
        if path_buf.file_stem().unwrap_or_default() != "core"
            && path_buf.file_stem().unwrap_or_default() != "prelude"
        {
            let import_stmt_id = arena.alloc_stmt(Stmt::Import {
                path: ustr::Ustr::from("pace:prelude"),
                alias: None,
                show: None,
                hide: None,
            }, pace_ast::Span::default());
            ast.insert(0, import_stmt_id);
        }

        // Resolve imports recursively
        let mut final_ast = Vec::new();
        for i in 0..ast.len() {
            let stmt_id = ast[i];
            let mut resolved = None;
            if let Stmt::Import {
                path: import_path, ..
            }
            | Stmt::Export { path: import_path } = arena.get_stmt(stmt_id)
            {
                let resolved_path = self.session.resolve_import_path(import_path.as_str(), &path_buf)?;

                if resolved_path.exists() {
                    let mod_name =
                        if import_path.starts_with("./") || import_path.starts_with("../") {
                            resolved_path
                                .canonicalize()
                                .unwrap_or_else(|_| resolved_path.clone())
                                .to_string_lossy()
                                .into_owned()
                        } else {
                            format!("pkg:{}", import_path.as_str())
                        };
                    resolved = Some((mod_name, resolved_path));
                } else {
                    return Err(miette::miette!(
                        "Cannot find module '{}' at {:?}",
                        import_path.as_str(),
                        resolved_path
                    ));
                }
            }

            if let Some((mod_name, resolved_path)) = resolved {
                // Mutate the statement in the arena
                if let Stmt::Import { path, .. } | Stmt::Export { path } =
                    arena.get_stmt_mut(stmt_id)
                {
                    *path = ustr::Ustr::from(&mod_name);
                }

                let mut imported_ast = self.load_file(
                    arena,
                    &resolved_path,
                    &mod_name,
                    visited,
                    override_path,
                    override_src,
                    sources,
                )?;
                final_ast.append(&mut imported_ast);
            }
        }

        // Append current file's AST after its dependencies
        let module_stmt_id = arena.alloc_stmt(Stmt::Module {
            name: ustr::Ustr::from(module_name),
            body: ast,
        }, pace_ast::Span::default());
        final_ast.push(module_stmt_id);

        Ok(final_ast)
    }

    fn process_ast_pipeline(
        &self,
        arena: &mut pace_ast::arena::AstArena,
        ast: Vec<pace_ast::arena::StmtId>,
        sources: std::collections::HashMap<ustr::Ustr, String>,
        module_path: &str,
    ) -> Result<(
        Vec<pace_ast::arena::StmtId>,
        Vec<pace_errors::SemanticWarning>,
        Vec<pace_ty::TypeError>,
        pace_ty::Environment,
        pace_hir::arena::HirArena,
        pace_mir::MirProgram,
    )> {
        // Symbol Resolution and Name Mangling pass
        let resolved_ast = resolve::SymbolResolver::run(arena, ast)?;

        // Monomorphization without flattening
        let mono_ast = monomorphize::Monomorphizer::run(arena, resolved_ast.clone())
            .unwrap_or_else(|_| resolved_ast.clone());

        // Apply AST Inlining
        let mono_ast = inline::Inliner::run(arena, mono_ast);

        // Apply Constant Folding
        let mono_ast = fold::ConstantFolder::run(arena, mono_ast);

        // Apply Dead Code Elimination (Tree Shaking)
        let mono_ast = shake::TreeShaker::run(arena, mono_ast);

        // Lower AST to HIR
        let builder = pace_hir::lower::HirBuilder::new(arena);
        let (hir_arena, hir_stmts) = builder.build(&mono_ast);

        // Run typechecker on the parsed HIR
        let (warnings, type_errors, env) = pace_ty::check(&hir_arena, &hir_stmts, sources, module_path);

        // Lower HIR to MIR
        let mir_builder = pace_mir::MirBuilder::new(&hir_arena, &env);
        let mir_program = mir_builder.build(&hir_stmts);

        Ok((mono_ast, warnings, type_errors, env, hir_arena, mir_program))
    }


    pub fn check_file(
        &self,
        arena: &mut pace_ast::arena::AstArena,
        path: &str,
    ) -> Result<(Vec<pace_ast::arena::StmtId>, pace_mir::MirProgram)> {
        let mut visited = std::collections::HashSet::new();
        let path_buf = std::path::Path::new(path);
        let module_name = path_buf
            .canonicalize()
            .unwrap_or_else(|_| path_buf.to_path_buf())
            .to_string_lossy()
            .into_owned();
        let mut sources = std::collections::HashMap::new();
        let ast = self.load_file(
            arena,
            path_buf,
            &module_name,
            &mut visited,
            None,
            None,
            &mut sources,
        )?;

        let (mono_ast, warnings, type_errors, _, _, mir) =
            self.process_ast_pipeline(arena, ast, sources, &path_buf.display().to_string())?;

        for warning in warnings {
            eprintln!("{:?}", miette::Report::new(warning));
        }
        if !type_errors.is_empty() {
            return Err(Report::new(MultipleTypeErrors {
                errors: type_errors,
            }));
        }

        Ok((mono_ast, mir))
    }

    pub fn check_file_with_source(
        &self,
        arena: &mut pace_ast::arena::AstArena,
        path: &std::path::Path,
        src: &str,
    ) -> Result<(
        Vec<pace_ast::arena::StmtId>,
        Vec<pace_errors::SemanticWarning>,
        Vec<pace_ty::TypeError>,
        pace_ty::Environment,
        pace_hir::arena::HirArena,
    )> {
        let mut visited = std::collections::HashSet::new();
        let path_buf = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let module_name = path_buf.to_string_lossy().into_owned();
        let mut sources = std::collections::HashMap::new();
        let ast = self.load_file(
            arena,
            &path_buf,
            &module_name,
            &mut visited,
            Some(&path_buf),
            Some(src),
            &mut sources,
        )?;

        let (mono_ast, warnings, type_errors, env, hir_arena, _mir) =
            self.process_ast_pipeline(arena, ast, sources, &path_buf.display().to_string())?;

        for warning in &warnings {
            eprintln!("{:?}", miette::Report::new(warning.clone()));
        }

        Ok((mono_ast, warnings, type_errors, env, hir_arena))
    }

    pub fn run_file(&self, path: &str) -> Result<()> {
        let mut arena = pace_ast::arena::AstArena::new();
        let (ast, mir) = self.check_file(&mut arena, path)?;
        let mut compiler = pace_codegen_cranelift::JITCompiler::new(if self.session.options.release_mode {
            "speed_and_size".to_string()
        } else {
            "none".to_string()
        });

        if self.session.options.use_mir {
            compiler
                .compile_and_run_mir(&mir)
                .map_err(Report::new)?;
        } else {
            compiler
                .compile_and_run(&mut arena, &ast)
                .map_err(Report::new)?;
        }

        Ok(())
    }

    pub fn run_source(&self, src: &str) -> Result<()> {
        let mut arena = pace_ast::arena::AstArena::new();
        let (ast, mir) = self.check_source(&mut arena, src)?;
        let mut compiler = pace_codegen_cranelift::JITCompiler::new(if self.session.options.release_mode {
            "speed_and_size".to_string()
        } else {
            "none".to_string()
        });

        if self.session.options.use_mir {
            compiler
                .compile_and_run_mir(&mir)
                .map_err(Report::new)?;
        } else {
            compiler
                .compile_and_run(&mut arena, &ast)
                .map_err(Report::new)?;
        }

        Ok(())
    }

    pub fn build_file(&self, path: &str, output: &str) -> Result<()> {
        let mut arena = pace_ast::arena::AstArena::new();
        let (ast, _mir) = self.check_file(&mut arena, path)?;
        self.build_from_ast(&mut arena, &ast, output)
    }

    pub fn build_source(&self, src: &str, output: &str) -> Result<()> {
        let mut arena = pace_ast::arena::AstArena::new();
        let (ast, _mir) = self.check_source(&mut arena, src)?;
        self.build_from_ast(&mut arena, &ast, output)
    }

    fn build_from_ast(
        &self,
        arena: &mut pace_ast::arena::AstArena,
        ast: &[pace_ast::arena::StmtId],
        output: &str,
    ) -> Result<()> {
        let compiler = pace_codegen_cranelift::AotCompiler::new(if self.session.options.release_mode {
            "speed_and_size".to_string()
        } else {
            "none".to_string()
        });

        let obj_bytes = compiler
            .compile_to_object(arena, ast)
            .map_err(Report::new)?;

        let obj_path = format!("{}.o", output);
        std::fs::write(&obj_path, obj_bytes).into_diagnostic()?;

        let mut runtime_path = None;
        if let Ok(home) = std::env::var("PACE_HOME") {
            let base_dir = std::path::PathBuf::from(home);
            let debug_path = base_dir.join("target/debug/libpace_runtime.a");
            let release_path = base_dir.join("target/release/libpace_runtime.a");
            if debug_path.exists() {
                runtime_path = Some(debug_path);
            } else if release_path.exists() {
                runtime_path = Some(release_path);
            }
        } else if let Ok(exe_path) = std::env::current_exe() {
            // exe_path is something like target/debug/pace
            // We want to look in the same directory as the executable
            if let Some(parent) = exe_path.parent() {
                let runtime_a = parent.join("libpace_runtime.a");
                if runtime_a.exists() {
                    runtime_path = Some(runtime_a);
                }
            }
        }

        // Fallback to current directory if not found yet (for development)
        if runtime_path.is_none() {
            let current = std::env::current_dir().unwrap();
            let debug_path = current.join("target/debug/libpace_runtime.a");
            let release_path = current.join("target/release/libpace_runtime.a");

            // Try parent directory as well (if running from examples/)
            let parent_debug = current
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("target/debug/libpace_runtime.a");

            if debug_path.exists() {
                runtime_path = Some(debug_path);
            } else if release_path.exists() {
                runtime_path = Some(release_path);
            } else if parent_debug.exists() {
                runtime_path = Some(parent_debug);
            }
        }

        let mut cmd = std::process::Command::new("gcc");
        cmd.arg(&obj_path).arg("-o").arg(output);

        if let Some(rp) = runtime_path {
            cmd.arg(&rp);
        } else {
            println!("Warning: libpace_runtime.a not found in target/debug or target/release");
        }

        let status = cmd.status().into_diagnostic()?;

        if !status.success() {
            return Err(miette::miette!("Failed to link executable with gcc"));
        }

        let _ = std::fs::remove_file(obj_path);

        Ok(())
    }

    pub fn check_source(
        &self,
        arena: &mut pace_ast::arena::AstArena,
        src: &str,
    ) -> Result<(Vec<pace_ast::arena::StmtId>, pace_mir::MirProgram)> {
        let ast = match pace_parser::parse(arena, src, "source") {
            Ok((ast, _)) => {
                let mod_id = arena.alloc_stmt(Stmt::Module {
                    name: ustr::Ustr::from("__repl__"),
                    body: ast,
                }, pace_ast::Span::default());
                vec![mod_id]
            }
            Err(parse_errors) => {
                return Err(Report::new(pace_errors::MultipleSyntaxErrors {
                    errors: parse_errors,
                }));
            }
        };

        let mut sources = std::collections::HashMap::new();
        sources.insert(ustr::Ustr::from("source"), src.to_string());

        let (mono_ast, warnings, type_errors, _env, _hir_arena, mir) =
            self.process_ast_pipeline(arena, ast, sources, "source")?;

        for warning in warnings {
            eprintln!("{:?}", miette::Report::new(warning));
        }
        if !type_errors.is_empty() {
            return Err(Report::new(MultipleTypeErrors {
                errors: type_errors,
            }));
        }

        Ok((mono_ast, mir))
    }
}
pub mod escape;
