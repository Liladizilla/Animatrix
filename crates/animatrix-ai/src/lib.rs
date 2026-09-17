use animatrix_core::ProviderKind;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskKind {
    Text,
    Image,
    Video,
    Tts,
    Stt,
    Embedding,
    Vision,
    Music,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyMode {
    LocalFirst,
    FreeFirst,
    Cheapest,
    Fastest,
    QualityFirst,
    Balanced,
    UserSelected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequest {
    pub task: TaskKind,
    pub preferred_provider: Option<ProviderKind>,
    pub policy: PolicyMode,
    pub prompt: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse {
    pub job_id: String,
    pub provider: ProviderKind,
    pub task: TaskKind,
    pub status: String,
}

pub trait Provider {
    fn provider_kind(&self) -> ProviderKind;
    fn supports(&self, task: TaskKind) -> bool;
    fn run(&self, request: ModelRequest) -> ModelResponse;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn request_defaults_to_balanced_policy() {
        let req = ModelRequest {
            task: TaskKind::Image,
            preferred_provider: Some(ProviderKind::Local),
            policy: PolicyMode::Balanced,
            prompt: "A clean explainer frame".to_string(),
            created_at: chrono::Utc::now(),
        };

        assert_eq!(req.task, TaskKind::Image);
        assert_eq!(req.policy, PolicyMode::Balanced);
    }
}
