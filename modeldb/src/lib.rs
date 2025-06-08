use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiModel {
    pub name: String,
    pub supports_images: bool,
    pub speed: Option<f32>,
    pub cost_per_token: Option<f32>,
}

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
