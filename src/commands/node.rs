use anyhow::{Result, bail};
use cli_table::{
    Cell, Color, Style, Table,
    format::{Border, Justify, Separator},
    print_stdout,
};
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

    let rows: Vec<Vec<cli_table::CellStruct>> = state
        .nodes
        .iter()
        .map(|node| {
            let vm = live_vms.iter().find(|v| v.name == node.name);
            let status = vm.map(|v| v.status.as_str()).unwrap_or("unknown");
            let ip = vm.and_then(|v| v.ip()).unwrap_or("-").to_string();
            let cpus = vm.map(|v| v.cpus.to_string()).unwrap_or("-".into());
            let mem = vm
                .map(|v| format!("{:.1}GiB", v.memory_gb()))
                .unwrap_or("-".into());

            let (status_color, status_str) = match status {
                "Running" => (Some(Color::Green), status.to_string()),
                "Stopped" => (Some(Color::Red), status.to_string()),
                _ => (Some(Color::Yellow), status.to_string()),
            };

            vec![
                node.name.clone().cell(),
                node.role
                    .to_string()
                    .cell()
                    .foreground_color(Some(Color::Cyan)),
                cpus.cell().justify(Justify::Right),
                mem.cell().justify(Justify::Right),
                ip.cell(),
                status_str.cell().foreground_color(status_color),
            ]
        })
        .collect();

    let table = rows
        .table()
        .title(vec![
            "NAME".cell().bold(true),
            "ROLE".cell().bold(true),
            "CPUS".cell().bold(true),
            "MEMORY".cell().bold(true),
            "IP".cell().bold(true),
            "STATUS".cell().bold(true),
        ])
        .border(Border::builder().build())
        .separator(Separator::builder().build());

    print_stdout(table)?;
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

pub async fn add(cluster: &str, control_plane: u32, workers: u32) -> Result<()> {
    let mut state = load_cluster(cluster)?;
    let provisioner = get_provisioner(&state.distro)?;

    let primary_cp_vm = state
        .control_plane_vm_name()
        .ok_or_else(|| anyhow::anyhow!("Cluster '{}' has no control-plane node", cluster))?
        .to_string();

    println!(
        "{} Adding {} control-plane and {} worker node(s) to cluster '{}' ...",
        "[kubelima]".cyan().bold(),
        control_plane,
        workers,
        cluster.yellow()
    );

    let cp_start = state.next_control_plane_index();
    for i in 0..control_plane as usize {
        let cp_vm = format!("{}-cp-{}", cluster, cp_start + i);
        println!(
            "  {} Starting control-plane VM '{}'",
            "+".green().bold(),
            cp_vm
        );
        let ctx = ProvisionContext {
            vm_name: cp_vm.clone(),
            cpus: state.cpus,
            memory_gb: state.memory_gb,
            disk_gb: state.disk_gb,
            cloud_init_scripts: state.cloud_init_scripts.clone(),
        };
        provisioner.create_control_plane(&ctx).await?;
        state.nodes.push(NodeConfig {
            name: cp_vm,
            role: NodeRole::ControlPlane,
            index: cp_start + i,
        });
    }

    let worker_start = state.next_worker_index();
    for i in 0..workers as usize {
        let worker_vm = format!("{}-worker-{}", cluster, worker_start + i);
        println!(
            "  {} Starting worker VM '{}'",
            "+".green().bold(),
            worker_vm
        );
        let ctx = ProvisionContext {
            vm_name: worker_vm.clone(),
            cpus: state.cpus,
            memory_gb: state.memory_gb,
            disk_gb: state.disk_gb,
            cloud_init_scripts: state.cloud_init_scripts.clone(),
        };
        provisioner.create_worker(&ctx, &primary_cp_vm).await?;
        state.nodes.push(NodeConfig {
            name: worker_vm,
            role: NodeRole::Worker,
            index: worker_start + i,
        });
    }

    save_cluster(&state)?;
    println!(
        "{} Added {} control-plane and {} worker node(s) to cluster '{}'.",
        "✓".green().bold(),
        control_plane,
        workers,
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

    if state.nodes[node_idx].role == NodeRole::ControlPlane && state.nodes.len() > 1 {
        bail!(
            "Cannot delete control-plane node '{}' while other nodes exist. Delete the cluster instead.",
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
            // Default to the first control-plane node
            state
                .control_plane_vm_name()
                .ok_or_else(|| anyhow::anyhow!("Cluster '{}' has no control-plane node", cluster))?
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
