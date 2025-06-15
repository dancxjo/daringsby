use psyche::Psyche;
use psyche::ling::OllamaProvider;
use std::process::Command;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _server = Command::new("ollama").arg("serve").spawn().ok();

    let narrator = OllamaProvider::new("http://localhost:11434", "mistral")?;
    let voice = OllamaProvider::new("http://localhost:11434", "mistral")?;
    let vectorizer = OllamaProvider::new("http://localhost:11434", "mistral")?;

    let psyche = Psyche::new(Box::new(narrator), Box::new(voice), Box::new(vectorizer));
    psyche.run();

    Ok(())
}
