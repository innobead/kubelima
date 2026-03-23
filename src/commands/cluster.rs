use anyhow::Result;
use chrono::Utc;
use colored::Colorize;

use crate::{
    config::{delete_cluster_state, list_clusters, load_cluster, save_cluster},
    lima::LimaClient,
    provisioner::{ProvisionContext, get_provisioner},
    types::{ClusterState, MountConfig, NodeConfig, NodeRole},
};

pub async fn create(
    name: &str,
    nodes: u32,
    distro: &str,
    cpus: u32,
    memory_gb: u32,
    disk_gb: u32,
    cloud_init: &[String],
) -> Result<()> {
    // Validate distro
    let provisioner = get_provisioner(distro)?;

    println!(
        "{} Creating cluster '{}' with {} node(s) using {} ...",
        "[kubelima]".cyan().bold(),
        name.yellow(),
        nodes,
        distro.yellow()
    );

    let server_vm = format!("{}-server-0", name);
    let server_ctx = ProvisionContext {
        vm_name: server_vm.clone(),
        cpus,
        memory_gb,
        disk_gb,
        cloud_init_scripts: cloud_init.to_vec(),
    };

    println!(
        "  {} Starting server VM '{}'",
        "+".green().bold(),
        server_vm
    );
    provisioner.create_server(&server_ctx).await?;

    let mut node_configs = vec![NodeConfig {
        name: server_vm.clone(),
        role: NodeRole::Server,
        index: 0,
    }];

    // Provision additional agent nodes (nodes - 1, since server counts as 1)
    let agent_count = nodes.saturating_sub(1) as usize;
    for i in 0..agent_count {
        let agent_vm = format!("{}-agent-{}", name, i);
        println!("  {} Starting agent VM '{}'", "+".green().bold(), agent_vm);
        let agent_ctx = ProvisionContext {
            vm_name: agent_vm.clone(),
            cpus,
            memory_gb,
            disk_gb,
            cloud_init_scripts: cloud_init.to_vec(),
        };
        provisioner.create_agent(&agent_ctx, &server_vm).await?;
        node_configs.push(NodeConfig {
            name: agent_vm,
            role: NodeRole::Agent,
            index: i,
        });
    }

    let state = ClusterState {
        name: name.to_string(),
        distro: distro.to_string(),
        cpus,
        memory_gb,
        disk_gb,
        created_at: Utc::now(),
        nodes: node_configs,
        cloud_init_scripts: cloud_init.to_vec(),
        mounts: vec![],
        service_exports: vec![],
    };
    save_cluster(&state)?;

    println!(
        "\n{} Cluster '{}' is ready.",
        "✓".green().bold(),
        name.yellow()
    );
    println!(
        "  Run: export KUBECONFIG=$(kubelima kubeconfig get --cluster {})",
        name
    );
    Ok(())
}

pub async fn list() -> Result<()> {
    let names = list_clusters()?;

    if names.is_empty() {
        println!("{}", "No clusters found.".dimmed());
        return Ok(());
    }

    // Fetch live VM data for status
    let live_vms = LimaClient::list_vms().await.unwrap_or_default();

    println!(
        "{:<20} {:<10} {:<8} {:<10} {:<10} {:<12} {}",
        "NAME".bold(),
        "DISTRO".bold(),
        "NODES".bold(),
        "CPUS".bold(),
        "MEMORY".bold(),
        "DISK".bold(),
        "STATUS".bold()
    );

    for cluster_name in &names {
        if let Ok(state) = load_cluster(cluster_name) {
            // Determine aggregate status from live VMs
            let vm_statuses: Vec<&str> = state
                .nodes
                .iter()
                .filter_map(|n| live_vms.iter().find(|v| v.name == n.name))
                .map(|v| v.status.as_str())
                .collect();

            let status = if vm_statuses.is_empty() {
                "unknown".to_string()
            } else if vm_statuses.iter().all(|s| *s == "Running") {
                "Running".green().to_string()
            } else if vm_statuses.iter().any(|s| *s == "Running") {
                "Partial".yellow().to_string()
            } else {
                "Stopped".red().to_string()
            };

            println!(
                "{:<20} {:<10} {:<8} {:<10} {:<10} {:<12} {}",
                cluster_name,
                state.distro,
                state.nodes.len(),
                state.cpus,
                format!("{}GiB", state.memory_gb),
                format!("{}GiB", state.disk_gb),
                status
            );
        }
    }
    Ok(())
}

pub async fn info(name: &str) -> Result<()> {
    let state = load_cluster(name)?;
    let live_vms = LimaClient::list_vms().await.unwrap_or_default();

    println!("{}", format!("Cluster: {}", name).cyan().bold());
    println!("  Distro:     {}", state.distro);
    println!("  CPUs:       {}", state.cpus);
    println!("  Memory:     {}GiB", state.memory_gb);
    println!("  Disk:       {}GiB", state.disk_gb);
    println!(
        "  Created:    {}",
        state.created_at.format("%Y-%m-%d %H:%M UTC")
    );

    println!("\n  {}", "Nodes:".bold());
    for node in &state.nodes {
        let vm = live_vms.iter().find(|v| v.name == node.name);
        let status = vm.map(|v| v.status.as_str()).unwrap_or("unknown");
        let ip = vm.and_then(|v| v.ip()).unwrap_or("-");
        let status_colored = match status {
            "Running" => status.green().to_string(),
            "Stopped" => status.red().to_string(),
            _ => status.yellow().to_string(),
        };
        println!(
            "    {:<30} {:<8} {:<12} {}",
            node.name,
            node.role.to_string().cyan(),
            ip,
            status_colored
        );
    }

    if !state.mounts.is_empty() {
        println!("\n  {}", "Mounts:".bold());
        for m in &state.mounts {
            let rw = if m.writable { "rw" } else { "ro" };
            println!("    {} -> {} [{}] ({})", m.local, m.remote, rw, m.node);
        }
    }

    if !state.service_exports.is_empty() {
        println!("\n  {}", "Service Exports:".bold());
        for svc in &state.service_exports {
            println!(
                "    {}/{} {}:{}",
                svc.namespace, svc.service, svc.local_port, svc.remote_port
            );
        }
    }

    if !state.cloud_init_scripts.is_empty() {
        println!("\n  {}", "Cloud-init scripts:".bold());
        for s in &state.cloud_init_scripts {
            println!("    {}", s);
        }
    }
    Ok(())
}

pub async fn delete(name: &str, force: bool) -> Result<()> {
    let state = load_cluster(name)?;

    println!(
        "{} Deleting cluster '{}' ...",
        "[kubelima]".cyan().bold(),
        name.yellow()
    );

    for node in &state.nodes {
        println!("  {} Removing VM '{}'", "-".red().bold(), node.name);
        if let Err(e) = LimaClient::stop_vm(&node.name).await {
            if force {
                eprintln!("    warn: stop failed ({}), continuing", e);
            } else {
                return Err(e);
            }
        }
        if let Err(e) = LimaClient::delete_vm(&node.name, force).await {
            if force {
                eprintln!("    warn: delete failed ({}), continuing", e);
            } else {
                return Err(e);
            }
        }
    }

    // Remove mount records for display; actual lima mount config is inside the VM
    let _: Vec<MountConfig> = state.mounts;

    delete_cluster_state(name)?;
    println!(
        "{} Cluster '{}' deleted.",
        "✓".green().bold(),
        name.yellow()
    );
    Ok(())
}
