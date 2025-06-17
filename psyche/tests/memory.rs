use psyche::{Event, Memory};
use tokio::sync::broadcast;

#[tokio::test]
async fn emits_cypher_statement() {
    let (tx, mut rx) = broadcast::channel(8);
    let mut mem = Memory::new(tx);
    mem.feel("Pete met Travis.");
    mem.consult().await.unwrap();
    let evt = rx.try_recv().unwrap();
    match evt {
        Event::StreamChunk(cypher) => assert!(cypher.contains("MERGE")),
        other => panic!("unexpected event {other:?}"),
    }
}
