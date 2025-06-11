use anyhow::Context;
use modeldb::{AiModel, ModelRepository};

/// Attributes describing an Ollama server.
#[derive(Debug, Clone, Default)]
pub struct OllamaServer {
    pub client: ollama_rs::Ollama,
    /// Whether the server runs on the local machine.
    pub local: bool,
    /// Whether the server is considered fast relative to others.
    pub fast: bool,
    /// Whether usage is free of charge.
    pub free: bool,
}

impl OllamaServer {
    /// Create a new server description.
    pub fn new(client: ollama_rs::Ollama, local: bool, fast: bool, free: bool) -> Self {
        Self {
            client,
            local,
            fast,
            free,
        }
    }

    /// Retrieve the list of installed model names.
    pub async fn list_models(&self) -> anyhow::Result<Vec<String>> {
        let models = self
            .client
            .list_local_models()
            .await
            .context("failed to query models from server")?;
        Ok(models.into_iter().map(|m| m.name).collect())
    }

    /// Ensure `model` is available on this server, pulling if necessary.
    pub async fn pull_model(&self, model: &str) -> anyhow::Result<()> {
        crate::ensure_model_with_client(&self.client, model).await
    }

    /// Convert the list of names from [`list_models`] into [`AiModel`]s using
    /// `repo` for metadata lookup.
    pub async fn models(&self, repo: &ModelRepository) -> anyhow::Result<Vec<AiModel>> {
        let names = self.list_models().await?;
        Ok(names
            .into_iter()
            .filter_map(|n| repo.find(&n).cloned())
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use warp::Filter;

    #[tokio::test]
    async fn lists_models() {
        let tags = warp::path("api").and(warp::path("tags")).map(|| {
            warp::reply::json(&serde_json::json!({
                "models": [{"name": "gemma3", "modified_at":"0", "size":0}]
            }))
        });
        let (addr, server) = warp::serve(tags).bind_ephemeral(([127, 0, 0, 1], 0));
        tokio::task::spawn(server);

        let client = ollama_rs::Ollama::new(format!("http://{}", addr.ip()), addr.port());
        let server = OllamaServer::new(client, true, true, true);
        let repo = modeldb::ollama_models();
        let models = server.models(&repo).await.unwrap();
        assert!(models.iter().any(|m| m.name == "gemma3"));
    }
}
