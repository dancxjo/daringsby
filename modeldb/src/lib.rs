use serde::{Deserialize, Serialize};

/// Metadata describing an available language model.
///
/// ```
/// use modeldb::{AiModel, ModelRepository};
/// let mut repo = ModelRepository::new();
/// repo.add_model(AiModel {
///     name: "gpt4".into(),
///     supports_images: true,
///     speed: Some(1.0),
///     cost_per_token: Some(0.02),
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

/// Simple in-memory collection of [`AiModel`]s.
///
/// ```
/// use modeldb::{AiModel, ModelRepository};
/// let mut repo = ModelRepository::new();
/// repo.add_model(AiModel {
///     name: "foo".into(),
///     supports_images: false,
///     speed: None,
///     cost_per_token: None,
/// });
/// assert!(repo.find("foo").is_some());
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
}
