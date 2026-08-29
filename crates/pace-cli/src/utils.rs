use miette::{Result, miette};

pub fn resolve_file(file: Option<String>) -> Result<String> {
    if let Some(f) = file {
        Ok(f)
    } else {
        if std::path::Path::new("pace.toml").exists() {
            if std::path::Path::new("src/main.pace").exists() {
                Ok("src/main.pace".to_string())
            } else {
                let current_dir = std::env::current_dir().unwrap();
                if let Ok(manifest) = pace_pkg::manifest::PaceToml::load_from_dir(&current_dir) {
                    let pkg_name = manifest.package.name;
                    let lib_path = format!("src/{}.pace", pkg_name);
                    if std::path::Path::new(&lib_path).exists() {
                        return Ok(lib_path);
                    }
                }
                Err(miette!(
                    "Default entry point 'src/main.pace' or 'src/<package-name>.pace' not found"
                ))
            }
        } else {
            Err(miette!(
                "No file specified and no pace.toml found in current directory"
            ))
        }
    }
}

pub fn check_sdk_compatibility(manifest: &pace_pkg::manifest::PaceToml) -> Result<()> {
    if let Some(sdk) = &manifest.sdk
        && let Some(req_str) = sdk.get("pace")
    {
        let sanitized_req = req_str.replace(" ", ", ").replace(",,", ",");
        let req = semver::VersionReq::parse(&sanitized_req)
            .map_err(|e| miette::miette!("Invalid SDK version requirement '{}': {}", req_str, e))?;

        let current_version_str = env!("CARGO_PKG_VERSION");
        let current_version = semver::Version::parse(current_version_str).map_err(|e| {
            miette::miette!(
                "Invalid current Pace version '{}': {}",
                current_version_str,
                e
            )
        })?;

        if !req.matches(&current_version) {
            return Err(miette::miette!(
                "Current Pace SDK version ({}) does not satisfy the requirement ({}) in pace.toml",
                current_version,
                req
            ));
        }
    }
    Ok(())
}
