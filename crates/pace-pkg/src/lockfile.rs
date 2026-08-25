use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use miette::Result;
use crate::manifest::ManifestError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PaceLock {
    #[serde(default)]
    pub packages: HashMap<String, LockedPackage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LockedPackage {
    pub version: Option<String>,
    pub git: Option<String>,
    pub rev: Option<String>,
    pub path: Option<String>,
}

impl PaceLock {
    /// Load and parse a pace.lock file from the given directory path
    pub fn load_from_dir(dir: &Path) -> Result<Option<Self>, ManifestError> {
        let lock_path = dir.join("pace.lock");
        if !lock_path.exists() {
            return Ok(None);
        }
        let content = fs::read_to_string(&lock_path)?;
        let lock: PaceLock = toml::from_str(&content)?;
        Ok(Some(lock))
    }

    /// Save the lockfile to a directory
    pub fn save_to_dir(&self, dir: &Path) -> Result<(), ManifestError> {
        let lock_path = dir.join("pace.lock");
        let content = toml::to_string_pretty(self)?;
        fs::write(&lock_path, content)?;
        Ok(())
    }
}
