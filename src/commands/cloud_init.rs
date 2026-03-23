use anyhow::{Context, Result, bail};
use colored::Colorize;

use crate::config::{cloud_init_dir, load_cluster, save_cluster};

pub fn add(cluster: &str, script: &str) -> Result<()> {
    let mut state = load_cluster(cluster)?;

    let script_path = std::path::Path::new(script);
    if !script_path.exists() {
        bail!("Script file '{}' does not exist", script);
    }

    let script_abs = script_path
        .canonicalize()
        .context("Failed to resolve script path")?;

    // Copy the script into the cluster's cloud-init dir for persistence
    let ci_dir = cloud_init_dir(cluster)?;
    std::fs::create_dir_all(&ci_dir)?;

    let dest = ci_dir.join(
        script_path
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Script path '{}' has no filename", script))?,
    );
    std::fs::copy(&script_abs, &dest)
        .with_context(|| format!("Failed to copy script to {:?}", dest))?;

    let dest_str = dest.to_string_lossy().to_string();

    if state.cloud_init_scripts.contains(&dest_str) {
        println!("{} Script already registered.", "·".dimmed());
        return Ok(());
    }

    state.cloud_init_scripts.push(dest_str.clone());
    save_cluster(&state)?;

    println!(
        "{} Cloud-init script '{}' added to cluster '{}'.",
        "✓".green().bold(),
        dest_str.yellow(),
        cluster.yellow()
    );
    println!(
        "  {}",
        "Note: scripts apply to newly provisioned nodes only.".dimmed()
    );
    Ok(())
}

pub fn list(cluster: &str) -> Result<()> {
    let state = load_cluster(cluster)?;

    if state.cloud_init_scripts.is_empty() {
        println!("{}", "No cloud-init scripts configured.".dimmed());
        return Ok(());
    }

    println!("{}", "Cloud-init scripts:".bold());
    for script in &state.cloud_init_scripts {
        println!("  {}", script);
    }
    Ok(())
}

pub fn remove(cluster: &str, script: &str) -> Result<()> {
    let mut state = load_cluster(cluster)?;

    let pos = state
        .cloud_init_scripts
        .iter()
        .position(|s| s == script || s.ends_with(script))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Script '{}' not found in cluster '{}' cloud-init configuration",
                script,
                cluster
            )
        })?;

    state.cloud_init_scripts.remove(pos);
    save_cluster(&state)?;

    println!(
        "{} Script '{}' removed from cluster '{}'.",
        "✓".green().bold(),
        script.yellow(),
        cluster.yellow()
    );
    Ok(())
}
