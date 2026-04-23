use clap::{Parser, Subcommand};
use std::path::PathBuf;
use crate::config::ProjectConfig;
use crate::project::ComposeProject;

#[derive(Parser)]
#[command(name = "perry-compose")]
#[command(about = "Native Rust reimplementation of container-compose", long_about = None)]
pub struct Cli {
    #[arg(short, long, value_name = "FILE")]
    pub file: Vec<PathBuf>,

    #[arg(short, long, value_name = "NAME")]
    pub project_name: Option<String>,

    #[arg(long, value_name = "FILE")]
    pub env_file: Vec<PathBuf>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start services
    Up {
        #[arg(short, long)]
        detach: bool,

        #[arg(long)]
        build: bool,

        #[arg(long)]
        remove_orphans: bool,

        services: Vec<String>,
    },
    /// Stop and remove services
    Down {
        #[arg(short, long)]
        volumes: bool,

        #[arg(long)]
        remove_orphans: bool,

        services: Vec<String>,
    },
    /// List service status
    Ps {
        #[arg(short, long)]
        all: bool,

        services: Vec<String>,
    },
    /// View output from containers
    Logs {
        #[arg(short, long)]
        follow: bool,

        #[arg(long)]
        tail: Option<u32>,

        #[arg(short, long)]
        timestamps: bool,

        services: Vec<String>,
    },
    /// Execute a command in a running service
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
    /// Validate and print resolved configuration
    Config {
        #[arg(long, default_value = "yaml")]
        format: String,

        #[arg(long)]
        resolve_image_digests: bool,
    },
    /// Start existing stopped services
    Start {
        services: Vec<String>,
    },
    /// Stop running services
    Stop {
        services: Vec<String>,
    },
    /// Restart services
    Restart {
        services: Vec<String>,
    },
}

pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let config = ProjectConfig {
        files: cli.file,
        project_name: cli.project_name,
        env_files: cli.env_file,
    };

    let project = ComposeProject::load(&config)?;
    let engine = project.engine();

    match cli.command {
        Commands::Up { .. } => {
            engine.up().await?;
        }
        Commands::Down { volumes, .. } => {
            engine.down(volumes).await?;
        }
        Commands::Ps { .. } => {
            let ps = engine.ps().await?;
            println!("{:<20} {:<20} {:<20} {:<20}", "NAME", "IMAGE", "STATUS", "PORTS");
            for info in ps {
                println!("{:<20} {:<20} {:<20} {:<20}", info.name, info.image, info.status, info.ports.join(", "));
            }
        }
        Commands::Logs { tail, services, .. } => {
            let svc = services.first().map(|s| s.as_str());
            let logs = engine.logs(svc, tail).await?;
            print!("{}", logs.stdout);
            eprint!("{}", logs.stderr);
        }
        Commands::Exec { service, cmd, .. } => {
            let logs = engine.exec(&service, &cmd).await?;
            print!("{}", logs.stdout);
            eprint!("{}", logs.stderr);
        }
        Commands::Config { .. } => {
            let yaml = serde_yaml::to_string(&engine.spec)?;
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
