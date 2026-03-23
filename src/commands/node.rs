use anyhow::{Result, bail};
use colored::Colorize;

use crate::{
    config::{load_cluster, save_cluster},
    lima::LimaClient,
    provisioner::{ProvisionContext, get_provisioner},
    types::{NodeConfig, NodeRole},
};

pub async fn list(cluster: &str) -> Result<()> {
    let state = load_cluster(cluster)?;
    let live_vms = LimaClient::list_vms().await.unwrap_or_default();

    println!(
        "{:<30} {:<8} {:<8} {:<14} {:<10} {}",
        "NAME".bold(),
        "ROLE".bold(),
        "CPUS".bold(),
        "MEMORY".bold(),
        "IP".bold(),
        "STATUS".bold()
    );

    for node in &state.nodes {
        let vm = live_vms.iter().find(|v| v.name == node.name);
        let status = vm.map(|v| v.status.as_str()).unwrap_or("unknown");
        let ip = vm.and_then(|v| v.ip()).unwrap_or("-").to_string();
        let cpus = vm.map(|v| v.cpus.to_string()).unwrap_or("-".into());
        let mem = vm
            .map(|v| format!("{:.1}GiB", v.memory_gb()))
            .unwrap_or("-".into());

        let status_colored = match status {
            "Running" => status.green().to_string(),
            "Stopped" => status.red().to_string(),
            _ => status.yellow().to_string(),
        };

        println!(
            "{:<30} {:<8} {:<8} {:<14} {:<10} {}",
            node.name,
            node.role.to_string().cyan(),
            cpus,
            mem,
            ip,
            status_colored
        );
    }
    Ok(())
}

pub async fn info(cluster: &str, node_name: &str) -> Result<()> {
    let state = load_cluster(cluster)?;
    let node = state
        .nodes
        .iter()
        .find(|n| n.name == node_name)
        .ok_or_else(|| {
            anyhow::anyhow!("Node '{}' not found in cluster '{}'", node_name, cluster)
        })?;

    let vm = LimaClient::get_vm(node_name).await?;

    println!("{}", format!("Node: {}", node_name).cyan().bold());
    println!("  Role:    {}", node.role);
    println!("  Cluster: {}", cluster);

    if let Some(vm) = &vm {
        println!("  Status:  {}", vm.status);
        println!("  CPUs:    {}", vm.cpus);
        println!("  Memory:  {:.1}GiB", vm.memory_gb());
        println!("  Disk:    {:.1}GiB", vm.disk_gb());
        println!("  IP:      {}", vm.ip().unwrap_or("-"));
        println!("  SSH Port: {}", vm.ssh_local_port);
        println!("  VM Type: {}", vm.vm_type);
    } else {
        println!("  Status:  {}", "not found in lima".red());
    }

    let node_mounts: Vec<_> = state
        .mounts
        .iter()
        .filter(|m| m.node == node_name)
        .collect();
    if !node_mounts.is_empty() {
        println!("\n  {}:", "Mounts".bold());
        for m in node_mounts {
            let rw = if m.writable {
                "read-write"
            } else {
                "read-only"
            };
            println!("    {} -> {} ({})", m.local, m.remote, rw);
        }
    }
    Ok(())
}

pub async fn add(cluster: &str, count: u32) -> Result<()> {
    let mut state = load_cluster(cluster)?;
    let provisioner = get_provisioner(&state.distro)?;

    let server_vm = state
        .server_vm_name()
        .ok_or_else(|| anyhow::anyhow!("Cluster '{}' has no server node", cluster))?
        .to_string();

    let start_index = state.next_agent_index();

    println!(
        "{} Adding {} agent node(s) to cluster '{}' ...",
        "[kubelima]".cyan().bold(),
        count,
        cluster.yellow()
    );

    for i in 0..count as usize {
        let agent_vm = format!("{}-agent-{}", cluster, start_index + i);
        println!("  {} Starting agent VM '{}'", "+".green().bold(), agent_vm);
        let ctx = ProvisionContext {
            vm_name: agent_vm.clone(),
            cpus: state.cpus,
            memory_gb: state.memory_gb,
            disk_gb: state.disk_gb,
            cloud_init_scripts: state.cloud_init_scripts.clone(),
        };
        provisioner.create_agent(&ctx, &server_vm).await?;
        state.nodes.push(NodeConfig {
            name: agent_vm,
            role: NodeRole::Agent,
            index: start_index + i,
        });
    }

    save_cluster(&state)?;
    println!(
        "{} Added {} agent(s) to cluster '{}'.",
        "✓".green().bold(),
        count,
        cluster.yellow()
    );
    Ok(())
}

pub async fn delete(cluster: &str, node_name: &str, force: bool) -> Result<()> {
    let mut state = load_cluster(cluster)?;

    let node_idx = state
        .nodes
        .iter()
        .position(|n| n.name == node_name)
        .ok_or_else(|| {
            anyhow::anyhow!("Node '{}' not found in cluster '{}'", node_name, cluster)
        })?;

    if state.nodes[node_idx].role == NodeRole::Server && state.nodes.len() > 1 {
        bail!(
            "Cannot delete server node '{}' while agent nodes exist. Delete the cluster instead.",
            node_name
        );
    }

    println!(
        "{} Removing node '{}' from cluster '{}' ...",
        "[kubelima]".cyan().bold(),
        node_name.yellow(),
        cluster.yellow()
    );

    LimaClient::stop_vm(node_name).await?;
    LimaClient::delete_vm(node_name, force).await?;

    state.nodes.remove(node_idx);
    // Remove any mounts associated with this node
    state.mounts.retain(|m| m.node != node_name);
    save_cluster(&state)?;

    println!(
        "{} Node '{}' removed.",
        "✓".green().bold(),
        node_name.yellow()
    );
    Ok(())
}

pub async fn ssh(cluster: &str, node_name: Option<&str>) -> Result<()> {
    let state = load_cluster(cluster)?;

    let vm_name = match node_name {
        Some(n) => state
            .nodes
            .iter()
            .find(|node| node.name == n)
            .ok_or_else(|| anyhow::anyhow!("Node '{}' not found in cluster '{}'", n, cluster))?
            .name
            .clone(),
        None => {
            // Default to the server node
            state
                .server_vm_name()
                .ok_or_else(|| anyhow::anyhow!("Cluster '{}' has no server node", cluster))?
                .to_string()
        }
    };

    println!(
        "{} Opening SSH session to '{}' ...",
        "[kubelima]".cyan().bold(),
        vm_name.yellow()
    );
    LimaClient::ssh_interactive(&vm_name).await
}
