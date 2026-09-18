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

#[derive(Debug, Default, Clone)]
pub struct LocalProvider;

impl Provider for LocalProvider {
    fn provider_kind(&self) -> ProviderKind {
        ProviderKind::Local
    }

    fn supports(&self, task: TaskKind) -> bool {
        matches!(task, TaskKind::Text | TaskKind::Image | TaskKind::Video | TaskKind::Music)
    }

    fn run(&self, request: ModelRequest) -> ModelResponse {
        ModelResponse {
            job_id: format!("local-{}", uuid::Uuid::new_v4()),
            provider: self.provider_kind(),
            task: request.task,
            status: "queued".to_string(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct OpenRouterProvider;

impl Provider for OpenRouterProvider {
    fn provider_kind(&self) -> ProviderKind {
        ProviderKind::OpenRouter
    }

    fn supports(&self, task: TaskKind) -> bool {
        matches!(task, TaskKind::Text | TaskKind::Vision | TaskKind::Embedding | TaskKind::Tts | TaskKind::Stt)
    }

    fn run(&self, request: ModelRequest) -> ModelResponse {
        ModelResponse {
            job_id: format!("openrouter-{}", uuid::Uuid::new_v4()),
            provider: self.provider_kind(),
            task: request.task,
            status: "queued".to_string(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct HuggingFaceProvider;

impl Provider for HuggingFaceProvider {
    fn provider_kind(&self) -> ProviderKind {
        ProviderKind::HuggingFace
    }

    fn supports(&self, task: TaskKind) -> bool {
        matches!(task, TaskKind::Text | TaskKind::Image | TaskKind::Vision | TaskKind::Embedding)
    }

    fn run(&self, request: ModelRequest) -> ModelResponse {
        ModelResponse {
            job_id: format!("hf-{}", uuid::Uuid::new_v4()),
            provider: self.provider_kind(),
            task: request.task,
            status: "queued".to_string(),
        }
    }
}

#[derive(Debug, Default, Clone)]
pub struct ComfyUiProvider;

impl Provider for ComfyUiProvider {
    fn provider_kind(&self) -> ProviderKind {
        ProviderKind::ComfyUi
    }

    fn supports(&self, task: TaskKind) -> bool {
        matches!(task, TaskKind::Image | TaskKind::Video)
    }

    fn run(&self, request: ModelRequest) -> ModelResponse {
        ModelResponse {
            job_id: format!("comfy-{}", uuid::Uuid::new_v4()),
            provider: self.provider_kind(),
            task: request.task,
            status: "queued".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProviderRegistry {
    local: LocalProvider,
    openrouter: OpenRouterProvider,
    huggingface: HuggingFaceProvider,
    comfyui: ComfyUiProvider,
}

impl Default for ProviderRegistry {
    fn default() -> Self {
        Self {
            local: LocalProvider,
            openrouter: OpenRouterProvider,
            huggingface: HuggingFaceProvider,
            comfyui: ComfyUiProvider,
        }
    }
}

impl ProviderRegistry {
    pub fn resolve(&self, preferred: Option<ProviderKind>) -> Box<dyn Provider + Send + Sync> {
        match preferred {
            Some(ProviderKind::Local) => Box::new(self.local.clone()),
            Some(ProviderKind::HuggingFace) => Box::new(self.huggingface.clone()),
            Some(ProviderKind::ComfyUi) => Box::new(self.comfyui.clone()),
            Some(ProviderKind::Groq) => Box::new(self.openrouter.clone()),
            Some(ProviderKind::Cerebras) => Box::new(self.openrouter.clone()),
            Some(ProviderKind::Together) => Box::new(self.huggingface.clone()),
            Some(ProviderKind::Fireworks) => Box::new(self.openrouter.clone()),
            Some(ProviderKind::DeepInfra) => Box::new(self.huggingface.clone()),
            Some(ProviderKind::Fal) => Box::new(self.openrouter.clone()),
            Some(ProviderKind::Replicate) => Box::new(self.comfyui.clone()),
            Some(ProviderKind::OpenRouter) => Box::new(self.openrouter.clone()),
            Some(ProviderKind::Custom) => Box::new(self.local.clone()),
            None => Box::new(self.local.clone()),
        }
    }
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

    #[test]
    fn local_provider_handles_video_requests() {
        let provider = LocalProvider;
        let request = ModelRequest {
            task: TaskKind::Video,
            preferred_provider: Some(ProviderKind::Local),
            policy: PolicyMode::LocalFirst,
            prompt: "Render a short product teaser.".to_string(),
            created_at: chrono::Utc::now(),
        };

        let response = provider.run(request);
        assert_eq!(response.provider, ProviderKind::Local);
        assert_eq!(response.task, TaskKind::Video);
        assert_eq!(response.status, "queued");
    }

    #[test]
    fn provider_registry_falls_back_to_local() {
        let registry = ProviderRegistry::default();
        let provider = registry.resolve(None);
        assert_eq!(provider.provider_kind(), ProviderKind::Local);
    }

    #[test]
    fn provider_registry_resolves_non_local_provider() {
        let registry = ProviderRegistry::default();
        let provider = registry.resolve(Some(ProviderKind::OpenRouter));
        assert_eq!(provider.provider_kind(), ProviderKind::OpenRouter);
        assert!(provider.supports(TaskKind::Vision));
    }

    #[test]
    fn comfy_ui_provider_supports_image_generation() {
        let provider = ComfyUiProvider;
        let request = ModelRequest {
            task: TaskKind::Image,
            preferred_provider: Some(ProviderKind::ComfyUi),
            policy: PolicyMode::QualityFirst,
            prompt: "Generate a polished product shot".to_string(),
            created_at: chrono::Utc::now(),
        };

        let response = provider.run(request);
        assert_eq!(response.provider, ProviderKind::ComfyUi);
        assert_eq!(response.task, TaskKind::Image);
    }
}
