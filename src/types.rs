use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeRole {
    #[serde(rename = "control-plane")]
    ControlPlane,
    #[serde(rename = "worker")]
    Worker,
}

impl std::fmt::Display for NodeRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeRole::ControlPlane => write!(f, "control-plane"),
            NodeRole::Worker => write!(f, "worker"),
        }
    }
}

impl std::str::FromStr for NodeRole {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "control-plane" | "controlplane" => Ok(NodeRole::ControlPlane),
            "worker" => Ok(NodeRole::Worker),
            _ => anyhow::bail!("Unknown node role '{}'. Valid: control-plane, worker", s),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeConfig {
    pub name: String,
    pub role: NodeRole,
    pub index: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountConfig {
    pub node: String,
    pub local: String,
    pub remote: String,
    pub writable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceExport {
    pub service: String,
    pub namespace: String,
    pub local_port: u16,
    pub remote_port: u16,
}

/// Persisted cluster state stored in ~/.kubelima/clusters/<name>/cluster.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterState {
    pub name: String,
    pub distro: String,
    pub cpus: u32,
    pub memory_gb: u32,
    pub disk_gb: u32,
    pub created_at: DateTime<Utc>,
    pub nodes: Vec<NodeConfig>,
    pub cloud_init_scripts: Vec<String>,
    pub mounts: Vec<MountConfig>,
    pub service_exports: Vec<ServiceExport>,
}

impl ClusterState {
    pub fn control_plane_nodes(&self) -> Vec<&NodeConfig> {
        self.nodes
            .iter()
            .filter(|n| n.role == NodeRole::ControlPlane)
            .collect()
    }

    pub fn control_plane_node(&self) -> Option<&NodeConfig> {
        self.nodes.iter().find(|n| n.role == NodeRole::ControlPlane)
    }

    pub fn control_plane_vm_name(&self) -> Option<&str> {
        self.control_plane_node().map(|n| n.name.as_str())
    }

    pub fn worker_nodes(&self) -> Vec<&NodeConfig> {
        self.nodes
            .iter()
            .filter(|n| n.role == NodeRole::Worker)
            .collect()
    }

    pub fn next_control_plane_index(&self) -> usize {
        self.control_plane_nodes()
            .iter()
            .map(|n| n.index + 1)
            .max()
            .unwrap_or(0)
    }

    pub fn next_worker_index(&self) -> usize {
        self.worker_nodes()
            .iter()
            .map(|n| n.index + 1)
            .max()
            .unwrap_or(0)
    }
}

/// Live VM info returned by limactl list --json
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LimaVm {
    pub name: String,
    pub status: String,
    #[serde(default)]
    pub network: Vec<LimaNetwork>,
    #[serde(rename = "sshLocalPort", default)]
    pub ssh_local_port: u16,
    #[serde(rename = "vmType", default)]
    pub vm_type: String,
    #[serde(default)]
    pub cpus: u32,
    #[serde(default)]
    pub memory: u64,
    #[serde(default)]
    pub disk: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LimaNetwork {
    pub interface: String,
    #[serde(rename = "localIPv4", default)]
    pub local_ipv4: String,
}

impl LimaVm {
    pub fn ip(&self) -> Option<&str> {
        self.network
            .iter()
            .find(|n| !n.local_ipv4.is_empty())
            .map(|n| n.local_ipv4.as_str())
    }

    pub fn memory_gb(&self) -> f64 {
        self.memory as f64 / 1024.0 / 1024.0 / 1024.0
    }

    pub fn disk_gb(&self) -> f64 {
        self.disk as f64 / 1024.0 / 1024.0 / 1024.0
    }
}
