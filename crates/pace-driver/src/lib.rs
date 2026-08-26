use miette::{Result, IntoDiagnostic, Report};
use pace_ast::Stmt;

use miette::Diagnostic;
use thiserror::Error;

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

pub struct CompilerSession;

impl Default for CompilerSession {
    fn default() -> Self {
        Self::new()
    }
}

impl CompilerSession {
    pub fn new() -> Self {
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
        Self
    }


    fn load_file(&self, path: &std::path::Path, module_name: &str, visited: &mut std::collections::HashSet<std::path::PathBuf>) -> Result<Vec<Stmt>> {
        let path_buf = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if visited.contains(&path_buf) {
            return Ok(Vec::new()); // Already loaded, prevent cycles
        }
        visited.insert(path_buf.clone());

        let src = std::fs::read_to_string(&path_buf)
            .into_diagnostic()?;
        
        let mut ast = match pace_parser::parse(&src, &path.display().to_string()) {
            Ok(ast) => ast,
            Err(parse_errors) => {
                return Err(Report::new(pace_errors::MultipleSyntaxErrors { errors: parse_errors }));
            }
        };

        // Auto-inject std:core if not the core library itself
        if path_buf.file_stem().unwrap_or_default() != "core" {
            ast.insert(0, Stmt::Import { path: "std:core".to_string(), alias: None, show: None, hide: None });
        }

        // Resolve imports recursively
        let mut final_ast = Vec::new();
        for stmt in &mut ast {
            if let Stmt::Import { path: import_path, .. } = stmt {
                let resolved_path;
                
                if import_path.starts_with("std:") {
                    let path_without_std = import_path.strip_prefix("std:").unwrap_or(import_path);
                    if let Ok(stdlib_path) = std::env::var("PACE_STDLIB") {
                        resolved_path = std::path::Path::new(&stdlib_path).join(format!("{}.pace", path_without_std));
                    } else if let Ok(home_path) = std::env::var("PACE_HOME") {
                        resolved_path = std::path::Path::new(&home_path).join("stdlib").join(format!("{}.pace", path_without_std));
                    } else {
                        // Fallback to compile-time repository root
                        let manifest_dir = env!("CARGO_MANIFEST_DIR");
                        let fallback_path = std::path::Path::new(manifest_dir).join("../../stdlib");
                        resolved_path = fallback_path.join(format!("{}.pace", path_without_std));
                        if !resolved_path.exists() {
                            return Err(miette::miette!("Package Error: Standard library not found at '{}'. Please set PACE_STDLIB or PACE_HOME.", resolved_path.display()));
                        }
                    }
                } else if import_path.starts_with("./") || import_path.starts_with("../") {
                    let parent_dir = path_buf.parent().unwrap_or(std::path::Path::new(""));
                    resolved_path = parent_dir.join(format!("{}.pace", import_path));
                } else {
                    // External package
                    let current_dir = std::env::current_dir().unwrap();
                    let lock_opt = pace_pkg::lockfile::PaceLock::load_from_dir(&current_dir).unwrap_or(None);
                        
                    if let Some(lock) = lock_opt {
                        if let Some(pkg) = lock.packages.get(import_path) {
                            if let Some(path) = &pkg.path {
                                let pkg_path = std::path::PathBuf::from(path);
                                // Default entry point is src/lib.pace as per design
                                resolved_path = pkg_path.join("src").join("lib.pace");
                            } else {
                                return Err(miette::miette!("Package Error: External package '{}' missing path in pace.lock.", import_path));
                            }
                        } else {
                            return Err(miette::miette!("Package Error: External package '{}' not found in pace.lock. Did you run 'pace fetch'?", import_path));
                        }
                    } else {
                        // Fallback for when there's no lockfile
                        let packages_dir = current_dir.join("packages");
                        resolved_path = packages_dir.join(format!("{}.pace", import_path));
                    }

                    if !resolved_path.exists() {
                        return Err(miette::miette!("Package Error: External package '{}' not found at expected path: {}", import_path, resolved_path.display()));
                    }
                }
                
                if resolved_path.exists() {
                    let mod_name = if import_path.starts_with("./") || import_path.starts_with("../") {
                        resolved_path.canonicalize().unwrap_or_else(|_| resolved_path.clone()).to_string_lossy().into_owned()
                    } else {
                        format!("pkg:{}", import_path)
                    };
                    
                    // Inject the exact resolved mod_name back into the AST so resolve.rs doesn't have to guess relative paths
                    *import_path = mod_name.clone();
                    
                    let mut imported_ast = self.load_file(&resolved_path, &mod_name, visited)?;
                    final_ast.append(&mut imported_ast);
                } else {
                    return Err(miette::miette!("Cannot find module '{}' at {:?}", import_path, resolved_path));
                }
            }
        }
        
        // Append current file's AST after its dependencies
        final_ast.push(Stmt::Module { name: module_name.to_string(), body: ast });

        Ok(final_ast)
    }

    pub fn check_file(&self, path: &str) -> Result<Vec<Stmt>> {
        let mut visited = std::collections::HashSet::new();
        let path_buf = std::path::Path::new(path);
        let module_name = path_buf.canonicalize().unwrap_or_else(|_| path_buf.to_path_buf()).to_string_lossy().into_owned();
        let ast = self.load_file(path_buf, &module_name, &mut visited)?;
        // Symbol Resolution and Name Mangling pass
        let resolved_ast = resolve::SymbolResolver::run(ast)?;

        // Flatten the AST before monomorphization and type checking!
        let flat_ast = Self::flatten_ast(&resolved_ast);
        let mono_ast = monomorphize::Monomorphizer::run(flat_ast).unwrap_or_else(|_| resolved_ast.clone());

        // Apply Dead Code Elimination (Tree Shaking)
        let mono_ast = shake::TreeShaker::run(mono_ast);
        
        let src = std::fs::read_to_string(path).into_diagnostic()?;
        
        // Run typechecker on the parsed AST
        match pace_ty::check(&mono_ast, &src, &path_buf.display().to_string()) {
            Ok(warnings) => {
                for mut warning in warnings {
                    let mut is_valid_span = true;
                    match &mut warning {
                        pace_errors::SemanticWarning::NamingConvention { src: s, span, .. } |
                        pace_errors::SemanticWarning::UnusedItem { src: s, span, .. } => {
                            if span.0 + span.1 <= src.len() {
                                *s = miette::NamedSource::new(path_buf.display().to_string(), src.clone());
                            } else {
                                is_valid_span = false;
                            }
                        }
                    }
                    if is_valid_span {
                        eprintln!("{:?}", miette::Report::new(warning));
                    }
                }
            }
            Err(type_errors) => {
                return Err(Report::new(MultipleTypeErrors { errors: type_errors }));
            }
        }
        
        Ok(mono_ast)
    }
    

    fn flatten_ast(ast: &[Stmt]) -> Vec<Stmt> {
        let mut flat = Vec::new();
        for stmt in ast {
            if let Stmt::Module { body, .. } = stmt {
                flat.append(&mut Self::flatten_ast(body));
            } else {
                flat.push(stmt.clone());
            }
        }
        flat
    }


    pub fn run_file(&self, path: &str, release: bool) -> Result<()> {
        let ast = self.check_file(path)?;
        let mut compiler = pace_codegen::JITCompiler::new(if release { "speed".to_string() } else { "none".to_string() });
        
        compiler.compile_and_run(&ast).map_err(|e| {
            Report::new(e)
        })?;
        
        Ok(())
    }

    pub fn run_source(&self, src: &str, release: bool) -> Result<()> {
        let ast = self.check_source(src)?;
        let flat_ast = Self::flatten_ast(&ast);
        let mut compiler = pace_codegen::JITCompiler::new(if release { "speed".to_string() } else { "none".to_string() });
        
        compiler.compile_and_run(&flat_ast).map_err(|e| {
            Report::new(e)
        })?;
        
        Ok(())
    }

    pub fn build_file(&self, path: &str, output: &str, release: bool) -> Result<()> {
        let ast = self.check_file(path)?;
        self.build_from_ast(&ast, output, release)
    }

    pub fn build_source(&self, src: &str, output: &str, release: bool) -> Result<()> {
        let ast = self.check_source(src)?;
        self.build_from_ast(&ast, output, release)
    }
    
    fn build_from_ast(&self, ast: &[Stmt], output: &str, release: bool) -> Result<()> {
        let compiler = pace_codegen::AotCompiler::new(if release { "speed".to_string() } else { "none".to_string() });
        
        let flat_ast = Self::flatten_ast(ast);
        let obj_bytes = compiler.compile_to_object(&flat_ast).map_err(|e| {
            Report::new(e)
        })?;
        
        let obj_path = format!("{}.o", output);
        std::fs::write(&obj_path, obj_bytes).into_diagnostic()?;
        
        let mut runtime_path = None;
        if let Ok(home) = std::env::var("PACE_HOME") {
            let base_dir = std::path::PathBuf::from(home);
            let debug_path = base_dir.join("target/debug/libpace_runtime.a");
            let release_path = base_dir.join("target/release/libpace_runtime.a");
            if debug_path.exists() { runtime_path = Some(debug_path); }
            else if release_path.exists() { runtime_path = Some(release_path); }
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
            let parent_debug = current.parent().unwrap().parent().unwrap().join("target/debug/libpace_runtime.a");
            
            if debug_path.exists() { runtime_path = Some(debug_path); }
            else if release_path.exists() { runtime_path = Some(release_path); }
            else if parent_debug.exists() { runtime_path = Some(parent_debug); }
        }
            
        let mut cmd = std::process::Command::new("gcc");
        cmd.arg(&obj_path)
           .arg("-o")
           .arg(output);
           
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

    pub fn check_source(&self, src: &str) -> Result<Vec<Stmt>> {
        let ast = match pace_parser::parse(src, "source") {
            Ok(ast) => vec![Stmt::Module { name: "source".to_string(), body: ast }],
            Err(parse_errors) => {
                return Err(Report::new(pace_errors::MultipleSyntaxErrors { errors: parse_errors }));
            }
        };

        // Symbol Resolution and Name Mangling pass
        let resolved_ast = resolve::SymbolResolver::run(ast)?;

        // Flatten the AST before monomorphization and type checking!
        let flat_ast = Self::flatten_ast(&resolved_ast);
        let mono_ast = monomorphize::Monomorphizer::run(flat_ast).unwrap_or_else(|_| resolved_ast.clone());

        // Apply Dead Code Elimination (Tree Shaking)
        let mono_ast = shake::TreeShaker::run(mono_ast);

        // Run typechecker on the parsed AST
        match pace_ty::check(&mono_ast, src, "source") {
            Ok(warnings) => {
                for mut warning in warnings {
                    let mut is_valid_span = true;
                    match &mut warning {
                        pace_errors::SemanticWarning::NamingConvention { src: s, span, .. } |
                        pace_errors::SemanticWarning::UnusedItem { src: s, span, .. } => {
                            if span.0 + span.1 <= src.len() {
                                *s = miette::NamedSource::new("source", src.to_string());
                            } else {
                                is_valid_span = false;
                            }
                        }
                    }
                    if is_valid_span {
                        eprintln!("{:?}", miette::Report::new(warning));
                    }
                }
            }
            Err(type_errors) => {
                return Err(Report::new(MultipleTypeErrors { errors: type_errors }));
            }
        }

        Ok(mono_ast)
    }
}
