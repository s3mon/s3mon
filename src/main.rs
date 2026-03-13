use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    let action = s3mon::cli::start()?;
    s3mon::cli::actions::run::execute(&action).await?;
    Ok(())
}
