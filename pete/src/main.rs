use anyhow::Result;
use log::info;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    let bus = Arc::new(psyche::bus::EventBus::new());
    psyche::logging::init(bus.clone())?;
    info!("starting pete webserver");
    psyche::server::run(bus, ([127, 0, 0, 1], 8080)).await;
    Ok(())
}
