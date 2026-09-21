use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const RUNTIME_LIB: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/libpace_rt.a"));
const RUNTIME_HEADER: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/pace_runtime.h"));

use pace_codegen::CGenerator;
use pace_hir::LoweringContext;
use pace_lexer::Lexer;
use pace_mir::MirBuilder;
use pace_parser::Parser;
use pace_ty::TypeChecker;

use std::collections::HashSet;

#[derive(Debug, Default)]
struct DependencyGraph {
    edges: std::collections::HashMap<String, Vec<String>>,
}

impl DependencyGraph {
    fn add_edge(&mut self, from: String, to: String) {
        self.edges.entry(from).or_default().push(to);
    }

    fn topological_sort(&self, modules: &[String]) -> Result<Vec<String>, String> {
        let mut order = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut temp_mark = std::collections::HashSet::new();

        fn visit(
            node: &str,
            edges: &std::collections::HashMap<String, Vec<String>>,
            visited: &mut std::collections::HashSet<String>,
            temp_mark: &mut std::collections::HashSet<String>,
            order: &mut Vec<String>,
        ) -> Result<(), String> {
            if temp_mark.contains(node) {
                return Err(format!(
                    "Circular dependency detected involving module '{}'",
                    node
                ));
            }
            if !visited.contains(node) {
                temp_mark.insert(node.to_string());
                if let Some(deps) = edges.get(node) {
                    for dep in deps {
                        visit(dep, edges, visited, temp_mark, order)?;
                    }
                }
                temp_mark.remove(node);
                visited.insert(node.to_string());
                order.push(node.to_string());
            }
            Ok(())
        }

        // Sort the input modules to ensure deterministic traversal when there are multiple disconnected components
        let mut sorted_modules = modules.to_vec();
        sorted_modules.sort();

        for node in &sorted_modules {
            if !visited.contains(node) {
                visit(node, &self.edges, &mut visited, &mut temp_mark, &mut order)?;
            }
        }
        Ok(order)
    }
}

#[allow(clippy::too_many_arguments)]
fn parse_file_and_imports(
    file_path: &Path,
    module_name_opt: Option<String>,
    visited: &mut HashSet<PathBuf>,
    modules: &mut std::collections::HashMap<pace_span::Symbol, pace_ast::Module>,
    deps: &mut DependencyGraph,
    dependencies: &std::collections::HashMap<String, PathBuf>,
    source_map: &mut pace_span::SourceMap,
    overrides: &std::collections::HashMap<PathBuf, String>,
    all_diags: &mut Vec<pace_errors::Diagnostic>,
) -> Result<(), String> {
    let canonical = file_path
        .canonicalize()
        .unwrap_or_else(|_| file_path.to_path_buf());
    if !visited.insert(canonical.clone()) {
        return Ok(());
    }

    let source = if let Some(text) = overrides.get(file_path) {
        text.clone()
    } else {
        fs::read_to_string(file_path)
            .map_err(|e| format!("Failed to read {}: {}", file_path.display(), e))?
    };
    let file_id = source_map.add_file(file_path.display().to_string(), source.clone());
    let lexer = Lexer::new(&source, file_id);
    let mut parser = Parser::new(lexer);

    let (ast, diags, comments) = parser.parse_program();
    all_diags.extend(diags);

    let mut module_decls = Vec::new();
    let module_name = module_name_opt.unwrap_or_else(|| {
        file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("unknown")
            .to_string()
    });

    for decl in ast {
        module_decls.push(decl.clone());

        if let pace_ast::Decl::Import { path, .. } = &decl {
            let first_ident = &path[0].name;

            // Try to resolve logically
            let mut import_path = file_path
                .parent()
                .ok_or_else(|| format!("Invalid file path: {:?}", file_path))?
                .to_path_buf();
            for (i, ident) in path.iter().enumerate() {
                if i == path.len() - 1 {
                    import_path.push(format!("{}.pace", ident.name));
                } else {
                    import_path.push(ident.name);
                }
            }

            if !import_path.exists()
                && let Some(pkg_path) = dependencies.get(&first_ident.to_string())
            {
                let mut resolved_path = pkg_path.join("src");
                for (i, ident) in path.iter().skip(1).enumerate() {
                    if i == path.len() - 2 {
                        resolved_path.push(format!("{}.pace", ident.name));
                    } else {
                        resolved_path.push(ident.name);
                    }
                }
                if path.len() == 1 {
                    resolved_path.push("lib.pace");
                }
                import_path = resolved_path;
            }

            if !import_path.exists() {
                return Err(format!(
                    "Could not resolve import {:?} (tried {})",
                    path.iter()
                        .map(|i| i.name.as_str())
                        .collect::<Vec<_>>()
                        .join("."),
                    import_path.display()
                ));
            }

            let imported_module_name = path
                .iter()
                .map(|i| i.name.as_str())
                .collect::<Vec<_>>()
                .join("_");
            deps.add_edge(module_name.clone(), imported_module_name.clone());
            parse_file_and_imports(
                &import_path,
                Some(imported_module_name),
                visited,
                modules,
                deps,
                dependencies,
                source_map,
                overrides,
                all_diags,
            )?;
        }
    }

    modules.insert(
        pace_span::intern(&module_name),
        pace_ast::Module {
            name: pace_span::intern(&module_name),
            file_id,
            declarations: module_decls,
            comments,
        },
    );

    Ok(())
}

pub fn analyze_workspace(
    file_path: &Path,
    overrides: &std::collections::HashMap<PathBuf, String>,
    dependencies: &std::collections::HashMap<String, PathBuf>,
) -> Result<
    (
        pace_ast::Program,
        pace_span::SourceMap,
        Vec<pace_errors::Diagnostic>,
    ),
    String,
> {
    if !file_path.exists() && !overrides.contains_key(file_path) {
        return Err(format!("File not found: {}", file_path.display()));
    }

    let mut visited = HashSet::new();
    let mut modules = std::collections::HashMap::<pace_span::Symbol, pace_ast::Module>::new();
    let mut source_map = pace_span::SourceMap::new();
    let mut deps = DependencyGraph::default();
    let mut all_diags = Vec::new();

    parse_file_and_imports(
        file_path,
        None,
        &mut visited,
        &mut modules,
        &mut deps,
        dependencies,
        &mut source_map,
        overrides,
        &mut all_diags,
    )?;

    let module_names: Vec<pace_span::Symbol> = modules.keys().cloned().collect();
    let module_names_str: Vec<String> = module_names.iter().map(|n| n.to_string()).collect();
    let module_order_str = deps.topological_sort(&module_names_str)?;
    let module_order: Vec<pace_span::Symbol> = module_order_str
        .into_iter()
        .map(|s| pace_span::intern(&s))
        .collect();

    let ast = pace_ast::Program {
        modules,
        module_order,
        span: pace_span::Span::DUMMY,
    };

    Ok((ast, source_map, all_diags))
}

pub struct CompileOptions<'a> {
    pub file_path: &'a Path,
    pub output_dir: &'a Path,
    pub output_name: &'a str,
    pub run: bool,
    pub check_only: bool,
    pub release: bool,
    pub dependencies: &'a std::collections::HashMap<String, PathBuf>,
    pub is_lib: bool,
}

pub fn compile_file(opts: CompileOptions) -> Result<(), String> {
    let empty_overrides = std::collections::HashMap::new();
    let (ast, source_map, diags) =
        analyze_workspace(opts.file_path, &empty_overrides, opts.dependencies)?;

    if !diags.is_empty() {
        let mut reporter = pace_errors::Reporter::new();
        for diag in diags {
            reporter.report(diag);
        }
        reporter.emit_all(&source_map);
        return Err("Compilation failed due to syntax errors".to_string());
    }

    // 2. Lower to HIR
    let mut lowerer = LoweringContext::new();
    let mut hir = lowerer.lower_program(ast)?;

    // 3. Typecheck
    let mut tc = TypeChecker::new();
    tc.is_lib = opts.is_lib;
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

    if opts.check_only {
        println!("Check finished successfully.");
        return Ok(());
    }

    if opts.is_lib {
        println!("Library built successfully.");
        return Ok(());
    }

    // 4. Build MIR
    let mut mir = MirBuilder::build_program(&hir, &mut tc);

    // 4.5 Optimize MIR
    pace_mir::opt::ConstantFolder::new().optimize_program(&mut mir);

    // 5. Generate C Code
    let mut codegen = CGenerator::new();
    let c_code = codegen.generate(&mir);

    fs::create_dir_all(opts.output_dir)
        .map_err(|e| format!("Failed to create output dir: {}", e))?;
    let c_file = opts.output_dir.join(format!("{}.c", opts.output_name));
    fs::write(&c_file, c_code).map_err(|e| format!("Failed to write C file: {}", e))?;

    let bin_file = opts.output_dir.join(opts.output_name);

    // Find runtime paths
    let tmp_dir = std::env::temp_dir().join(format!("pace-rt-{}", std::process::id()));
    fs::create_dir_all(&tmp_dir).map_err(|e| format!("Failed to create tmp dir: {}", e))?;
    let tmp_lib = tmp_dir.join("libpace_rt.a");
    let tmp_header = tmp_dir.join("pace_runtime.h");
    fs::write(&tmp_lib, RUNTIME_LIB).map_err(|e| format!("Failed to write libpace_rt.a: {}", e))?;
    fs::write(&tmp_header, RUNTIME_HEADER)
        .map_err(|e| format!("Failed to write pace_runtime.h: {}", e))?;

    let runtime_dir = tmp_dir.clone();
    let lib_dir = tmp_dir.clone();

    let mut gcc_cmd = Command::new("gcc");
    if opts.release {
        gcc_cmd.arg("-O3");
    }

    let status = gcc_cmd
        .arg(format!("-I{}", runtime_dir.display()))
        .arg(c_file)
        .arg("-L")
        .arg(lib_dir)
        .arg("-lpace_rt")
        .arg("-o")
        .arg(&bin_file)
        .status()
        .map_err(|e| format!("Failed to invoke gcc: {}", e))?;

    if !status.success() {
        return Err("GCC compilation failed".to_string());
    }

    if !opts.run {
        let file_name = opts.file_path.file_name().unwrap_or_default().to_string_lossy();
        let bin_name = bin_file.file_name().unwrap_or_default().to_string_lossy();
        println!("build {} into ./build/{}", file_name, bin_name);
    }

    if opts.run {
        let run_status = Command::new(&bin_file)
            .status()
            .map_err(|e| format!("Failed to run executable: {}", e))?;
        if !run_status.success() {
            return Err("Execution failed".to_string());
        }
    }

    Ok(())
}

pub fn format_file(file_path: &Path, write: bool) -> Result<bool, String> {
    if !file_path.exists() {
        return Err(format!("File not found: {}", file_path.display()));
    }

    let source = fs::read_to_string(file_path)
        .map_err(|e| format!("Failed to read {}: {}", file_path.display(), e))?;

    let mut source_map = pace_span::SourceMap::new();
    let file_id = source_map.add_file(file_path.display().to_string(), source.clone());

    let lexer = Lexer::new(&source, file_id);
    let mut parser = Parser::new(lexer);

    let (ast, diags, comments) = parser.parse_program();
    if !diags.is_empty() {
        let mut reporter = pace_errors::Reporter::new();
        for diag in diags {
            reporter.report(diag);
        }
        reporter.emit_all(&source_map);
        return Err(format!("Syntax error in {}", file_path.display()));
    }

    let module = pace_ast::Module {
        name: pace_span::intern(
            file_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown"),
        ),
        file_id,
        declarations: ast,
        comments,
    };

    let formatted = pace_fmt::format_module(&module);

    let changed = source != formatted;

    if write {
        if changed {
            fs::write(file_path, formatted)
                .map_err(|e| format!("Failed to write {}: {}", file_path.display(), e))?;
        }
    } else {
        println!("{}", formatted);
    }

    Ok(changed)
}
