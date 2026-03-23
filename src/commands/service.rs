use anyhow::{Context, Result};
use colored::Colorize;
use tokio::process::Command;

use crate::{
    config::{load_cluster, save_cluster},
    provisioner::get_provisioner,
    types::ServiceExport,
};

pub async fn export(
    cluster: &str,
    service: &str,
    namespace: &str,
    local_port: u16,
    remote_port: u16,
) -> Result<()> {
    let mut state = load_cluster(cluster)?;
    let provisioner = get_provisioner(&state.distro)?;

    let server_vm = state
        .server_vm_name()
        .ok_or_else(|| anyhow::anyhow!("Cluster '{}' has no server node", cluster))?
        .to_string();

    // Get the kubeconfig path to pass to kubectl
    let kubeconfig = provisioner.kubeconfig_path(&server_vm)?;
    if !kubeconfig.exists() {
        anyhow::bail!(
            "Kubeconfig not found at {:?}. Is the cluster running?",
            kubeconfig
        );
    }

    // Record the export in state (idempotent)
    let already_recorded = state
        .service_exports
        .iter()
        .any(|e| e.service == service && e.namespace == namespace && e.local_port == local_port);
    if !already_recorded {
        state.service_exports.push(ServiceExport {
            service: service.to_string(),
            namespace: namespace.to_string(),
            local_port,
            remote_port,
        });
        save_cluster(&state)?;
    }

    println!(
        "{} Forwarding {}/{} :{} -> :{} (press Ctrl+C to stop)",
        "[kubelima]".cyan().bold(),
        namespace.yellow(),
        service.yellow(),
        local_port,
        remote_port
    );

    // Run kubectl port-forward in the foreground
    let status = Command::new("kubectl")
        .args([
            "port-forward",
            "-n",
            namespace,
            &format!("svc/{}", service),
            &format!("{}:{}", local_port, remote_port),
            "--address=0.0.0.0",
        ])
        .env("KUBECONFIG", &kubeconfig)
        .status()
        .await
        .context("Failed to run kubectl port-forward. Is kubectl installed?")?;

    if !status.success() {
        anyhow::bail!("kubectl port-forward exited with status: {}", status);
    }
    Ok(())
}

pub fn list(cluster: &str) -> Result<()> {
    let state = load_cluster(cluster)?;

    if state.service_exports.is_empty() {
        println!("{}", "No service exports configured.".dimmed());
        return Ok(());
    }

    println!(
        "{:<20} {:<15} {:<12} {}",
        "NAMESPACE".bold(),
        "SERVICE".bold(),
        "LOCAL PORT".bold(),
        "REMOTE PORT".bold()
    );
    for svc in &state.service_exports {
        println!(
            "{:<20} {:<15} {:<12} {}",
            svc.namespace, svc.service, svc.local_port, svc.remote_port
        );
    }
    Ok(())
}
