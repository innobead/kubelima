use clap::{Parser, Subcommand};

/// K8s variant distro cluster provisioning and management via Lima VMs.
#[derive(Parser)]
#[command(
    name = "kubelima",
    about = "Provision and manage K8s clusters backed by Lima VMs",
    version,
    propagate_version = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Manage clusters
    Cluster {
        #[command(subcommand)]
        cmd: ClusterCmd,
    },
    /// Manage nodes within a cluster
    Node {
        #[command(subcommand)]
        cmd: NodeCmd,
    },
    /// Download or display cluster kubeconfig
    Kubeconfig {
        #[command(subcommand)]
        cmd: KubeconfigCmd,
    },
    /// Manage host↔node directory mounts
    Mount {
        #[command(subcommand)]
        cmd: MountCmd,
    },
    /// Export in-cluster services to the host
    Service {
        #[command(subcommand)]
        cmd: ServiceCmd,
    },
    /// Manage cloud-init scripts applied during node provisioning
    #[command(name = "cloud-init")]
    CloudInit {
        #[command(subcommand)]
        cmd: CloudInitCmd,
    },
}

// ── cluster ───────────────────────────────────────────────────────────────────

#[derive(Subcommand)]
pub enum ClusterCmd {
    /// Create a new cluster with control-plane and optional worker nodes
    Create {
        /// Cluster name
        #[arg(short, long)]
        name: String,

        /// Number of control-plane nodes
        #[arg(long, default_value = "1")]
        control_plane: u32,

        /// Number of worker nodes
        #[arg(long, default_value = "0")]
        workers: u32,

        /// K8s distribution to use
        #[arg(short, long, default_value = "k3s")]
        distro: String,

        /// vCPUs per node
        #[arg(long, default_value = "2")]
        cpus: u32,

        /// Memory per node in GiB
        #[arg(long, default_value = "4")]
        memory: u32,

        /// Disk per node in GiB
        #[arg(long, default_value = "20")]
        disk: u32,

        /// Cloud-init script paths to run on each node at provision time
        #[arg(long = "cloud-init", value_name = "SCRIPT")]
        cloud_init: Vec<String>,
    },

    /// List all known clusters
    List,

    /// Show detailed info about a cluster
    Info {
        /// Cluster name
        #[arg(short, long)]
        name: String,
    },

    /// Delete a cluster and all its VMs
    Delete {
        /// Cluster name
        #[arg(short, long)]
        name: String,

        /// Force delete even if VMs cannot be stopped cleanly
        #[arg(long)]
        force: bool,
    },
}

// ── node ──────────────────────────────────────────────────────────────────────

#[derive(Subcommand)]
pub enum NodeCmd {
    /// List nodes in a cluster
    List {
        /// Cluster name
        #[arg(short, long)]
        cluster: String,
    },

    /// Show detailed info about a specific node
    Info {
        /// Cluster name
        #[arg(short, long)]
        cluster: String,

        /// Node VM name (e.g. mycluster-cp-0, mycluster-worker-0)
        #[arg(short, long)]
        node: String,
    },

    /// Add control-plane or worker nodes to an existing cluster
    Add {
        /// Cluster name
        #[arg(short, long)]
        cluster: String,

        /// Number of control-plane nodes to add
        #[arg(long, default_value = "0")]
        control_plane: u32,

        /// Number of worker nodes to add
        #[arg(long, default_value = "1")]
        workers: u32,
    },

    /// Remove a node from a cluster
    Delete {
        /// Cluster name
        #[arg(short, long)]
        cluster: String,

        /// Node VM name to delete
        #[arg(short, long)]
        node: String,

        /// Force delete even if the VM cannot be stopped cleanly
        #[arg(long)]
        force: bool,
    },

    /// Open an interactive SSH session into a node
    Ssh {
        /// Cluster name
        #[arg(short, long)]
        cluster: String,

        /// Node VM name (defaults to the first control-plane node)
        #[arg(short, long)]
        node: Option<String>,
    },
}

// ── kubeconfig ────────────────────────────────────────────────────────────────

#[derive(Subcommand)]
pub enum KubeconfigCmd {
    /// Download (or locate) the kubeconfig for a cluster
    Get {
        /// Cluster name
        #[arg(short, long)]
        cluster: String,

        /// Directory to save kubeconfig into (default: current directory)
        #[arg(short, long)]
        output: Option<String>,

        /// Merge into the output file instead of overwriting
        #[arg(long)]
        merge: bool,
    },
}

// ── mount ─────────────────────────────────────────────────────────────────────

#[derive(Subcommand)]
pub enum MountCmd {
    /// Mount a local directory into a node (stops/restarts the VM)
    Add {
        /// Cluster name
        #[arg(short, long)]
        cluster: String,

        /// Node VM name
        #[arg(short, long)]
        node: String,

        /// Local directory to mount
        #[arg(long)]
        local: String,

        /// Mount point inside the VM
        #[arg(long)]
        remote: String,

        /// Mount read-only (default is read-write / bi-directional)
        #[arg(long)]
        readonly: bool,
    },

    /// List configured mounts for a cluster
    List {
        /// Cluster name
        #[arg(short, long)]
        cluster: String,

        /// Filter by node name
        #[arg(short, long)]
        node: Option<String>,
    },

    /// Remove a mount from a node (stops/restarts the VM)
    Remove {
        /// Cluster name
        #[arg(short, long)]
        cluster: String,

        /// Node VM name
        #[arg(short, long)]
        node: String,

        /// Local directory path of the mount to remove
        #[arg(long)]
        local: String,
    },
}

// ── service ───────────────────────────────────────────────────────────────────

#[derive(Subcommand)]
pub enum ServiceCmd {
    /// Forward a cluster service to a local port (runs in the foreground)
    Export {
        /// Cluster name
        #[arg(short, long)]
        cluster: String,

        /// Kubernetes service name
        #[arg(short, long)]
        service: String,

        /// Kubernetes namespace
        #[arg(short, long, default_value = "default")]
        namespace: String,

        /// Local port to listen on
        #[arg(long)]
        local_port: u16,

        /// Remote service port (defaults to local-port)
        #[arg(long)]
        remote_port: Option<u16>,
    },

    /// List service exports recorded for a cluster
    List {
        /// Cluster name
        #[arg(short, long)]
        cluster: String,
    },
}

// ── cloud-init ────────────────────────────────────────────────────────────────

#[derive(Subcommand)]
pub enum CloudInitCmd {
    /// Register a cloud-init script for future node provisioning
    Add {
        /// Cluster name
        #[arg(short, long)]
        cluster: String,

        /// Path to the shell script to run during node provisioning
        #[arg(short, long)]
        script: String,
    },

    /// List registered cloud-init scripts for a cluster
    List {
        /// Cluster name
        #[arg(short, long)]
        cluster: String,
    },

    /// Remove a registered cloud-init script from a cluster
    Remove {
        /// Cluster name
        #[arg(short, long)]
        cluster: String,

        /// Script path or filename to remove
        #[arg(short, long)]
        script: String,
    },
}
