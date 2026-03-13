mod cli;
mod config;
mod s3;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let action = cli::start()?;
    cli::actions::run::execute(&action).await?;
    Ok(())
}
