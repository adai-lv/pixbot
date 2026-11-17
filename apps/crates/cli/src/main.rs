//! # Pixbot CLI - Command Line Interface

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    cli::cmd::exec().await
}
