use pete::ollama_psyche;

#[tokio::test]
async fn creates_psyche() {
    let psyche = ollama_psyche(
        "http://localhost:11434",
        "gemma3",
        "http://localhost:11434",
        "gemma3",
        "http://localhost:11434",
        "embeddinggemma",
        "http://localhost:6333",
        "bolt://localhost:7687",
        "neo4j",
        "password",
    )
    .unwrap();
    assert_eq!(psyche.conversation().lock().await.all().len(), 0);
}
