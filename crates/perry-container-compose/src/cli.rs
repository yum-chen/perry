//! CLI entry point for `perry-compose` binary.
//!
//! clap-based CLI with all subcommands.

use crate::compose::ComposeEngine;
use crate::error::Result;
use crate::project::ComposeProject;
use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

/// perry-compose: Docker Compose-like experience for Apple Container / Podman
#[derive(Parser, Debug)]
#[command(
    name = "perry-compose",
    version,
    about = "Docker Compose-like CLI for container backends, powered by Perry",
    long_about = None
)]
pub struct Cli {
    /// Path to compose file(s)
    #[arg(short = 'f', long = "file", value_name = "FILE", global = true)]
    pub files: Vec<PathBuf>,

    /// Project name (default: directory name)
    #[arg(short = 'p', long = "project-name", global = true)]
    pub project_name: Option<String>,

    /// Environment file(s)
    #[arg(long = "env-file", value_name = "FILE", global = true)]
    pub env_files: Vec<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start services
    Up(UpArgs),
    /// Stop and remove services
    Down(DownArgs),
    /// Start existing stopped services
    Start(ServiceArgs),
    /// Stop running services
    Stop(ServiceArgs),
    /// Restart services
    Restart(ServiceArgs),
    /// List service status
    Ps(PsArgs),
    /// View output from containers
    Logs(LogsArgs),
    /// Execute a command in a running service
    Exec(ExecArgs),
    /// Validate and view the Compose file
    Config(ConfigArgs),
}

#[derive(Args, Debug)]
pub struct UpArgs {
    #[arg(short = 'd', long = "detach")]
    pub detach: bool,
    #[arg(long = "build")]
    pub build: bool,
    #[arg(long = "remove-orphans")]
    pub remove_orphans: bool,
    pub services: Vec<String>,
}

#[derive(Args, Debug)]
pub struct DownArgs {
    #[arg(short = 'v', long = "volumes")]
    pub volumes: bool,
    #[arg(long = "remove-orphans")]
    pub remove_orphans: bool,
    pub services: Vec<String>,
}

#[derive(Args, Debug)]
pub struct ServiceArgs {
    pub services: Vec<String>,
}

#[derive(Args, Debug)]
pub struct PsArgs {
    #[arg(short = 'a', long = "all")]
    pub all: bool,
    pub services: Vec<String>,
}

#[derive(Args, Debug)]
pub struct LogsArgs {
    #[arg(short = 'f', long = "follow")]
    pub follow: bool,
    #[arg(long = "tail")]
    pub tail: Option<u32>,
    #[arg(short = 't', long = "timestamps")]
    pub timestamps: bool,
    pub services: Vec<String>,
}

#[derive(Args, Debug)]
pub struct ExecArgs {
    pub service: String,
    pub cmd: Vec<String>,
    #[arg(short = 'u', long = "user")]
    pub user: Option<String>,
    #[arg(short = 'w', long = "workdir")]
    pub workdir: Option<String>,
    #[arg(short = 'e', long = "env")]
    pub env: Vec<String>,
}

#[derive(Args, Debug)]
pub struct ConfigArgs {
    #[arg(long = "format", default_value = "yaml")]
    pub format: String,
    #[arg(long = "resolve-image-digests")]
    pub resolve: bool,
}

// ============ Command dispatch ============

pub async fn run(cli: Cli) -> Result<()> {
    let config = crate::config::ProjectConfig::new(
        cli.files.clone(),
        cli.project_name.clone(),
        cli.env_files.clone(),
    );
    let project = ComposeProject::load(&config)?;

    let backend_res = crate::backend::detect_backend().await;
    let backend = match backend_res {
        Ok(b) => std::sync::Arc::new(b) as std::sync::Arc<dyn crate::backend::ContainerBackend>,
        Err(probed) => return Err(crate::error::ComposeError::NoBackendFound { probed }),
    };

    let engine = ComposeEngine::new(project.spec.clone(), project.project_name.clone(), backend);

    match cli.command {
        Commands::Up(args) => {
            engine
                .up(&args.services, args.detach, args.build, args.remove_orphans)
                .await?;
        }

        Commands::Down(args) => {
            engine
                .down(&args.services, args.remove_orphans, args.volumes)
                .await?;
        }

        Commands::Start(args) => {
            engine.start(&args.services).await?;
        }

        Commands::Stop(args) => {
            engine.stop(&args.services).await?;
        }

        Commands::Restart(args) => {
            engine.restart(&args.services).await?;
        }

        Commands::Ps(_args) => {
            let infos = engine.ps().await?;
            print_ps_table(&infos);
        }

        Commands::Logs(args) => {
            let service = if args.services.is_empty() { None } else { Some(args.services[0].as_str()) };
            let logs = engine.logs(service, args.tail).await?;
            print!("{}", logs.stdout);
            eprint!("{}", logs.stderr);
        }

        Commands::Exec(args) => {
            let env: std::collections::HashMap<String, String> = args
                .env
                .iter()
                .filter_map(|e| {
                    let mut parts = e.splitn(2, '=');
                    let k = parts.next()?.to_owned();
                    let v = parts.next().unwrap_or("").to_owned();
                    Some((k, v))
                })
                .collect();

            let cmd = args.cmd.clone();
            let result = if !env.is_empty() || args.workdir.is_some() {
                // Use backend directly for workdir/env support
                let svc = engine
                    .spec
                    .services
                    .get(&args.service)
                    .ok_or_else(|| crate::error::ComposeError::NotFound(args.service.clone()))?;
                let container_name =
                    crate::service::service_container_name(svc, &args.service);

                engine
                    .backend
                    .exec(
                        &container_name,
                        &cmd,
                        if env.is_empty() { None } else { Some(&env) },
                        args.workdir.as_deref(),
                    )
                    .await?
            } else {
                engine.exec(&args.service, &cmd).await?
            };

            print!("{}", result.stdout);
            eprint!("{}", result.stderr);
        }

        Commands::Config(args) => {
            let yaml = engine.config()?;
            if args.format == "json" {
                let value: serde_yaml::Value = serde_yaml::from_str(&yaml)?;
                let json = serde_json::to_string_pretty(&value)?;
                println!("{}", json);
            } else {
                println!("{}", yaml);
            }
        }
    }

    Ok(())
}

fn print_ps_table(infos: &[crate::types::ContainerInfo]) {
    let col_w_svc = 24usize;
    let col_w_status = 12usize;
    let col_w_container = 36usize;

    println!(
        "{:<col_w_svc$}  {:<col_w_status$}  {:<col_w_container$}",
        "SERVICE", "STATUS", "CONTAINER",
        col_w_svc = col_w_svc,
        col_w_status = col_w_status,
        col_w_container = col_w_container,
    );
    println!(
        "{}",
        "-".repeat(col_w_svc + col_w_status + col_w_container + 4)
    );

    for info in infos {
        println!(
            "{:<col_w_svc$}  {:<col_w_status$}  {:<col_w_container$}",
            info.name,
            info.status,
            info.id,
            col_w_svc = col_w_svc,
            col_w_status = col_w_status,
            col_w_container = col_w_container,
        );
    }
}
