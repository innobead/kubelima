use anyhow::{Context, Result, bail};
use colored::Colorize;

use crate::{
    config::{load_cluster, save_cluster},
    lima::LimaClient,
    types::MountConfig,
};

pub async fn add(
    cluster: &str,
    node_name: &str,
    local: &str,
    remote: &str,
    readonly: bool,
) -> Result<()> {
    let mut state = load_cluster(cluster)?;

    // Verify the node belongs to this cluster
    if !state.nodes.iter().any(|n| n.name == node_name) {
        bail!("Node '{}' not found in cluster '{}'", node_name, cluster);
    }

    // Verify local path exists
    let local_path = std::path::Path::new(local);
    if !local_path.exists() {
        bail!("Local path '{}' does not exist", local);
    }
    let local_abs = local_path
        .canonicalize()
        .context("Failed to resolve local path")?;
    let local_str = local_abs.to_string_lossy().to_string();

    // Check for duplicate
    if state
        .mounts
        .iter()
        .any(|m| m.node == node_name && m.local == local_str)
    {
        bail!(
            "Mount '{}' already exists on node '{}'",
            local_str,
            node_name
        );
    }

    let writable = !readonly;
    let mount_type = if writable { "read-write" } else { "read-only" };

    println!(
        "{} Adding {} mount: {} -> {} on '{}'",
        "[kubelima]".cyan().bold(),
        mount_type,
        local_str.yellow(),
        remote.yellow(),
        node_name
    );

    // Stop the VM, patch its lima.yaml, then restart
    LimaClient::stop_vm(node_name).await?;

    LimaClient::edit_vm_yaml(node_name, |config| {
        let mounts = config.get_mut("mounts").and_then(|v| v.as_sequence_mut());

        let mut mount_entry = serde_yaml::Mapping::new();
        mount_entry.insert(
            serde_yaml::Value::String("location".into()),
            serde_yaml::Value::String(local_str.clone()),
        );
        mount_entry.insert(
            serde_yaml::Value::String("writable".into()),
            serde_yaml::Value::Bool(writable),
        );
        mount_entry.insert(
            serde_yaml::Value::String("mountPoint".into()),
            serde_yaml::Value::String(remote.into()),
        );

        if let Some(seq) = mounts {
            seq.push(serde_yaml::Value::Mapping(mount_entry));
        } else {
            config["mounts"] =
                serde_yaml::Value::Sequence(vec![serde_yaml::Value::Mapping(mount_entry)]);
        }
        Ok(())
    })
    .await?;

    LimaClient::restart_vm(node_name).await?;

    state.mounts.push(MountConfig {
        node: node_name.to_string(),
        local: local_str,
        remote: remote.to_string(),
        writable,
    });
    save_cluster(&state)?;

    println!("{} Mount added.", "✓".green().bold());
    Ok(())
}

pub fn list(cluster: &str, node_filter: Option<&str>) -> Result<()> {
    let state = load_cluster(cluster)?;

    let mounts: Vec<_> = state
        .mounts
        .iter()
        .filter(|m| node_filter.is_none() || node_filter == Some(m.node.as_str()))
        .collect();

    if mounts.is_empty() {
        println!("{}", "No mounts configured.".dimmed());
        return Ok(());
    }

    println!(
        "{:<30} {:<20} {:<30} {}",
        "NODE".bold(),
        "LOCAL".bold(),
        "REMOTE".bold(),
        "MODE".bold()
    );
    for m in mounts {
        let mode = if m.writable {
            "read-write"
        } else {
            "read-only"
        };
        println!("{:<30} {:<20} {:<30} {}", m.node, m.local, m.remote, mode);
    }
    Ok(())
}

pub async fn remove(cluster: &str, node_name: &str, local: &str) -> Result<()> {
    let mut state = load_cluster(cluster)?;

    let pos = state
        .mounts
        .iter()
        .position(|m| m.node == node_name && m.local == local)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Mount '{}' not found on node '{}' in cluster '{}'",
                local,
                node_name,
                cluster
            )
        })?;

    println!(
        "{} Removing mount '{}' from '{}' ...",
        "[kubelima]".cyan().bold(),
        local.yellow(),
        node_name.yellow()
    );

    LimaClient::stop_vm(node_name).await?;

    let local_owned = local.to_string();
    LimaClient::edit_vm_yaml(node_name, |config| {
        if let Some(seq) = config.get_mut("mounts").and_then(|v| v.as_sequence_mut()) {
            seq.retain(|entry| {
                entry
                    .get("location")
                    .and_then(|v| v.as_str())
                    .map(|loc| loc != local_owned)
                    .unwrap_or(true)
            });
        }
        Ok(())
    })
    .await?;

    LimaClient::restart_vm(node_name).await?;

    state.mounts.remove(pos);
    save_cluster(&state)?;

    println!("{} Mount removed.", "✓".green().bold());
    Ok(())
}
