//! CLI entry point for `perry-compose` binary.

use perry_container_compose::cli::run;
use tracing_subscriber::{fmt, EnvFilter};

#[tokio::main]
async fn main() {
    // Initialise tracing (RUST_LOG env controls verbosity)
    fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .init();

    if let Err(e) = run().await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
