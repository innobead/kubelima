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
# Create a single control-plane cluster (defaults: 2 CPUs, 4GiB RAM, 20GiB disk)
kubelima cluster create --name dev

# Create a cluster with 1 control-plane and 2 workers
kubelima cluster create --name dev --control-plane 1 --workers 2

# Create with custom resources
kubelima cluster create --name dev --control-plane 1 --workers 2 --cpus 4 --memory 8 --disk 40

# List clusters (always shown even when nodes are not yet running)
kubelima cluster list

# Show cluster details
kubelima cluster info --name dev

# Delete a cluster
kubelima cluster delete --name dev
```

### Kubeconfig

```bash
# Download kubeconfig to the current directory (./kubeconfig-dev.yaml)
kubelima kubeconfig get --cluster dev

# Download to a specific directory
kubelima kubeconfig get --cluster dev --output ~/.kube

# Use the cluster
export KUBECONFIG=./kubeconfig-dev.yaml
kubectl get nodes
```

### Nodes

Nodes have two roles: **control-plane** and **worker**.
VM names follow the pattern `<cluster>-cp-<n>` and `<cluster>-worker-<n>`.

```bash
# List nodes
kubelima node list --cluster dev

# Show node details
kubelima node info --cluster dev --node dev-cp-0

# Add worker nodes to an existing cluster
kubelima node add --cluster dev --workers 2

# Add control-plane nodes
kubelima node add --cluster dev --control-plane 1

# Add both at once
kubelima node add --cluster dev --control-plane 1 --workers 2

# Remove a node
kubelima node delete --cluster dev --node dev-worker-0

# SSH into a node (defaults to first control-plane)
kubelima node ssh --cluster dev
kubelima node ssh --cluster dev --node dev-worker-0
```

### Mounts (Host → VM directory sharing)

Mounts share directories from the host into a VM over virtfs. The VM is
stopped and restarted automatically when a mount is added or removed.

```bash
# Mount a local directory into a node (read-write by default)
kubelima mount add --cluster dev --node dev-cp-0 --local /tmp/data --remote /mnt/data

# Mount read-only
kubelima mount add --cluster dev --node dev-cp-0 --local /tmp/data --remote /mnt/data --readonly

# List mounts
kubelima mount list --cluster dev

# Remove a mount
kubelima mount remove --cluster dev --node dev-cp-0 --local /tmp/data
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