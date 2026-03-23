mod cli;
mod commands;
mod config;
mod lima;
mod provisioner;
mod types;

use anyhow::Result;
use clap::Parser;

use cli::{Cli, CloudInitCmd, ClusterCmd, Commands, KubeconfigCmd, MountCmd, NodeCmd, ServiceCmd};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Cluster { cmd } => match cmd {
            ClusterCmd::Create {
                name,
                nodes,
                distro,
                cpus,
                memory,
                disk,
                cloud_init,
            } => {
                commands::cluster::create(&name, nodes, &distro, cpus, memory, disk, &cloud_init)
                    .await?
            }
            ClusterCmd::List => commands::cluster::list().await?,
            ClusterCmd::Info { name } => commands::cluster::info(&name).await?,
            ClusterCmd::Delete { name, force } => commands::cluster::delete(&name, force).await?,
        },

        Commands::Node { cmd } => match cmd {
            NodeCmd::List { cluster } => commands::node::list(&cluster).await?,
            NodeCmd::Info { cluster, node } => commands::node::info(&cluster, &node).await?,
            NodeCmd::Add { cluster, count } => commands::node::add(&cluster, count).await?,
            NodeCmd::Delete {
                cluster,
                node,
                force,
            } => commands::node::delete(&cluster, &node, force).await?,
            NodeCmd::Ssh { cluster, node } => {
                commands::node::ssh(&cluster, node.as_deref()).await?
            }
        },

        Commands::Kubeconfig { cmd } => match cmd {
            KubeconfigCmd::Get {
                cluster,
                output,
                merge,
            } => commands::kubeconfig::get(&cluster, output.as_deref(), merge).await?,
        },

        Commands::Mount { cmd } => match cmd {
            MountCmd::Add {
                cluster,
                node,
                local,
                remote,
                readonly,
            } => commands::mount::add(&cluster, &node, &local, &remote, readonly).await?,
            MountCmd::List { cluster, node } => commands::mount::list(&cluster, node.as_deref())?,
            MountCmd::Remove {
                cluster,
                node,
                local,
            } => commands::mount::remove(&cluster, &node, &local).await?,
        },

        Commands::Service { cmd } => match cmd {
            ServiceCmd::Export {
                cluster,
                service,
                namespace,
                local_port,
                remote_port,
            } => {
                let remote = remote_port.unwrap_or(local_port);
                commands::service::export(&cluster, &service, &namespace, local_port, remote)
                    .await?
            }
            ServiceCmd::List { cluster } => commands::service::list(&cluster)?,
        },

        Commands::CloudInit { cmd } => match cmd {
            CloudInitCmd::Add { cluster, script } => commands::cloud_init::add(&cluster, &script)?,
            CloudInitCmd::List { cluster } => commands::cloud_init::list(&cluster)?,
            CloudInitCmd::Remove { cluster, script } => {
                commands::cloud_init::remove(&cluster, &script)?
            }
        },
    }

    Ok(())
}
