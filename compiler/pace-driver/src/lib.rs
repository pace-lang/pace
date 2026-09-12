use std::fs;
use std::path::Path;
use std::process::Command;

use pace_codegen::CGenerator;
use pace_hir::LoweringContext;
use pace_lexer::Lexer;
use pace_mir::MirBuilder;
use pace_parser::Parser;
use pace_ty::TypeChecker;

use std::collections::HashSet;
use std::path::PathBuf;

fn parse_file_and_imports(
    file_path: &Path,
    visited: &mut HashSet<PathBuf>,
    modules: &mut std::collections::HashMap<String, pace_ast::Module>,
    lockfile: Option<&pace_pkg::resolve::PaceLock>,
    cache: &pace_pkg::cache::CacheManager,
    source_map: &mut pace_span::SourceMap,
) -> Result<(), String> {
    let canonical = file_path.canonicalize().unwrap_or_else(|_| file_path.to_path_buf());
    if !visited.insert(canonical.clone()) {
        return Ok(());
    }

    let source = fs::read_to_string(file_path).map_err(|e| format!("Failed to read {}: {}", file_path.display(), e))?;
    let file_id = source_map.add_file(file_path.display().to_string(), source.clone());
    let lexer = Lexer::new(&source, file_id);
    let mut parser = Parser::new(lexer);
    
    let ast = parser.parse_program().map_err(|diag| {
        let mut reporter = pace_errors::Reporter::new();
        reporter.report(diag);
        reporter.emit_all(source_map);
        format!("Compilation failed due to syntax errors in {}", file_path.display())
    })?;

    let mut module_decls = Vec::new();
    let module_name = file_path.file_stem().unwrap().to_string_lossy().to_string();

    for decl in ast {
        module_decls.push(decl.clone());

        if let pace_ast::Decl::Import { path, .. } = &decl {
            let first_ident = &path[0].name;
            
            // Try to resolve logically
            let mut import_path = file_path.parent().unwrap().to_path_buf();
            for (i, ident) in path.iter().enumerate() {
                if i == path.len() - 1 {
                    import_path.push(format!("{}.pace", ident.name));
                } else {
                    import_path.push(&ident.name);
                }
            }

            if !import_path.exists() {
                if let Some(lock) = lockfile {
                    let pkg_entry = lock.packages.get(first_ident)
                        .map(|pkg| (first_ident.clone(), pkg));

                    if let Some((actual_name, pkg)) = pkg_entry {
                        let mut pkg_path = cache.get_package_path(&actual_name, &pkg.version);
                        if let Some(source) = &pkg.source {
                            if source.starts_with("local+") {
                                let local_path = source.strip_prefix("local+").unwrap();
                                if let Some(manifest) = pace_pkg::find_manifest(file_path) {
                                    pkg_path = manifest.parent().unwrap().join(local_path);
                                }
                            }
                        }
                        let mut resolved_path = pkg_path.join("src");
                        for (i, ident) in path.iter().skip(1).enumerate() {
                            if i == path.len() - 2 {
                                resolved_path.push(format!("{}.pace", ident.name));
                            } else {
                                resolved_path.push(&ident.name);
                            }
                        }
                        if path.len() == 1 {
                            resolved_path.push("lib.pace");
                        }
                        import_path = resolved_path;
                    }
                }
            }
            
            if !import_path.exists() {
                return Err(format!("Could not resolve import {:?} (tried {})", path.iter().map(|i| i.name.as_str()).collect::<Vec<_>>().join("."), import_path.display()));
            }

            parse_file_and_imports(&import_path, visited, modules, lockfile, cache, source_map)?;
        }
    }
    
    modules.insert(module_name.clone(), pace_ast::Module {
        name: module_name,
        file_id,
        declarations: module_decls,
    });

    Ok(())
}

pub fn compile_file(
    file_path: &Path,
    output_dir: &Path,
    output_name: &str,
    run: bool,
    check_only: bool,
) -> Result<(), String> {
    if !file_path.exists() {
        return Err(format!("File not found: {}", file_path.display()));
    }

    let cache = pace_pkg::cache::CacheManager::new();
    let lockfile_path = pace_pkg::find_manifest(file_path).map(|m| m.parent().unwrap().join("pace.lock"));
    let lockfile = lockfile_path.and_then(|p| pace_pkg::resolve::DependencyResolver::read_lockfile(&p).ok());

    let mut visited = HashSet::new();
    let mut modules = std::collections::HashMap::new();
    let mut source_map = pace_span::SourceMap::new();

    parse_file_and_imports(file_path, &mut visited, &mut modules, lockfile.as_ref(), &cache, &mut source_map)?;

    let ast = pace_ast::Program {
        modules,
        span: pace_span::Span::DUMMY, // We could merge spans, but DUMMY is fine for the program root
    };

    // 2. Lower to HIR
    let mut lowerer = LoweringContext::new();
    let mut hir = lowerer.lower_program(ast)?;

    // 3. Typecheck
    let mut tc = TypeChecker::new();
    hir.resolve_traits(&mut tc.reporter);
    if let Err(e) = tc.check_program(&hir) {
        if !tc.reporter.has_errors() {
            let mut reporter = pace_errors::Reporter::new();
            reporter.report(pace_errors::Diagnostic::error(e));
            reporter.emit_all(&source_map);
        }
        tc.reporter.emit_all(&source_map);
        return Err("Compilation failed due to type errors.".to_string());
    }

    tc.reporter.emit_all(&source_map);
    if tc.reporter.has_errors() {
        return Err("Compilation failed due to type errors.".to_string());
    }

    if check_only {
        println!("Check finished successfully.");
        return Ok(());
    }

    // 4. Build MIR
    let mir = MirBuilder::build_program(&hir, &mut tc);

    // 5. Generate C Code
    let mut codegen = CGenerator::new();
    let c_code = codegen.generate(&mir);

    let c_file = "/tmp/pace_out.c";
    fs::write(c_file, c_code).map_err(|e| format!("Failed to write C file: {}", e))?;

    // 6. Compile with GCC
    fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create output directory: {}", e))?;
    let bin_file = output_dir.join(output_name);

    // Find runtime path
    let runtime_path = if Path::new("runtime/pace_runtime.c").exists() {
        "runtime/pace_runtime.c".to_string()
    } else if Path::new("../runtime/pace_runtime.c").exists() {
        "../runtime/pace_runtime.c".to_string()
    } else if Path::new("../../runtime/pace_runtime.c").exists() {
        "../../runtime/pace_runtime.c".to_string()
    } else {
        "runtime/pace_runtime.c".to_string()
    };

    let status = Command::new("gcc")
        .arg("-O2")
        .arg(format!(
            "-I{}",
            Path::new(&runtime_path).parent().unwrap().display()
        ))
        .arg(c_file)
        .arg(&runtime_path)
        .arg("-o")
        .arg(&bin_file)
        .status()
        .map_err(|e| format!("Failed to invoke gcc: {}", e))?;

    if !status.success() {
        return Err("GCC compilation failed".to_string());
    }

    if !run {
        let file_name = file_path.file_name().unwrap_or_default().to_string_lossy();
        let bin_name = bin_file.file_name().unwrap_or_default().to_string_lossy();
        println!("build {} into ./build/{}", file_name, bin_name);
    }

    if run {
        let run_status = Command::new(&bin_file)
            .status()
            .map_err(|e| format!("Failed to run executable: {}", e))?;
        if !run_status.success() {
            return Err("Execution failed".to_string());
        }
    }

    Ok(())
}
