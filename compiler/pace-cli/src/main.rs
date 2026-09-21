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
    version = "0.1.0",
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
    /// Build the current package
    Build {
        /// Optional path to a specific .pace file to build in script mode
        file: Option<String>,
    },
    /// Typecheck the current package
    Check {
        /// Optional path to a specific .pace file to check in script mode
        file: Option<String>,
    },
    /// Build and run the current package
    Run {
        /// Optional path to a specific .pace file to run in script mode
        file: Option<String>,
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

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Lsp => {
            pace_lsp::start_server().await;
        }
        Commands::New { name, lib } => {
            if let Err(e) = pace_pkg::scaffold_project(&name, lib) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Build { file } => {
            if let Err(e) = execute_build_or_run(file, false, false, true).await {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Commands::Check { file } => {
            if let Err(e) = execute_build_or_run(file, false, true, false).await {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Commands::Run { file } => {
            if let Err(e) = execute_build_or_run(file, true, false, false).await {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Commands::Version => {
            println!("pace version 0.1.0");
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
                eprintln!("Upgrade failed.");
                std::process::exit(1);
            }
        }
        Commands::Fmt { file, stdout } => {
            if let Err(e) = execute_fmt(file, !stdout) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
        Commands::Add { name, version, dev } => {
            if let Err(e) = pace_pkg::commands::add_dependency(&name, version.as_deref(), dev).await
            {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Install => {
            // Install behaves the same as update without `--latest`
            if let Err(e) = pace_pkg::commands::update_dependencies(false).await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Update { latest } => {
            if let Err(e) = pace_pkg::commands::update_dependencies(latest).await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Outdated => {
            if let Err(e) = pace_pkg::commands::list_outdated().await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Publish => {
            if let Err(e) = pace_pkg::commands::publish().await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Login { token } => {
            if let Err(e) = pace_pkg::commands::login(&token) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Remove { name } => {
            if let Err(e) = pace_pkg::commands::remove_dependency(&name).await {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
        Commands::Clean { cache } => {
            if let Err(e) = pace_pkg::commands::clean(cache) {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        }
    }
}
