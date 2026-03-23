use anyhow::Result;
use async_trait::async_trait;
use std::path::PathBuf;

pub mod k3s;

/// Context for provisioning a single VM node.
pub struct ProvisionContext {
    /// Lima VM name for this node.
    pub vm_name: String,
    /// Number of vCPUs.
    pub cpus: u32,
    /// Memory in GiB.
    pub memory_gb: u32,
    /// Disk size in GiB.
    pub disk_gb: u32,
    /// Paths to cloud-init scripts to embed as provision steps.
    pub cloud_init_scripts: Vec<String>,
}

/// Implemented by each K8s distro provisioner.
#[async_trait]
pub trait Provisioner: Send + Sync {
    /// Human-readable distro name (e.g. "k3s").
    #[allow(dead_code)]
    fn distro_name(&self) -> &str;

    /// Provision the first server/control-plane VM.
    async fn create_server(&self, ctx: &ProvisionContext) -> Result<()>;

    /// Provision an agent VM that joins an existing server.
    async fn create_agent(&self, ctx: &ProvisionContext, server_vm: &str) -> Result<()>;

    /// Return the path to the kubeconfig file for this cluster (on the host).
    fn kubeconfig_path(&self, server_vm: &str) -> Result<PathBuf>;

    /// Fetch the raw kubeconfig YAML content from the server VM.
    async fn fetch_kubeconfig(&self, server_vm: &str) -> Result<String>;
}

/// Return a boxed provisioner for the given distro identifier.
pub fn get_provisioner(distro: &str) -> Result<Box<dyn Provisioner>> {
    match distro {
        "k3s" => Ok(Box::new(k3s::K3sProvisioner)),
        other => anyhow::bail!("Unknown distro '{}'. Supported distros: k3s", other),
    }
}

/// List all supported distro names.
#[allow(dead_code)]
pub fn supported_distros() -> &'static [&'static str] {
    &["k3s"]
}
