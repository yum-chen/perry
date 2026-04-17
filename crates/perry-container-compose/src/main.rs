use perry_container_compose::error::Result;
use perry_container_compose::cli;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    cli::run().await
}
