#[tokio::main]
async fn main() {
    let graph = psyche::Neo4jClient::new(
        "bolt://localhost:7687".into(),
        "neo4j".into(),
        "password".into(),
    );
    match graph.latest_will_context().await {
        Ok(Some(ctx)) => println!("GOT: {:?}", ctx.system_prompt.len()),
        Ok(None) => println!("GOT: None"),
        Err(e) => println!("ERR: {:?}", e),
    }
}
