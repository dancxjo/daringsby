use anyhow::Result;
use log::info;

#[tokio::main]
async fn main() -> Result<()> {
    psyche::logging::init()?;
    info!("starting pete webserver");
    psyche::server::run(([127, 0, 0, 1], 8080)).await;
    Ok(())
}
