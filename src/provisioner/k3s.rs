use anyhow::{Context, Result};
use async_trait::async_trait;
use std::path::PathBuf;

use crate::lima::{LimaClient, VmTemplate};

use super::{ProvisionContext, Provisioner};

/// Network used for inter-VM communication in multi-node clusters.
const LIMA_NETWORK: &str = "lima:user-v2";

pub struct K3sProvisioner;

impl K3sProvisioner {
    /// Build a VmTemplate that extends template:k3s with resource sizing,
    /// optional k3s params (url/token for agents), and cloud-init scripts.
    fn build_vm_template(
        ctx: &ProvisionContext,
        k3s_url: Option<&str>,
        k3s_token: Option<&str>,
    ) -> Result<VmTemplate> {
        let mut tpl = VmTemplate::new("template:k3s", ctx.cpus, ctx.memory_gb, ctx.disk_gb);

        // k3s params (empty strings use the template defaults, i.e. server mode).
        tpl.set_param("url", k3s_url.unwrap_or(""));
        tpl.set_param("token", k3s_token.unwrap_or(""));

        // Extra provision steps from cloud-init scripts.
        for script_path in &ctx.cloud_init_scripts {
            let script = std::fs::read_to_string(script_path)
                .with_context(|| format!("Cannot read cloud-init script '{}'", script_path))?;
            tpl.add_provision_script(script);
        }

        Ok(tpl)
    }

    /// Fetch the node-token from an already-running k3s control-plane VM.
    pub async fn fetch_token(control_plane_vm: &str) -> Result<String> {
        LimaClient::run_in_vm(
            control_plane_vm,
            "sudo cat /var/lib/rancher/k3s/server/node-token",
        )
        .await
        .context("Failed to retrieve k3s node token from control-plane")
    }

    /// Build the internal DNS name for a VM within the lima:user-v2 network.
    pub fn internal_url(control_plane_vm: &str) -> String {
        format!("https://lima-{}.internal:6443", control_plane_vm)
    }
}

#[async_trait]
impl Provisioner for K3sProvisioner {
    fn distro_name(&self) -> &str {
        "k3s"
    }

    async fn create_control_plane(&self, ctx: &ProvisionContext) -> Result<()> {
        if ctx.cloud_init_scripts.is_empty() {
            // Fast path: use the stock template with --set for resource sizing.
            let set_expr = format!(
                ".cpus={} | .memory=\"{}GiB\" | .disk=\"{}GiB\"",
                ctx.cpus, ctx.memory_gb, ctx.disk_gb
            );
            LimaClient::start_vm(
                &ctx.vm_name,
                "template:k3s",
                Some(&set_expr),
                &[&format!("--network={}", LIMA_NETWORK)],
            )
            .await
        } else {
            // Generate a custom YAML to incorporate cloud-init scripts and sizes.
            let tmp = Self::build_vm_template(ctx, None, None)?.write_temp(&ctx.vm_name)?;
            let result = LimaClient::start_vm(
                &ctx.vm_name,
                &tmp.to_string_lossy(),
                None,
                &[&format!("--network={}", LIMA_NETWORK)],
            )
            .await;
            let _ = std::fs::remove_file(&tmp);
            result
        }
    }

    async fn create_worker(&self, ctx: &ProvisionContext, control_plane_vm: &str) -> Result<()> {
        let token = Self::fetch_token(control_plane_vm).await?;
        let url = Self::internal_url(control_plane_vm);

        let tmp =
            Self::build_vm_template(ctx, Some(&url), Some(&token))?.write_temp(&ctx.vm_name)?;
        let result = LimaClient::start_vm(
            &ctx.vm_name,
            &tmp.to_string_lossy(),
            None,
            &[&format!("--network={}", LIMA_NETWORK)],
        )
        .await;
        let _ = std::fs::remove_file(&tmp);
        result
    }

    fn kubeconfig_path(&self, control_plane_vm: &str) -> Result<PathBuf> {
        let home = dirs::home_dir().context("Cannot determine home directory")?;
        Ok(home
            .join(".lima")
            .join(control_plane_vm)
            .join("copied-from-guest")
            .join("kubeconfig.yaml"))
    }

    async fn fetch_kubeconfig(&self, control_plane_vm: &str) -> Result<String> {
        let path = self.kubeconfig_path(control_plane_vm)?;
        std::fs::read_to_string(&path).with_context(|| {
            format!(
                "Kubeconfig not found at {:?}. Is the cluster running?",
                path
            )
        })
    }
}
