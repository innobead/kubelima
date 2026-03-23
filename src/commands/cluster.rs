use anyhow::Result;
use chrono::Utc;
use cli_table::{
    Cell, Color, Style, Table,
    format::{Border, Separator},
    print_stdout,
};
use colored::Colorize;

use crate::{
    config::{delete_cluster_state, list_clusters, load_cluster, save_cluster},
    lima::LimaClient,
    provisioner::{ProvisionContext, get_provisioner},
    types::{ClusterState, MountConfig, NodeConfig, NodeRole},
};

pub async fn create(
    name: &str,
    control_plane: u32,
    workers: u32,
    distro: &str,
    cpus: u32,
    memory_gb: u32,
    disk_gb: u32,
    cloud_init: &[String],
) -> Result<()> {
    // Validate distro
    let provisioner = get_provisioner(distro)?;

    println!(
        "{} Creating cluster '{}' ({} control-plane, {} worker) using {} ...",
        "[kubelima]".cyan().bold(),
        name.yellow(),
        control_plane,
        workers,
        distro.yellow()
    );

    let mut node_configs = vec![];

    for i in 0..control_plane as usize {
        let cp_vm = format!("{}-cp-{}", name, i);
        println!(
            "  {} Starting control-plane VM '{}'",
            "+".green().bold(),
            cp_vm
        );
        let cp_ctx = ProvisionContext {
            vm_name: cp_vm.clone(),
            cpus,
            memory_gb,
            disk_gb,
            cloud_init_scripts: cloud_init.to_vec(),
        };
        provisioner.create_control_plane(&cp_ctx).await?;
        node_configs.push(NodeConfig {
            name: cp_vm,
            role: NodeRole::ControlPlane,
            index: i,
        });
    }

    let primary_cp_vm = node_configs
        .iter()
        .find(|n| n.role == NodeRole::ControlPlane)
        .map(|n| n.name.clone())
        .ok_or_else(|| anyhow::anyhow!("No control-plane node was created"))?;

    for i in 0..workers as usize {
        let worker_vm = format!("{}-worker-{}", name, i);
        println!(
            "  {} Starting worker VM '{}'",
            "+".green().bold(),
            worker_vm
        );
        let worker_ctx = ProvisionContext {
            vm_name: worker_vm.clone(),
            cpus,
            memory_gb,
            disk_gb,
            cloud_init_scripts: cloud_init.to_vec(),
        };
        provisioner
            .create_worker(&worker_ctx, &primary_cp_vm)
            .await?;
        node_configs.push(NodeConfig {
            name: worker_vm,
            role: NodeRole::Worker,
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

    let rows: Vec<Vec<cli_table::CellStruct>> = names
        .iter()
        .map(|cluster_name| match load_cluster(cluster_name) {
            Err(_) => vec![
                cluster_name.cell(),
                "?".cell(),
                "?".cell(),
                "?".cell(),
                "?".cell(),
                "?".cell(),
                "Unknown".cell().foreground_color(Some(Color::White)),
            ],
            Ok(state) => {
                let vm_statuses: Vec<&str> = state
                    .nodes
                    .iter()
                    .filter_map(|n| live_vms.iter().find(|v| v.name == n.name))
                    .map(|v| v.status.as_str())
                    .collect();

                let (status_str, status_color) = if vm_statuses.is_empty() {
                    ("Unknown".to_string(), Some(Color::White))
                } else if vm_statuses.iter().all(|s| *s == "Running") {
                    ("Running".to_string(), Some(Color::Green))
                } else if vm_statuses.iter().any(|s| *s == "Running") {
                    ("Partial".to_string(), Some(Color::Yellow))
                } else {
                    (vm_statuses[0].to_string(), Some(Color::Red))
                };

                vec![
                    cluster_name.cell(),
                    state.distro.cell(),
                    state.nodes.len().cell(),
                    state.cpus.cell(),
                    format!("{}GiB", state.memory_gb).cell(),
                    format!("{}GiB", state.disk_gb).cell(),
                    status_str.cell().foreground_color(status_color),
                ]
            }
        })
        .collect();

    let table = rows
        .table()
        .title(vec![
            "NAME".cell().bold(true),
            "DISTRO".cell().bold(true),
            "NODES".cell().bold(true),
            "CPUS".cell().bold(true),
            "MEMORY".cell().bold(true),
            "DISK".cell().bold(true),
            "STATUS".cell().bold(true),
        ])
        .border(Border::builder().build())
        .separator(Separator::builder().build());

    print_stdout(table)?;
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
    {
        let node_rows: Vec<Vec<cli_table::CellStruct>> = state
            .nodes
            .iter()
            .map(|node| {
                let vm = live_vms.iter().find(|v| v.name == node.name);
                let status = vm.map(|v| v.status.as_str()).unwrap_or("unknown");
                let ip = vm.and_then(|v| v.ip()).unwrap_or("-");
                let status_color = match status {
                    "Running" => Some(Color::Green),
                    "Stopped" => Some(Color::Red),
                    _ => Some(Color::Yellow),
                };
                vec![
                    node.name.clone().cell(),
                    node.role
                        .to_string()
                        .cell()
                        .foreground_color(Some(Color::Cyan)),
                    ip.cell(),
                    status.cell().foreground_color(status_color),
                ]
            })
            .collect();
        let node_table = node_rows
            .table()
            .title(vec![
                "NAME".cell().bold(true),
                "ROLE".cell().bold(true),
                "IP".cell().bold(true),
                "STATUS".cell().bold(true),
            ])
            .border(Border::builder().build())
            .separator(Separator::builder().build());
        print_stdout(node_table)?;
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
