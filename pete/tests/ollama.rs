use pete::ollama_psyche;

#[tokio::test]
async fn creates_psyche() {
    let psyche = ollama_psyche("http://localhost:11434", "mistral").unwrap();
    assert_eq!(psyche.conversation().lock().await.all().len(), 0);
}
