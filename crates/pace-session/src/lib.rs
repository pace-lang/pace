pub mod options;

pub use options::{Options, OutputFormat};

use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default)]
pub struct Session {
    pub options: Options,
}

impl Session {
    pub fn new(options: Options) -> Self {
        Self { options }
    }

    pub fn resolve_import_path(
        &self,
        import_path: &str,
        path_buf: &Path,
    ) -> Result<PathBuf, miette::Report> {
        let target_platform = &self.options.target_platform;

        if import_path.starts_with("pace:") {
            Self::resolve_stdlib_path(import_path)
        } else if import_path.starts_with("self:") {
            Self::resolve_self_path(import_path, path_buf)
        } else if import_path.starts_with("./") || import_path.starts_with("../") {
            Self::resolve_relative_path(import_path, path_buf)
        } else if import_path.starts_with("package:") {
            Self::resolve_package_path(import_path, path_buf, target_platform)
        } else {
            Err(miette::miette!(
                "Invalid import path format: '{}'. Must start with pace:, package:, self:, or ./",
                import_path
            ))
        }
    }

    fn resolve_stdlib_path(import_path: &str) -> Result<PathBuf, miette::Report> {
        let path_without_pace = import_path.strip_prefix("pace:").unwrap_or(import_path);
        let resolved_path = if let Ok(stdlib_path) = std::env::var("PACE_STDLIB") {
            Path::new(&stdlib_path).join(format!("{}.pace", path_without_pace))
        } else if let Ok(home_path) = std::env::var("PACE_HOME") {
            Path::new(&home_path)
                .join("stdlib")
                .join(format!("{}.pace", path_without_pace))
        } else {
            // Fallback to compile-time repository root
            let manifest_dir = env!("CARGO_MANIFEST_DIR");
            let fallback_path = Path::new(manifest_dir).join("../../stdlib");
            fallback_path.join(format!("{}.pace", path_without_pace))
        };

        if !resolved_path.exists() {
            Err(miette::miette!(
                "Package Error: Standard library not found at '{}'. Please set PACE_STDLIB or PACE_HOME.",
                resolved_path.display()
            ))
        } else {
            Ok(resolved_path)
        }
    }

    fn resolve_self_path(
        import_path: &str,
        path_buf: &Path,
    ) -> Result<PathBuf, miette::Report> {
        let path_without_self = import_path.strip_prefix("self:").unwrap();
        let mut current_dir = path_buf
            .parent()
            .unwrap_or(Path::new(""))
            .to_path_buf();
        while !current_dir.join("pace.toml").exists() && current_dir.parent().is_some() {
            current_dir = current_dir.parent().unwrap().to_path_buf();
        }
        Ok(current_dir
            .join("src")
            .join(format!("{}.pace", path_without_self)))
    }

    fn resolve_relative_path(
        import_path: &str,
        path_buf: &Path,
    ) -> Result<PathBuf, miette::Report> {
        let parent_dir = path_buf.parent().unwrap_or(Path::new(""));
        Ok(parent_dir.join(format!("{}.pace", import_path)))
    }

    fn resolve_package_path(
        import_path: &str,
        path_buf: &Path,
        target_platform: &str,
    ) -> Result<PathBuf, miette::Report> {
        let path_without_pkg = import_path.strip_prefix("package:").unwrap();

        let (pkg_name, sub_path) = if let Some(idx) = path_without_pkg.find('/') {
            (&path_without_pkg[..idx], &path_without_pkg[idx + 1..])
        } else {
            (path_without_pkg, path_without_pkg)
        };

        let mut current_dir = path_buf
            .parent()
            .unwrap_or(Path::new(""))
            .to_path_buf();
        while !current_dir.join("pace.toml").exists() && current_dir.parent().is_some() {
            current_dir = current_dir.parent().unwrap().to_path_buf();
        }

        let package_root = current_dir.join(".pace").join("deps").join(pkg_name);
        if !package_root.exists() {
            return Err(miette::miette!(
                "Package '{}' not found in dependencies. Run `pace fetch` first.",
                pkg_name
            ));
        }

        let resolved_path = package_root.join(format!("{}.pace", sub_path));

        if resolved_path.exists() {
            Ok(resolved_path)
        } else {
            // Try platform-specific implementation
            let platform_path = package_root
                .join("impl")
                .join(target_platform)
                .join(format!("{}.pace", sub_path));

            if platform_path.exists() {
                Ok(platform_path)
            } else {
                Err(miette::miette!(
                    "Module '{}' not found in package '{}' (checked generic and {} specific paths)",
                    sub_path,
                    pkg_name,
                    target_platform
                ))
            }
        }
    }
}
