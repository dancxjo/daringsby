use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    psyche::server::run(([127, 0, 0, 1], 8080)).await;
    Ok(())
}
