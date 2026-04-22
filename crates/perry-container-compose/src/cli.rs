use clap::{Parser, Subcommand};
use std::path::PathBuf;
use crate::project::ComposeProject;
use crate::config::ProjectConfig;
use crate::compose::ComposeEngine;
use crate::backend::detect_backend;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "perry-compose")]
#[command(version)]
pub struct Cli {
    #[arg(short, long, global = true)]
    pub file: Vec<PathBuf>,

    #[arg(short, long, global = true)]
    pub project_name: Option<String>,

    #[arg(long, global = true)]
    pub env_file: Vec<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Clone)]
pub enum Commands {
    Up {
        #[arg(short, long)]
        detach: bool,
        #[arg(long)]
        build: bool,
        #[arg(long)]
        remove_orphans: bool,
        services: Vec<String>,
    },
    Down {
        #[arg(short, long)]
        volumes: bool,
        #[arg(long)]
        remove_orphans: bool,
        services: Vec<String>,
    },
    Ps {
        #[arg(short, long)]
        all: bool,
        services: Vec<String>,
    },
    Logs {
        #[arg(short, long)]
        follow: bool,
        #[arg(long)]
        tail: Option<u32>,
        #[arg(short, long)]
        timestamps: bool,
        services: Vec<String>,
    },
    Exec {
        service: String,
        cmd: Vec<String>,
        #[arg(short, long)]
        env: Vec<String>,
        #[arg(short, long)]
        workdir: Option<String>,
        #[arg(short, long)]
        user: Option<String>,
    },
    Config {
        #[arg(long, default_value = "yaml")]
        format: String,
        #[arg(long)]
        resolve_image_digests: bool,
    },
    Start {
        services: Vec<String>,
    },
    Stop {
        services: Vec<String>,
    },
    Restart {
        services: Vec<String>,
    },
}

pub async fn run(args: Cli) -> anyhow::Result<()> {
    let config = ProjectConfig {
        files: args.file,
        project_name: args.project_name,
        env_files: args.env_file,
    };

    let project = ComposeProject::load(&config)?;
    let backend_boxed = detect_backend().await.map_err(|e| {
        anyhow::anyhow!("No container backend found: {:?}", e)
    })?;

    let backend: Arc<dyn crate::backend::ContainerBackend + Send + Sync> = Arc::from(backend_boxed);

    let engine = ComposeEngine::new(project.spec.clone(), project.project_name.clone(), backend);

    match args.command {
        Commands::Up { detach, build, remove_orphans, services } => {
            engine.up(&services, detach, build, remove_orphans).await?;
        }
        Commands::Down { volumes, remove_orphans, services: _ } => {
            engine.down(volumes, remove_orphans).await?;
        }
        Commands::Ps { all: _, services: _ } => {
            let info = engine.ps().await?;
            for c in info {
                println!("{} {} {} {}", c.id, c.name, c.image, c.status);
            }
        }
        Commands::Logs { follow: _, tail, timestamps: _, services } => {
            if services.is_empty() {
                let logs = engine.logs(None, tail).await?;
                print!("{}", logs.stdout);
                eprint!("{}", logs.stderr);
            } else {
                for svc in services {
                    let logs = engine.logs(Some(&svc), tail).await?;
                    print!("{}", logs.stdout);
                    eprint!("{}", logs.stderr);
                }
            }
        }
        Commands::Exec { service, cmd, env: _, workdir: _, user: _ } => {
            let logs = engine.exec(&service, &cmd).await?;
            print!("{}", logs.stdout);
            eprint!("{}", logs.stderr);
        }
        Commands::Config { format: _, resolve_image_digests: _ } => {
            let yaml = engine.config()?;
            println!("{}", yaml);
        }
        Commands::Start { services } => {
            engine.start(&services).await?;
        }
        Commands::Stop { services } => {
            engine.stop(&services).await?;
        }
        Commands::Restart { services } => {
            engine.restart(&services).await?;
        }
    }

    Ok(())
}
