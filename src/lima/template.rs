use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// A provision step embedded in a lima VM config.
#[derive(Debug, Serialize, Deserialize)]
pub struct ProvisionStep {
    pub mode: String,
    pub script: String,
}

/// Typed representation of a lima VM YAML configuration.
///
/// Fields are serialized/deserialized directly by serde — no manual
/// `serde_yaml::Mapping` manipulation needed. Reusable across provisioners.
#[derive(Debug, Serialize, Deserialize)]
pub struct VmTemplate {
    pub base: String,
    pub cpus: u32,
    pub memory: String,
    pub disk: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub param: Option<HashMap<String, String>>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub provision: Vec<ProvisionStep>,
}

impl VmTemplate {
    pub fn new(base: impl Into<String>, cpus: u32, memory_gb: u32, disk_gb: u32) -> Self {
        Self {
            base: base.into(),
            cpus,
            memory: format!("{}GiB", memory_gb),
            disk: format!("{}GiB", disk_gb),
            param: None,
            provision: Vec::new(),
        }
    }

    /// Set a key/value pair under `param`, creating the map if absent.
    pub fn set_param(&mut self, key: impl Into<String>, value: impl Into<String>) {
        self.param
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value.into());
    }

    /// Append a provision step that runs `script` in system mode.
    pub fn add_provision_script(&mut self, script: impl Into<String>) {
        self.provision.push(ProvisionStep {
            mode: "system".into(),
            script: script.into(),
        });
    }

    /// Serialize to a YAML string.
    pub fn to_yaml(&self) -> Result<String> {
        serde_yaml::to_string(self).context("Failed to serialize VM template YAML")
    }

    /// Write YAML to a temp file and return its path.
    pub fn write_temp(&self, vm_name: &str) -> Result<PathBuf> {
        let content = self.to_yaml()?;
        let path = std::env::temp_dir().join(format!("kubelima-{}.yaml", vm_name));
        std::fs::write(&path, &content)
            .with_context(|| format!("Failed to write temp YAML to {:?}", path))?;
        Ok(path)
    }
}