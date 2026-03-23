use anyhow::{Context, Result};
use colored::Colorize;
use std::path::PathBuf;

use crate::{config::load_cluster, provisioner::get_provisioner};

pub async fn get(cluster: &str, output: Option<&str>, merge: bool) -> Result<()> {
    let state = load_cluster(cluster)?;
    let provisioner = get_provisioner(&state.distro)?;

    let server_vm = state
        .server_vm_name()
        .ok_or_else(|| anyhow::anyhow!("Cluster '{}' has no server node", cluster))?;

    let kubeconfig_content = provisioner.fetch_kubeconfig(server_vm).await?;

    // Fix the server address: lima copies kubeconfig with 127.0.0.1:<port>
    // which is already correct for local access since lima port-forwards automatically.
    let content = kubeconfig_content.replace("name: default", &format!("name: {}", cluster));
    let content = content.replace("cluster: default", &format!("cluster: {}", cluster));
    let content = content.replace("user: default", &format!("user: {}", cluster));
    let content = content.replace("context: default", &format!("context: {}", cluster));
    let content = content.replace(
        "current-context: default",
        &format!("current-context: {}", cluster),
    );

    let dest = match output {
        Some(path) => PathBuf::from(path),
        None => {
            let home = dirs::home_dir().context("Cannot determine home directory")?;
            home.join(format!("kubeconfig-{}.yaml", cluster))
        }
    };

    if merge {
        merge_kubeconfig(&content, &dest)?;
        println!(
            "{} Merged kubeconfig for '{}' into {:?}",
            "✓".green().bold(),
            cluster.yellow(),
            dest
        );
    } else {
        std::fs::write(&dest, &content)
            .with_context(|| format!("Failed to write kubeconfig to {:?}", dest))?;
        println!("{} Kubeconfig saved to {:?}", "✓".green().bold(), dest);
        println!("\n  export KUBECONFIG={}", dest.display());
    }

    Ok(())
}

fn merge_kubeconfig(new_content: &str, dest: &PathBuf) -> Result<()> {
    // Write the new kubeconfig to a temp file, then use `kubectl config view
    // --flatten` with KUBECONFIG=<existing>:<new> to produce a merged result.
    if !dest.exists() {
        std::fs::write(dest, new_content)?;
        return Ok(());
    }

    let tmp = std::env::temp_dir().join("kubelima-merge.yaml");
    std::fs::write(&tmp, new_content)?;

    let kubeconfig_env = format!("{}:{}", dest.display(), tmp.display());
    let output = std::process::Command::new("kubectl")
        .args(["config", "view", "--flatten"])
        .env("KUBECONFIG", &kubeconfig_env)
        .output();

    let _ = std::fs::remove_file(&tmp);

    match output {
        Ok(out) if out.status.success() => {
            std::fs::write(dest, &out.stdout)?;
        }
        _ => {
            // kubectl not available or failed — just overwrite.
            std::fs::write(dest, new_content)?;
        }
    }
    Ok(())
}
