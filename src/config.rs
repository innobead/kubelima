use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::types::ClusterState;

pub fn kubelima_dir() -> Result<PathBuf> {
    let home = dirs::home_dir().context("Cannot determine home directory")?;
    Ok(home.join(".kubelima"))
}

pub fn cluster_dir(name: &str) -> Result<PathBuf> {
    Ok(kubelima_dir()?.join("clusters").join(name))
}

pub fn cloud_init_dir(name: &str) -> Result<PathBuf> {
    Ok(cluster_dir(name)?.join("cloud-init"))
}

pub fn cluster_state_path(name: &str) -> Result<PathBuf> {
    Ok(cluster_dir(name)?.join("cluster.json"))
}

pub fn load_cluster(name: &str) -> Result<ClusterState> {
    let path = cluster_state_path(name)?;
    let content = std::fs::read_to_string(&path).with_context(|| {
        format!(
            "Cluster '{}' not found. Run `kubelima cluster list` to see available clusters.",
            name
        )
    })?;
    serde_json::from_str(&content)
        .with_context(|| format!("Failed to parse cluster state for '{}'", name))
}

pub fn save_cluster(state: &ClusterState) -> Result<()> {
    let dir = cluster_dir(&state.name)?;
    std::fs::create_dir_all(&dir)?;
    let path = cluster_state_path(&state.name)?;
    let content = serde_json::to_string_pretty(state)?;
    std::fs::write(path, content)?;
    Ok(())
}

pub fn delete_cluster_state(name: &str) -> Result<()> {
    let dir = cluster_dir(name)?;
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    Ok(())
}

pub fn list_clusters() -> Result<Vec<String>> {
    let dir = kubelima_dir()?.join("clusters");
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut names = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            let state_file = entry.path().join("cluster.json");
            if state_file.exists() {
                if let Some(name) = entry.file_name().to_str() {
                    names.push(name.to_string());
                }
            }
        }
    }
    names.sort();
    Ok(names)
}
