use miette::{Result, miette};
use pubgrub::{
    Dependencies, DependencyConstraints, DependencyProvider, PackageResolutionStatistics, Ranges,
    SemanticVersion,
};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PackageName(pub String);

impl std::fmt::Display for PackageName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub struct RegistryProvider {
    cache: Arc<
        Mutex<
            HashMap<
                PackageName,
                HashMap<
                    SemanticVersion,
                    Dependencies<PackageName, Ranges<SemanticVersion>, String>,
                >,
            >,
        >,
    >,
    versions_cache: Arc<Mutex<HashMap<PackageName, Vec<SemanticVersion>>>>,
}

impl RegistryProvider {
    pub fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
            versions_cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn add_root_dependencies(
        &self,
        root_name: PackageName,
        root_version: SemanticVersion,
        deps: HashMap<String, Ranges<SemanticVersion>>,
    ) {
        let mut v_cache = self.versions_cache.lock().unwrap();
        v_cache.insert(root_name.clone(), vec![root_version.clone()]);

        let mut c = self.cache.lock().unwrap();
        let pkg_cache = c.entry(root_name).or_insert_with(HashMap::new);

        let mut pubgrub_deps = Vec::new();
        for (k, v) in deps {
            pubgrub_deps.push((PackageName(k), v));
        }
        pkg_cache.insert(
            root_version,
            Dependencies::Available(pubgrub_deps.into_iter().collect()),
        );
    }

    fn fetch_package_info(&self, package: &PackageName) -> Result<(), miette::Report> {
        let mut v_cache = self.versions_cache.lock().unwrap();
        if v_cache.contains_key(package) {
            return Ok(());
        }

        let registry_url = std::env::var("PACE_REGISTRY_URL")
            .unwrap_or_else(|_| "https://registry.pace.dev".to_string());
        let url = format!("{}/api/packages/{}", registry_url, package.0);

        let resp = match ureq::get(&url).call() {
            Ok(r) => {
                if r.status() == 404 {
                    return Err(miette!("Package '{}' not found in registry", package.0));
                } else if r.status() != 200 {
                    return Err(miette!("Registry returned error status: {}", r.status()));
                }
                r
            }
            Err(e) => return Err(miette!("Failed to connect to registry: {}", e)),
        };

        let parsed: crate::fetcher::RegistryResponse = resp
            .into_body()
            .read_json()
            .map_err(|e| miette!("Failed to parse registry response: {}", e))?;

        let mut available_versions = Vec::new();
        let mut c = self.cache.lock().unwrap();
        let pkg_cache = c.entry(package.clone()).or_insert_with(HashMap::new);

        if let Some(info_list) = parsed.version_info {
            for info in info_list {
                // Parse x.y.z into pubgrub's SemanticVersion
                let parts: Vec<&str> = info.version.split('.').collect();
                if parts.len() >= 3 {
                    if let (Ok(major), Ok(minor), Ok(patch)) =
                        (parts[0].parse(), parts[1].parse(), parts[2].parse())
                    {
                        let v = SemanticVersion::new(major, minor, patch);
                        available_versions.push(v.clone());

                        // Parse dependencies
                        let mut pubgrub_deps = Vec::new();
                        if let Some(deps_map) = info.dependencies {
                            for (dep_name, constraint) in deps_map {
                                // Parse standard semver constraints properly
                                pubgrub_deps.push((
                                    PackageName(dep_name),
                                    crate::utils::parse_range(&constraint),
                                ));
                            }
                        }

                        pkg_cache.insert(
                            v,
                            Dependencies::Available(pubgrub_deps.into_iter().collect()),
                        );
                    }
                }
            }
        } else {
            // Fallback for older registry responses that only have versions/latest_version
            let mut fallback_versions = Vec::new();
            if let Some(versions) = parsed.versions {
                fallback_versions.extend(versions);
            } else if let Some(latest) = parsed.latest_version {
                fallback_versions.push(latest);
            }

            for version_str in fallback_versions {
                let parts: Vec<&str> = version_str.split('.').collect();
                if parts.len() >= 3 {
                    if let (Ok(major), Ok(minor), Ok(patch)) =
                        (parts[0].parse(), parts[1].parse(), parts[2].parse())
                    {
                        let v = SemanticVersion::new(major, minor, patch);
                        available_versions.push(v.clone());
                        pkg_cache
                            .insert(v, Dependencies::Available(DependencyConstraints::default()));
                    }
                }
            }
        }

        available_versions.sort();
        available_versions.reverse();
        v_cache.insert(package.clone(), available_versions);

        Ok(())
    }
}

// Define a proper std::error::Error type for our provider
#[derive(Debug, thiserror::Error)]
pub enum ResolverError {
    #[error("API Error: {0}")]
    Api(String),
}

impl DependencyProvider for RegistryProvider {
    type P = PackageName;
    type V = SemanticVersion;
    type VS = Ranges<SemanticVersion>;
    type M = String;
    type Err = ResolverError;
    type Priority = usize;

    fn choose_version(
        &self,
        package: &Self::P,
        range: &Self::VS,
    ) -> std::result::Result<Option<Self::V>, Self::Err> {
        if let Err(e) = self.fetch_package_info(package) {
            return Err(ResolverError::Api(e.to_string()));
        }

        let cache = self.versions_cache.lock().unwrap();
        if let Some(versions) = cache.get(package) {
            for v in versions {
                if range.contains(v) {
                    return Ok(Some(v.clone()));
                }
            }
        }
        Ok(None)
    }

    fn prioritize(
        &self,
        _package: &Self::P,
        _range: &Self::VS,
        _conflicts_counts: &PackageResolutionStatistics,
    ) -> Self::Priority {
        0
    }

    fn get_dependencies(
        &self,
        package: &Self::P,
        version: &Self::V,
    ) -> std::result::Result<Dependencies<Self::P, Self::VS, Self::M>, Self::Err> {
        if let Err(e) = self.fetch_package_info(package) {
            return Err(ResolverError::Api(e.to_string()));
        }

        let cache = self.cache.lock().unwrap();
        if let Some(pkg_cache) = cache.get(package) {
            if let Some(deps) = pkg_cache.get(version) {
                return Ok(deps.clone());
            }
        }

        Ok(Dependencies::Unavailable(format!(
            "Not found: {}@{}",
            package, version
        )))
    }
}
