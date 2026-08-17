use std::path::Path;
use std::process::exit;

use diagnostics::{Severity, print_diagnostics};
use lowering::ProgramBuilder;
use resolver::Resolver;
use typechecker::TypeChecker;

use crate::utils::errors::print_global_error;

pub fn compile_to_mir(file: &Path) -> mir::Program {
    if file.extension().and_then(|e| e.to_str()) != Some("pace") {
        print_global_error("File must have a .pace extension");
        exit(1);
    }

    // Find pace.toml root
    let mut current_dir = file.canonicalize().unwrap_or(file.to_path_buf());
    let mut package_root = None;
    while let Some(parent) = current_dir.parent() {
        if parent.join("pace.toml").exists() {
            package_root = Some(parent.to_path_buf());
            break;
        }
        current_dir = parent.to_path_buf();
    }

    let mut package_manager = package::manager::PackageManager::new();
    let package_graph = if let Some(root) = package_root {
        package_manager.load_root(&root);
        if !package_manager.errors.is_empty() {
            for diag in &package_manager.errors {
                print_global_error(&format!("Package Error: {}", diag.message));
            }
            exit(1);
        }
        Some(package_manager.into_graph())
    } else {
        package_manager.load_dummy_root();
        if !package_manager.errors.is_empty() {
            for diag in &package_manager.errors {
                print_global_error(&format!("Package Error: {}", diag.message));
            }
            exit(1);
        }
        Some(package_manager.into_graph())
    };

    let session = session::CompilerSession::new();
    let mut loader = module::loader::ModuleLoader::new(package_graph.as_ref(), &session);
    loader.load_root(file);

    let loader_errors = std::mem::take(&mut loader.errors);
    let (graph, source_map) = loader.into_graph();

    let mut has_errors = false;

    if !loader_errors.is_empty() {
        print_diagnostics(&loader_errors, &source_map);
        if loader_errors.iter().any(|d| d.severity == Severity::Error) {
            has_errors = true;
        }
    }

    if has_errors {
        exit(1);
    }

    // Lint Pass
    let mut linter = linter::Linter::new(&session);
    for module in graph.modules() {
        linter.lint(&module.ast);
    }
    let linter_warnings = linter.into_diagnostics();
    if !linter_warnings.is_empty() {
        print_diagnostics(&linter_warnings, &source_map);
    }

    // 3. Name Resolution
    let mut resolver = Resolver::new(&session);
    resolver.resolve_graph(&graph);
    if !resolver.errors.is_empty() {
        print_diagnostics(&resolver.errors, &source_map);
        if resolver
            .errors
            .iter()
            .any(|d| d.severity == Severity::Error)
        {
            has_errors = true;
        }
    }

    if has_errors {
        exit(1);
    }

    // 4. Type Checking
    let mut generic_registry = generics::GenericDefinitionRegistry::new();
    let mut spec_registry = generics::SpecializationRegistry::new();
    let mut typechecker = TypeChecker::new(&session, &mut generic_registry, &mut spec_registry);
    let typed_ast = typechecker.check_graph(&graph);
    if !typechecker.errors.is_empty() {
        print_diagnostics(&typechecker.errors, &source_map);
        if typechecker
            .errors
            .iter()
            .any(|d| d.severity == Severity::Error)
        {
            has_errors = true;
        }
    }

    if has_errors {
        exit(1);
    }

    // 5. Lowering (AST -> MIR)
    let builder = lowering::ProgramBuilder::new(&session);
    let mut mir_program = builder.build(&typed_ast);

    // 6. ARC Pass
    let arc_pass = arc::arc_pass::ArcPass::new();
    arc_pass.run(&mut mir_program);

    mir_program
}
