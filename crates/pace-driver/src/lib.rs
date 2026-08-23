use miette::{Result, IntoDiagnostic, Report};
use pace_ast::Stmt;
use pace_errors::ParseError;

pub struct CompilerSession;

impl CompilerSession {
    pub fn new() -> Self {
        Self
    }

    pub fn check_file(&self, path: &str) -> Result<Vec<Stmt>> {
        let src = std::fs::read_to_string(path)
            .into_diagnostic()?;
        
        self.check_source(&src)
    }

    pub fn run_file(&self, path: &str) -> Result<()> {
        let src = std::fs::read_to_string(path)
            .into_diagnostic()?;
        self.run_source(&src)
    }

    pub fn run_source(&self, src: &str) -> Result<()> {
        let ast = self.check_source(src)?;
        let mut compiler = pace_codegen::JITCompiler::new();
        
        compiler.compile_and_run(&ast).map_err(|e| {
            Report::new(e)
        })?;
        
        Ok(())
    }

    pub fn build_file(&self, path: &str, output: &str) -> Result<()> {
        let src = std::fs::read_to_string(path)
            .into_diagnostic()?;
        self.build_source(&src, output)
    }

    pub fn build_source(&self, src: &str, output: &str) -> Result<()> {
        let ast = self.check_source(src)?;
        let mut compiler = pace_codegen::AotCompiler::new();
        
        let obj_bytes = compiler.compile_to_object(&ast).map_err(|e| {
            Report::new(e)
        })?;
        
        let obj_path = format!("{}.o", output);
        std::fs::write(&obj_path, obj_bytes).into_diagnostic()?;
        
        // Use gcc to link it
        let status = std::process::Command::new("gcc")
            .arg(&obj_path)
            .arg("-o")
            .arg(output)
            .status()
            .into_diagnostic()?;
            
        if !status.success() {
            return Err(miette::miette!("Failed to link executable with gcc"));
        }
        
        // Clean up the object file
        let _ = std::fs::remove_file(obj_path);
        
        Ok(())
    }

    pub fn check_source(&self, src: &str) -> Result<Vec<Stmt>> {
        let ast = match pace_parser::parse(src) {
            Ok(ast) => ast,
            Err(err_msg) => {
                let err = ParseError {
                    message: err_msg,
                };
                return Err(Report::new(err));
            }
        };

        // Run typechecker on the parsed AST
        if let Err(type_err) = pace_ty::check(&ast) {
            return Err(Report::new(type_err));
        }

        Ok(ast)
    }
}
