pub mod template;
pub use template::VmTemplate;

use anyhow::{Context, Result, bail};
use std::path::Path;
use tokio::process::Command;

use crate::types::LimaVm;

pub struct LimaClient;

impl LimaClient {
    /// List all VMs known to limactl.
    pub async fn list_vms() -> Result<Vec<LimaVm>> {
        let output = Command::new("limactl")
            .args(["list", "--json"])
            .output()
            .await
            .context("Failed to run limactl. Is lima installed? (brew install lima)")?;

        if !output.status.success() {
            bail!(
                "limactl list failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|l| serde_json::from_str::<LimaVm>(l).context("Failed to parse limactl JSON"))
            .collect()
    }

    /// Get a single VM by name, or None if it doesn't exist.
    pub async fn get_vm(name: &str) -> Result<Option<LimaVm>> {
        let vms = Self::list_vms().await?;
        Ok(vms.into_iter().find(|v| v.name == name))
    }

    /// Start a VM from a template or YAML file path, with optional --set expressions and extra flags.
    pub async fn start_vm(
        name: &str,
        template_or_path: &str,
        set_expr: Option<&str>,
        extra_flags: &[&str],
    ) -> Result<()> {
        let mut cmd = Command::new("limactl");
        cmd.args(["start", "--name", name, "--tty=false"]);
        for flag in extra_flags {
            cmd.arg(flag);
        }
        if let Some(expr) = set_expr {
            cmd.args(["--set", expr]);
        }
        cmd.arg(template_or_path);

        let status = cmd.status().await.context("Failed to run limactl start")?;

        if !status.success() {
            bail!("Failed to start VM '{}'", name);
        }
        Ok(())
    }

    /// Stop a running VM.
    pub async fn stop_vm(name: &str) -> Result<()> {
        let status = Command::new("limactl")
            .args(["stop", name])
            .status()
            .await
            .context("Failed to run limactl stop")?;

        if !status.success() {
            bail!("Failed to stop VM '{}'", name);
        }
        Ok(())
    }

    /// Delete a VM, optionally with --force.
    pub async fn delete_vm(name: &str, force: bool) -> Result<()> {
        let mut args = vec!["delete", name];
        if force {
            args.push("--force");
        }
        let status = Command::new("limactl")
            .args(&args)
            .status()
            .await
            .context("Failed to run limactl delete")?;

        if !status.success() {
            bail!("Failed to delete VM '{}'", name);
        }
        Ok(())
    }

    /// Run a shell command inside a VM and capture stdout.
    pub async fn run_in_vm(vm: &str, command: &str) -> Result<String> {
        let output = Command::new("limactl")
            .args(["shell", vm, "--", "sh", "-c", command])
            .output()
            .await
            .with_context(|| format!("Failed to run command in VM '{}'", vm))?;

        if !output.status.success() {
            bail!(
                "Command failed in VM '{}': {}",
                vm,
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Copy a file from a VM guest to a local path.
    #[allow(dead_code)]
    pub async fn copy_from_vm(vm: &str, remote_path: &str, local_path: &Path) -> Result<()> {
        let src = format!("{}:{}", vm, remote_path);
        let dst = local_path.to_string_lossy();
        let status = Command::new("limactl")
            .args(["copy", &src, &dst])
            .status()
            .await
            .context("Failed to run limactl copy")?;

        if !status.success() {
            bail!("Failed to copy '{}' from VM '{}'", remote_path, vm);
        }
        Ok(())
    }

    /// Open an interactive SSH shell into a VM.
    pub async fn ssh_interactive(vm: &str) -> Result<()> {
        let status = Command::new("limactl")
            .args(["shell", vm])
            .status()
            .await
            .with_context(|| format!("Failed to start SSH session to '{}'", vm))?;

        if !status.success() {
            bail!("SSH session exited with error for VM '{}'", vm);
        }
        Ok(())
    }

    /// Restart an existing VM (stop + start without re-specifying a template).
    pub async fn restart_vm(name: &str) -> Result<()> {
        Self::stop_vm(name).await?;
        let status = Command::new("limactl")
            .args(["start", name])
            .status()
            .await
            .context("Failed to run limactl start")?;
        if !status.success() {
            bail!("Failed to restart VM '{}'", name);
        }
        Ok(())
    }

    /// Edit the lima YAML config for a VM using serde_yaml.
    pub async fn edit_vm_yaml<F>(vm: &str, f: F) -> Result<()>
    where
        F: FnOnce(&mut serde_yaml::Value) -> Result<()>,
    {
        let home = dirs::home_dir().context("Cannot determine home directory")?;
        let config_path = home.join(".lima").join(vm).join("lima.yaml");

        let content = std::fs::read_to_string(&config_path)
            .with_context(|| format!("Cannot read lima config for VM '{}'", vm))?;

        let mut config: serde_yaml::Value =
            serde_yaml::from_str(&content).context("Failed to parse lima YAML config")?;

        f(&mut config)?;

        let new_content =
            serde_yaml::to_string(&config).context("Failed to serialize lima YAML config")?;
        std::fs::write(&config_path, new_content)
            .with_context(|| format!("Failed to write lima config for VM '{}'", vm))?;

        Ok(())
    }
}
