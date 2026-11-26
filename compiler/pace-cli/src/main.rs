use clap::{Parser as ClapParser, Subcommand};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[derive(ClapParser)]
#[command(
    name = "pace",
    version = env!("CARGO_PKG_VERSION"),
    about = "Pace Toolchain and Package Manager"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new Pace package
    New {
        /// Name of the package
        name: String,
        /// Use a library template
        #[arg(long)]
        lib: bool,
    },
    /// Build the current package (defaults to release)
    Build {
        /// Optional path to a specific .pace file to build in script mode
        file: Option<String>,
        /// Build in debug mode (unoptimized)
        #[arg(long)]
        debug: bool,
    },
    /// Typecheck the current package
    Check {
        /// Optional path to a specific .pace file to check in script mode
        file: Option<String>,
    },
    /// Build and run the current package (defaults to debug)
    Run {
        /// Optional path to a specific .pace file to run in script mode
        file: Option<String>,
        /// Run with release optimizations enabled
        #[arg(long)]
        release: bool,
    },
    /// Run test files (.test.pace) in the package
    Test {
        /// Optional path to a specific test file or filter pattern
        filter: Option<String>,
        /// Stop immediately on the first test failure
        #[arg(long)]
        fail_fast: bool,
    },
    /// Print the toolchain version
    Version,
    /// Upgrade the toolchain to the latest version
    Upgrade,
    /// Format a Pace file or project
    Fmt {
        /// The file to format (optional, formats whole project if omitted)
        file: Option<String>,
        /// Print the output to stdout instead of writing to files
        #[arg(long)]
        stdout: bool,
    },
    /// Start the Language Server (Internal use by editors)
    #[command(hide = true)]
    Lsp,
    /// Add a dependency to the project
    Add {
        /// Name of the package
        name: String,
        /// Optional version constraint (e.g. "^1.0.0")
        version: Option<String>,
        /// Add as a dev dependency
        #[arg(long, short)]
        dev: bool,
    },
    /// Remove a dependency from the project
    Remove {
        /// Name of the package to remove
        name: String,
    },
    /// Update dependencies
    Update {
        /// Update to the absolute latest versions, modifying pace.toml
        #[arg(long)]
        latest: bool,
    },
    /// Install dependencies from pace.toml
    Install,
    /// List outdated dependencies
    Outdated,
    /// Publish the package
    Publish,
    /// Login to the registry
    Login {
        /// Authentication token
        token: String,
    },
    /// Clean the build artifacts and cache
    Clean {
        /// Also clean the global package cache
        #[arg(long)]
        cache: bool,
    },
}

fn get_project_info() -> Result<(PathBuf, String), String> {
    let current_dir = env::current_dir().map_err(|_| "Failed to get current directory")?;
    let manifest_path = pace_pkg::find_manifest(&current_dir)
        .ok_or("No pace.toml found in this directory or any parent directory")?;

    let toml = pace_pkg::parse_manifest(&manifest_path)?;
    let root = manifest_path
        .parent()
        .ok_or("Invalid manifest path")?
        .to_path_buf();

    Ok((root, toml.package.name))
}

async fn execute_build_or_run(
    file: Option<String>,
    run: bool,
    check: bool,
    release: bool,
) -> Result<(), String> {
    if let Some(f) = file {
        let p = PathBuf::from(&f);
        let name = p
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or("Invalid file name")?
            .to_string();
        let empty_deps = std::collections::HashMap::new();
        pace_driver::compile_file(pace_driver::CompileOptions {
            file_path: &p,
            output_dir: Path::new("."),
            output_name: &name,
            run,
            check_only: check,
            release,
            dependencies: &empty_deps,
            is_lib: false,
            capture_output: false,
        })
    } else {
        let (root, project_name) = get_project_info()?;

        let manifest_path = root.join("pace.toml");
        let toml = pace_pkg::parse_manifest(&manifest_path)?;
        let env = &toml.environment;
        let req = semver::VersionReq::parse(&env.sdk)
            .map_err(|_| format!("Invalid SDK version requirement: {}", env.sdk))?;
        let compiler_version = semver::Version::parse(env!("CARGO_PKG_VERSION"))
            .unwrap_or_else(|_| semver::Version::new(0, 1, 0));

        if !req.matches(&compiler_version) {
            return Err(format!(
                "The current project requires Pace SDK version {}, but you are using {}",
                env.sdk, compiler_version
            ));
        }
        let mut resolver = pace_pkg::resolve::DependencyResolver::new();
        let lockfile_path = root.join("pace.lock");
        if lockfile_path.exists()
            && let Ok(lock) = pace_pkg::resolve::DependencyResolver::read_lockfile(&lockfile_path)
        {
            resolver.load_lock(lock);
        }
        let lock = resolver.resolve(&toml).await?;
        resolver.write_lockfile(&lockfile_path, &lock)?;

        let main_file = root.join("src/main.pace");
        let lib_file = root.join("src/lib.pace");
        let is_lib = !main_file.exists() && lib_file.exists();

        let target_file = if main_file.exists() {
            main_file
        } else if lib_file.exists() {
            lib_file
        } else {
            return Err("Neither src/main.pace nor src/lib.pace found".to_string());
        };

        let cache = pace_pkg::cache::CacheManager::new();
        let mut dependencies = std::collections::HashMap::new();
        for (pkg_name, pkg_info) in &lock.packages {
            let mut pkg_path = cache.get_package_path(pkg_name, &pkg_info.version);
            if let Some(source) = &pkg_info.source {
                if source.starts_with("local+") {
                    let local_path = source
                        .strip_prefix("local+")
                        .ok_or("Invalid local+ prefix")?;
                    pkg_path = root.join(local_path);
                } else if !pkg_path.exists() {
                    // Automatically download if missing from cache
                    if let Some(checksum) = &pkg_info.checksum {
                        println!("Downloading {} v{}...", pkg_name, pkg_info.version);
                        if let Err(e) = cache
                            .download_and_extract(pkg_name, &pkg_info.version, checksum)
                            .await
                        {
                            eprintln!(
                                "Warning: Failed to download {}. You may be offline, and this package is not in your local cache. Error: {}",
                                pkg_name, e
                            );
                        }
                    }
                }
            }
            dependencies.insert(pkg_name.clone(), pkg_path);
        }

        let build_dir = root.join("build");
        pace_driver::compile_file(pace_driver::CompileOptions {
            file_path: &target_file,
            output_dir: &build_dir,
            output_name: &project_name,
            run,
            check_only: check,
            release,
            dependencies: &dependencies,
            is_lib,
            capture_output: false,
        })
    }
}

fn format_dir_recursively(dir: &Path, write: bool) -> Result<(), String> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let path = entry.path();
            if path.is_dir() {
                format_dir_recursively(&path, write)?;
            } else if path.extension().is_some_and(|ext| ext == "pace") {
                match pace_driver::format_file(&path, write) {
                    Err(e) => eprintln!("Error formatting {}: {}", path.display(), e),
                    Ok(changed) => {
                        if changed && write {
                            println!("Formatted {}", path.display());
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

fn execute_fmt(file: Option<String>, write: bool) -> Result<(), String> {
    if let Some(f) = file {
        let path = Path::new(&f);
        match pace_driver::format_file(path, write) {
            Ok(changed) => {
                if changed && write {
                    println!("Formatted {}", path.display());
                }
                Ok(())
            }
            Err(e) => Err(e),
        }
    } else {
        let (root, _) = get_project_info()?;
        let src_dir = root.join("src");
        if src_dir.exists() {
            format_dir_recursively(&src_dir, write)
        } else {
            Err("No src/ directory found to format".to_string())
        }
    }
}

async fn resolve_package_deps_for_dir(
    dir: &Path,
) -> std::collections::HashMap<String, PathBuf> {
    let mut deps = std::collections::HashMap::new();

    let mut curr = Some(dir);
    let mut manifest_dir = None;
    while let Some(d) = curr {
        if d.join("pace.toml").exists() {
            manifest_dir = Some(d);
            break;
        }
        curr = d.parent();
    }

    if let Some(root) = manifest_dir {
        let toml_path = root.join("pace.toml");
        if let Ok(toml) = pace_pkg::parse_manifest(&toml_path) {
            deps.insert(toml.package.name.clone(), root.to_path_buf());

            let mut resolver = pace_pkg::resolve::DependencyResolver::new();
            let lockfile_path = root.join("pace.lock");
            if lockfile_path.exists()
                && let Ok(lock) =
                    pace_pkg::resolve::DependencyResolver::read_lockfile(&lockfile_path)
            {
                resolver.load_lock(lock);
            }
            if let Ok(lock) = resolver.resolve(&toml).await {
                let cache = pace_pkg::cache::CacheManager::new();
                for (name, info) in &lock.packages {
                    let mut path = cache.get_package_path(name, &info.version);
                    if let Some(source) = &info.source {
                        if source.starts_with("local+") {
                            if let Some(local_path) = source.strip_prefix("local+") {
                                path = root.join(local_path);
                            }
                        }
                    }
                    deps.insert(name.clone(), path);
                }
            }
        }
    }
    deps
}

async fn execute_test(filter: Option<String>, fail_fast: bool) -> Result<(), String> {
    let current_dir = env::current_dir().map_err(|_| "Failed to get current directory")?;

    let mut test_files = Vec::new();

    if let Some(ref f) = filter {
        let p = PathBuf::from(f);
        if p.is_file() {
            test_files.push(p);
        }
    }

    if test_files.is_empty() {
        let search_dirs = [current_dir.join("tests"), current_dir.join("src"), current_dir.clone()];
        for dir in &search_dirs {
            if dir.is_dir() {
                for entry in walkdir::WalkDir::new(dir).into_iter().filter_map(|e| e.ok()) {
                    let path = entry.path();
                    if path.is_file() {
                        let fname = path.file_name().unwrap_or_default().to_string_lossy();
                        if fname.ends_with(".test.pace") {
                            if let Some(ref f) = filter {
                                if path.to_string_lossy().contains(f) {
                                    test_files.push(path.to_path_buf());
                                }
                            } else {
                                test_files.push(path.to_path_buf());
                            }
                        }
                    }
                }
            }
        }
    }

    test_files.sort();
    test_files.dedup();

    if test_files.is_empty() {
        println!("No test files (*.test.pace) found.");
        return Ok(());
    }

    println!("\x1b[1mpace test\x1b[0m v{}", env!("CARGO_PKG_VERSION"));
    println!("Running {} test file(s)...\n", test_files.len());

    let mut passed = 0;
    let mut failed = 0;
    let suite_start = std::time::Instant::now();

    let output_dir = std::env::temp_dir().join(format!("pace-test-{}", std::process::id()));
    let _ = fs::create_dir_all(&output_dir);
    let _guard = std::mem::DropGuard::new(output_dir.clone(), |dir| {
        let _ = fs::remove_dir_all(&dir);
    });

    let mut package_deps_cache: std::collections::HashMap<
        PathBuf,
        std::collections::HashMap<String, PathBuf>,
    > = std::collections::HashMap::new();

    for test_file in &test_files {
        let test_dir = test_file.parent().unwrap_or(&current_dir);
        if !package_deps_cache.contains_key(test_dir) {
            let d = resolve_package_deps_for_dir(test_dir).await;
            package_deps_cache.insert(test_dir.to_path_buf(), d);
        }
    }

    let concurrency = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(concurrency));
    let aborted = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let mut handles = Vec::with_capacity(test_files.len());

    for (idx, test_file) in test_files.iter().enumerate() {
        let rel_path = test_file
            .strip_prefix(&current_dir)
            .unwrap_or(test_file)
            .to_path_buf();
        let test_name = test_file
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let test_dir = test_file.parent().unwrap_or(&current_dir);
        let deps = package_deps_cache
            .get(test_dir)
            .cloned()
            .unwrap_or_default();
        let test_out_dir = output_dir.join(format!("test_{}", idx));
        let test_file = test_file.clone();
        let sem = semaphore.clone();
        let aborted_flag = aborted.clone();

        handles.push(tokio::spawn(async move {
            let _permit = sem.acquire_owned().await.unwrap();
            if fail_fast && aborted_flag.load(std::sync::atomic::Ordering::Relaxed) {
                return (rel_path, None);
            }
            let _ = fs::create_dir_all(&test_out_dir);
            let start = std::time::Instant::now();
            let res = tokio::task::spawn_blocking(move || {
                pace_driver::compile_file(pace_driver::CompileOptions {
                    file_path: &test_file,
                    output_dir: &test_out_dir,
                    output_name: &test_name,
                    run: true,
                    check_only: false,
                    release: false,
                    dependencies: &deps,
                    is_lib: false,
                    capture_output: true,
                })
            })
            .await
            .unwrap_or_else(|e| Err(format!("Task panicked: {}", e)));

            let duration = start.elapsed();
            if res.is_err() && fail_fast {
                aborted_flag.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            (rel_path, Some((res, duration)))
        }));
    }

    for handle in handles {
        let (rel_path, outcome) = handle
            .await
            .map_err(|e| format!("Test runner join error: {}", e))?;
        let Some((res, duration)) = outcome else {
            continue;
        };
        let duration_str = format!("{:.1}ms", duration.as_secs_f64() * 1000.0);

        match res {
            Ok(()) => {
                println!(
                    "  \x1b[32m✓\x1b[0m {} \x1b[2m({})\x1b[0m",
                    rel_path.display(),
                    duration_str
                );
                passed += 1;
            }
            Err(e) => {
                println!(
                    "  \x1b[31m✗\x1b[0m {} \x1b[2m({})\x1b[0m",
                    rel_path.display(),
                    duration_str
                );
                eprintln!(
                    "    \x1b[31mError:\x1b[0m {}",
                    e.lines().collect::<Vec<_>>().join("\n    ")
                );
                failed += 1;
            }
        }
    }

    let total_duration = suite_start.elapsed();
    println!();
    if failed == 0 {
        println!(
            "\x1b[32m\x1b[1m{} passed\x1b[0m ({:.1}ms)",
            passed,
            total_duration.as_secs_f64() * 1000.0
        );
        Ok(())
    } else {
        println!(
            "\x1b[31m\x1b[1m{} failed\x1b[0m, \x1b[32m{} passed\x1b[0m ({:.1}ms)",
            failed,
            passed,
            total_duration.as_secs_f64() * 1000.0
        );
        Err(format!("{} test(s) failed", failed))
    }
}

fn fatal_exit(err: impl std::fmt::Display) -> ! {
    eprintln!("{}", err);
    std::process::exit(1);
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Lsp => {
            pace_lsp::start_server().await;
        }
        Commands::New { name, lib } => {
            if let Err(e) = pace_pkg::scaffold_project(&name, lib) {
                fatal_exit(format!("Error: {}", e));
            }
        }
        Commands::Build { file, debug } => {
            if let Err(e) = execute_build_or_run(file, false, false, !debug).await {
                fatal_exit(e);
            }
        }
        Commands::Check { file } => {
            if let Err(e) = execute_build_or_run(file, false, true, false).await {
                fatal_exit(e);
            }
        }
        Commands::Run { file, release } => {
            if let Err(e) = execute_build_or_run(file, true, false, release).await {
                fatal_exit(e);
            }
        }
        Commands::Test { filter, fail_fast } => {
            if let Err(e) = execute_test(filter, fail_fast).await {
                fatal_exit(e);
            }
        }
        Commands::Version => {
            println!("pace version {}", env!("CARGO_PKG_VERSION"));
        }
        Commands::Upgrade => {
            println!("Upgrading Pace toolchain from GitHub...");
            let status = Command::new("sh")
                .arg("-c")
                .arg("curl -sSf https://raw.githubusercontent.com/pace-lang/pace/main/installer/install.sh | sh")
                .status()
                .expect("Failed to run upgrade script");
            if status.success() {
                println!("Upgrade complete!");
            } else {
                fatal_exit("Upgrade failed.");
            }
        }
        Commands::Fmt { file, stdout } => {
            if let Err(e) = execute_fmt(file, !stdout) {
                fatal_exit(e);
            }
        }
        Commands::Add { name, version, dev } => {
            if let Err(e) = pace_pkg::commands::add_dependency(&name, version.as_deref(), dev).await {
                fatal_exit(format!("Error: {}", e));
            }
        }
        Commands::Install => {
            // Install behaves the same as update without `--latest`
            if let Err(e) = pace_pkg::commands::update_dependencies(false).await {
                fatal_exit(format!("Error: {}", e));
            }
        }
        Commands::Update { latest } => {
            if let Err(e) = pace_pkg::commands::update_dependencies(latest).await {
                fatal_exit(format!("Error: {}", e));
            }
        }
        Commands::Outdated => {
            if let Err(e) = pace_pkg::commands::list_outdated().await {
                fatal_exit(format!("Error: {}", e));
            }
        }
        Commands::Publish => {
            if let Err(e) = pace_pkg::commands::publish().await {
                fatal_exit(format!("Error: {}", e));
            }
        }
        Commands::Login { token } => {
            if let Err(e) = pace_pkg::commands::login(&token) {
                fatal_exit(format!("Error: {}", e));
            }
        }
        Commands::Remove { name } => {
            if let Err(e) = pace_pkg::commands::remove_dependency(&name).await {
                fatal_exit(format!("Error: {}", e));
            }
        }
        Commands::Clean { cache } => {
            if let Err(e) = pace_pkg::commands::clean(cache) {
                fatal_exit(format!("Error: {}", e));
            }
        }
    }
}
