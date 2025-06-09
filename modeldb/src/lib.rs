//! Lightweight in-memory catalog describing available AI models.
//! See [`ModelRepository`] for usage.
use serde::{Deserialize, Serialize};

/// Basic information about an AI model.
///
/// The struct stores simple metadata that can be used to select an
/// appropriate model at runtime.
///
/// # Examples
///
/// ```
/// use modeldb::{AiModel, ModelRepository};
///
/// let mut repo = ModelRepository::new();
/// repo.add_model(AiModel {
///     name: "gpt4".to_string(),
///     supports_images: true,
///     speed: Some(1.0),
///     cost_per_token: Some(0.01),
/// });
/// assert!(repo.find("gpt4").is_some());
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiModel {
    /// Unique name of the model.
    pub name: String,
    /// Whether the model accepts image inputs.
    pub supports_images: bool,
    /// Relative speed factor of the model.
    pub speed: Option<f32>,
    /// Cost in dollars per generated token.
    pub cost_per_token: Option<f32>,
}

/// Collection of available [`AiModel`]s.
///
/// # Examples
///
/// ```
/// use modeldb::{AiModel, ModelRepository};
///
/// let mut repo = ModelRepository::new();
/// repo.add_model(AiModel {
///     name: "gpt4".into(),
///     supports_images: true,
///     speed: None,
///     cost_per_token: None,
/// });
/// let model = repo.find("gpt4").unwrap();
/// assert!(model.supports_images);
/// ```
pub struct ModelRepository {
    models: Vec<AiModel>,
}

impl ModelRepository {
    pub fn new() -> Self {
        Self { models: Vec::new() }
    }

    pub fn add_model(&mut self, model: AiModel) {
        self.models.push(model);
    }

    pub fn find(&self, name: &str) -> Option<&AiModel> {
        self.models.iter().find(|m| m.name == name)
    }
}

/// Return a [`ModelRepository`] preloaded with common models from the
/// [Ollama library](https://ollama.com).
///
/// The repository includes several freely available models with their basic
/// capabilities.
///
/// # Examples
///
/// ```
/// let repo = modeldb::ollama_models();
/// assert!(repo.find("gemma3").is_some());
/// ```
pub fn ollama_models() -> ModelRepository {
    let mut repo = ModelRepository::new();
    repo.add_model(AiModel {
        name: "gemma3".into(),
        supports_images: false,
        speed: None,
        cost_per_token: None,
    });
    repo.add_model(AiModel {
        name: "llama3:70b".into(),
        supports_images: false,
        speed: None,
        cost_per_token: None,
    });
    repo.add_model(AiModel {
        name: "phi3:mini".into(),
        supports_images: false,
        speed: None,
        cost_per_token: None,
    });
    repo.add_model(AiModel {
        name: "codellama:34b".into(),
        supports_images: false,
        speed: None,
        cost_per_token: None,
    });
    repo.add_model(AiModel {
        name: "llava:34b".into(),
        supports_images: true,
        speed: None,
        cost_per_token: None,
    });
    repo
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_and_find() {
        let mut repo = ModelRepository::new();
        repo.add_model(AiModel {
            name: "gpt4".into(),
            supports_images: true,
            speed: Some(1.0),
            cost_per_token: Some(0.01),
        });
        assert!(repo.find("gpt4").is_some());
    }

    #[test]
    fn default_ollama_models_present() {
        let repo = ollama_models();
        assert!(repo.find("gemma3").is_some());
    }
}
