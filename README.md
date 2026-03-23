# kubelima

Provision and manage Kubernetes clusters backed by [Lima](https://lima-vm.io/) VMs.

> **Supported provisioners:** K3s (more distros planned)

## Prerequisites

- [Lima](https://lima-vm.io/) (`brew install lima`)
- [kubectl](https://kubernetes.io/docs/tasks/tools/) for cluster interaction

## Installation

```bash
cargo install --path .
```

## Usage

### Cluster

```bash
# Create a single-node k3s cluster (defaults: 2 CPUs, 4GiB RAM, 20GiB disk)
kubelima cluster create --name dev

# Create a 3-node cluster (1 server + 2 agents)
kubelima cluster create --name dev --nodes 3 --cpus 4 --memory 8 --disk 40

# List clusters
kubelima cluster list

# Show cluster details
kubelima cluster info --name dev

# Delete a cluster
kubelima cluster delete --name dev
```

### Kubeconfig

```bash
# Download kubeconfig
kubelima kubeconfig get --cluster dev

# Use the cluster
export KUBECONFIG=~/kubeconfig-dev.yaml
kubectl get nodes
```

### Nodes

```bash
# List nodes
kubelima node list --cluster dev

# Show node details
kubelima node info --cluster dev --node dev-server-0

# Add agent nodes to an existing cluster
kubelima node add --cluster dev --count 2

# Remove a node
kubelima node delete --cluster dev --node dev-agent-0

# SSH into a node
kubelima node ssh --cluster dev
kubelima node ssh --cluster dev --node dev-agent-0
```

### Mounts (Host → VM directory sharing)

Mounts share directories from the host into a VM over virtfs. The VM is
stopped and restarted automatically when a mount is added or removed.

```bash
# Mount a local directory into a node (read-write by default)
kubelima mount add --cluster dev --node dev-server-0 --local /tmp/data --remote /mnt/data

# Mount read-only
kubelima mount add --cluster dev --node dev-server-0 --local /tmp/data --remote /mnt/data --readonly

# List mounts
kubelima mount list --cluster dev

# Remove a mount
kubelima mount remove --cluster dev --node dev-server-0 --local /tmp/data
```

> **Multiple disks / block devices:** Lima does not expose raw block devices
> directly. To attach additional storage to a node, mount separate host
> directories as additional virtfs mounts and format/use them inside the VM
> as needed. For example, mount `/data/disk1` and `/data/disk2` on the host
> to `/mnt/disk1` and `/mnt/disk2` inside the VM — each mount appears as an
> independent filesystem the guest can use independently.

### Services

```bash
# Forward a cluster service to the host (runs in the foreground)
kubelima service export --cluster dev --service my-svc --local-port 8080

# List service exports
kubelima service list --cluster dev
```

### Provisioning scripts (cloud-init)

> **Note:** The `cloud-init` subcommand manages **shell scripts** that run
> inside the VM during node provisioning — not cloud-init YAML configurations.
> Think of them as post-boot setup scripts (e.g. installing packages, applying
> kernel settings) that are embedded into the lima VM template and executed
> once on first start.

```bash
# Register a provisioning script to run on every new node
kubelima cloud-init add --cluster dev --script ./setup.sh

# List registered scripts
kubelima cloud-init list --cluster dev

# Remove a script
kubelima cloud-init remove --cluster dev --script ./setup.sh
```

Scripts are re-applied when new nodes are added to the cluster. They run in
**system mode** (as root) inside the VM.

## State

Cluster state is persisted to `~/.kubelima/clusters/<name>/cluster.json`.

## Architecture

- **Provisioner**: extensible trait — currently only **K3s** is supported, backed by `limactl` and the official `template:k3s`
- **Multi-node networking**: `lima:user-v2` network with internal DNS (`lima-<vm>.internal`)
- **VM templates**: typed `VmTemplate` struct (serde) in the `lima` package, reusable across provisioners

## License

MIT