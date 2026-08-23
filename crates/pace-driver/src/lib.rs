use miette::{Result, IntoDiagnostic, Report};
use pace_ast::Stmt;
use pace_errors::ParseError;
use miette::Diagnostic;
use thiserror::Error;

#[derive(Error, Diagnostic, Debug)]
#[error("Found multiple type errors")]
#[diagnostic(code(pace::multiple_type_errors))]
pub struct MultipleTypeErrors {
    #[related]
    pub errors: Vec<pace_ty::TypeError>,
}

pub struct CompilerSession;

impl CompilerSession {
    pub fn new() -> Self {
        // Ensure pace-runtime is linked in the driver for JIT
        let _ = pace_runtime::__pace_print_int as *const () as usize;
        let _ = pace_runtime::__pace_print_float as *const () as usize;
        let _ = pace_runtime::__pace_print_string as *const () as usize;
        let _ = pace_runtime::__pace_malloc as *const () as usize;
        Self
    }


    fn load_file(&self, path: &std::path::Path, visited: &mut std::collections::HashSet<std::path::PathBuf>) -> Result<Vec<Stmt>> {
        let path_buf = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        if visited.contains(&path_buf) {
            return Ok(Vec::new()); // Already loaded, prevent cycles
        }
        visited.insert(path_buf.clone());

        let src = std::fs::read_to_string(&path_buf)
            .into_diagnostic()?;
        
        let ast = match pace_parser::parse(&src) {
            Ok(ast) => ast,
            Err(errors) => {
                let parse_errors = errors.into_iter().map(|(msg, span)| {
                    ParseError {
                        message: msg,
                        src: miette::NamedSource::new(path.display().to_string(), src.clone()),
                        span,
                    }
                }).collect();
                return Err(Report::new(pace_errors::MultipleSyntaxErrors { errors: parse_errors }));
            }
        };

        // Resolve imports recursively
        let mut final_ast = Vec::new();
        for stmt in &ast {
            if let Stmt::Import { path: import_path, .. } = stmt {
                let resolved_path;
                
                if import_path.starts_with("./") || import_path.starts_with("../") {
                    let parent_dir = path_buf.parent().unwrap_or(std::path::Path::new(""));
                    resolved_path = parent_dir.join(format!("{}.pace", import_path));
                } else {
                    if let Ok(stdlib_path) = std::env::var("PACE_STDLIB") {
                        let path_without_std = import_path.strip_prefix("std/").unwrap_or(import_path);
                        resolved_path = std::path::Path::new(&stdlib_path).join(format!("{}.pace", path_without_std));
                    } else if let Ok(home_path) = std::env::var("PACE_HOME") {
                        let path_without_std = import_path.strip_prefix("std/").unwrap_or(import_path);
                        resolved_path = std::path::Path::new(&home_path).join("stdlib").join(format!("{}.pace", path_without_std));
                    } else {
                        return Err(miette::miette!("Package Error: Standard library not found. Please set PACE_STDLIB or PACE_HOME."));
                    }
                }
                
                if resolved_path.exists() {
                    let mut imported_ast = self.load_file(&resolved_path, visited)?;
                    final_ast.append(&mut imported_ast);
                } else {
                    return Err(miette::miette!("Cannot find module '{}' at {:?}", import_path, resolved_path));
                }
            }
        }
        
        // Append current file's AST after its dependencies
        let module_name = path_buf.file_stem().unwrap_or_default().to_string_lossy().into_owned();
        final_ast.push(Stmt::Module { name: module_name, body: ast });

        Ok(final_ast)
    }

    pub fn check_file(&self, path: &str) -> Result<Vec<Stmt>> {
        let mut visited = std::collections::HashSet::new();
        let path_buf = std::path::Path::new(path);
        let ast = self.load_file(path_buf, &mut visited)?;
        let src = std::fs::read_to_string(path).into_diagnostic()?;
        
        // Run typechecker on the parsed AST
        match pace_ty::check(&ast) {
            Ok(warnings) => {
                for mut warning in warnings {
                    match &mut warning {
                        pace_errors::SemanticWarning::NamingConvention { src: s, .. } |
                        pace_errors::SemanticWarning::UnusedItem { src: s, .. } => {
                            *s = miette::NamedSource::new(path_buf.display().to_string(), src.clone());
                        }
                    }
                    eprintln!("{:?}", miette::Report::new(warning));
                }
            }
            Err(type_errors) => {
                return Err(Report::new(MultipleTypeErrors { errors: type_errors }));
            }
        }
        
        Ok(ast)
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

    pub fn run_file(&self, path: &str) -> Result<()> {
        let ast = self.check_file(path)?;
        let flat_ast = Self::flatten_ast(&ast);
        let mut compiler = pace_codegen::JITCompiler::new();
        
        compiler.compile_and_run(&flat_ast).map_err(|e| {
            Report::new(e)
        })?;
        
        Ok(())
    }

    pub fn run_source(&self, src: &str) -> Result<()> {
        let ast = self.check_source(src)?;
        let flat_ast = Self::flatten_ast(&ast);
        let mut compiler = pace_codegen::JITCompiler::new();
        
        compiler.compile_and_run(&flat_ast).map_err(|e| {
            Report::new(e)
        })?;
        
        Ok(())
    }

    pub fn build_file(&self, path: &str, output: &str) -> Result<()> {
        let ast = self.check_file(path)?;
        self.build_from_ast(&ast, output)
    }

    pub fn build_source(&self, src: &str, output: &str) -> Result<()> {
        let ast = self.check_source(src)?;
        self.build_from_ast(&ast, output)
    }
    
    fn build_from_ast(&self, ast: &[Stmt], output: &str) -> Result<()> {
        let compiler = pace_codegen::AotCompiler::new();
        
        let flat_ast = Self::flatten_ast(ast);
        let obj_bytes = compiler.compile_to_object(&flat_ast).map_err(|e| {
            Report::new(e)
        })?;
        
        let obj_path = format!("{}.o", output);
        std::fs::write(&obj_path, obj_bytes).into_diagnostic()?;
        
        let runtime_path = if let Ok(home) = std::env::var("PACE_HOME") {
            std::path::PathBuf::from(home).join("target/debug/libpace_runtime.a")
        } else {
            std::env::current_dir()
                .unwrap()
                .join("target/debug/libpace_runtime.a")
        };
            
        let mut cmd = std::process::Command::new("gcc");
        cmd.arg(&obj_path)
           .arg("-o")
           .arg(output);
           
        if runtime_path.exists() {
            cmd.arg(&runtime_path);
        } else {
            println!("Warning: libpace_runtime.a not found at {:?}", runtime_path);
        }
           
        let status = cmd.status().into_diagnostic()?;
            
        if !status.success() {
            return Err(miette::miette!("Failed to link executable with gcc"));
        }
        
        let _ = std::fs::remove_file(obj_path);
        
        Ok(())
    }

    pub fn check_source(&self, src: &str) -> Result<Vec<Stmt>> {
        let ast = match pace_parser::parse(src) {
            Ok(ast) => ast,
            Err(errors) => {
                let parse_errors = errors.into_iter().map(|(msg, span)| {
                    ParseError {
                        message: msg,
                        src: miette::NamedSource::new("source", src.to_string()),
                        span,
                    }
                }).collect();
                return Err(Report::new(pace_errors::MultipleSyntaxErrors { errors: parse_errors }));
            }
        };

        // Run typechecker on the parsed AST
        match pace_ty::check(&ast) {
            Ok(warnings) => {
                for mut warning in warnings {
                    match &mut warning {
                        pace_errors::SemanticWarning::NamingConvention { src: s, .. } |
                        pace_errors::SemanticWarning::UnusedItem { src: s, .. } => {
                            *s = miette::NamedSource::new("source", src.to_string());
                        }
                    }
                    eprintln!("{:?}", miette::Report::new(warning));
                }
            }
            Err(type_errors) => {
                return Err(Report::new(MultipleTypeErrors { errors: type_errors }));
            }
        }

        Ok(ast)
    }
}
