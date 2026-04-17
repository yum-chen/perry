use clap::{Parser, Subcommand};
use std::path::PathBuf;
use crate::config::ProjectConfig;
use crate::project::ComposeProject;
use crate::compose::ComposeEngine;
use crate::backend::detect_backend;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "perry-compose")]
#[command(version)]
pub struct Cli {
    #[arg(short, long)]
    pub file: Vec<PathBuf>,

    #[arg(short, long)]
    pub project_name: Option<String>,

    #[arg(long)]
    pub env_file: Vec<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
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

pub async fn run() -> crate::error::Result<()> {
    let cli = Cli::parse();

    let config = ProjectConfig {
        files: cli.file,
        project_name: cli.project_name,
        env_files: cli.env_file,
    };

    let project = ComposeProject::load(&config)?;
    let backend = detect_backend().await.map_err(|probed| {
        crate::error::ComposeError::NoBackendFound { probed }
    })?;

    let engine = Arc::new(ComposeEngine::new(project.spec, project.project_name, Arc::from(backend)));

    match cli.command {
        Commands::Up { detach, build, remove_orphans, services } => {
            engine.up(&services, detach, build, remove_orphans).await?;
        }
        Commands::Down { volumes, remove_orphans, services } => {
            engine.down(&services, remove_orphans, volumes).await?;
        }
        Commands::Ps { all: _, services } => {
            let infos = engine.ps().await?;
            for info in infos {
                if !services.is_empty() && !services.contains(&info.name) { continue; }
                println!("{:<20} {:<20} {:<20}", info.name, info.image, info.status);
            }
        }
        Commands::Logs { follow, tail, timestamps: _, services } => {
            let logs = engine.logs(&services, tail, follow).await?;
            for (svc, content) in logs {
                println!("=== {} ===\n{}", svc, content);
            }
        }
        Commands::Exec { service, cmd, env: _, workdir, user: _ } => {
            // TODO: parse env
            let res = engine.exec(&service, &cmd, None, workdir.as_deref()).await?;
            print!("{}", res.stdout);
            eprint!("{}", res.stderr);
        }
        Commands::Config { format: _, resolve_image_digests: _ } => {
            println!("{}", engine.config()?);
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
