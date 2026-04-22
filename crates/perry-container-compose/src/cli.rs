use crate::error::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(name = "perry-compose")]
#[command(version = "0.5.58")]
pub struct Cli {
    #[arg(short, long, action = clap::ArgAction::Append)]
    pub file: Vec<PathBuf>,

    #[arg(short, long)]
    pub project_name: Option<String>,

    #[arg(long, action = clap::ArgAction::Append)]
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
        #[arg(trailing_var_arg = true)]
        cmd: Vec<String>,
        #[arg(short, long, action = clap::ArgAction::Append)]
        env: Vec<String>,
        #[arg(short, long)]
        workdir: Option<PathBuf>,
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

pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    // 1. Resolve project configuration
    let project_dir = std::env::current_dir()?;
    let env = crate::yaml::load_env(&project_dir, &cli.env_file);

    let compose_files = crate::config::resolve_compose_files(&project_dir, cli.file.clone());

    let spec = crate::yaml::parse_and_merge_files(&compose_files, &env)?;
    let project_name = crate::config::resolve_project_name(&project_dir, cli.project_name.clone());

    // 2. Detect backend
    let backend = crate::backend::detect_backend().await
        .map_err(|e| crate::error::ComposeError::NoBackendFound { probed: e })?;
    let backend_shared = Arc::from(backend);

    // 3. Create engine and execute command
    let engine = crate::compose::ComposeEngine::new(spec, project_name, backend_shared);

    match cli.command {
        Commands::Up { detach, build, remove_orphans, services } => {
            engine.up(&services, detach, build, remove_orphans).await?;
        }
        Commands::Down { volumes, .. } => {
            engine.down(volumes).await?;
        }
        Commands::Ps { .. } => {
            let infos = engine.ps().await?;
            println!("{:<20} {:<20} {:<20} {:<20}", "NAME", "IMAGE", "STATUS", "PORTS");
            for info in infos {
                println!("{:<20} {:<20} {:<20} {:<20?}", info.name, info.image, info.status, info.ports);
            }
        }
        Commands::Logs { services, tail, .. } => {
            let svc = services.first().map(|s| s.as_str());
            let logs = engine.logs(svc, tail).await?;
            println!("STDOUT:\n{}", logs.stdout);
            println!("STDERR:\n{}", logs.stderr);
        }
        Commands::Exec { service, cmd, .. } => {
            let res = engine.exec(&service, &cmd).await?;
            print!("{}", res.stdout);
            eprint!("{}", res.stderr);
        }
        Commands::Config { .. } => {
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
