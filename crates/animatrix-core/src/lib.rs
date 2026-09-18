use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct ChannelId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct ProjectId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct JobId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct AssetId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct RenderId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Timestamped<T> {
    pub value: T,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Queued,
    Running,
    Paused,
    Completed,
    Failed,
    Cancelled,
    Retrying,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderKind {
    HuggingFace,
    Local,
    ComfyUi,
    Groq,
    Cerebras,
    Together,
    Fireworks,
    DeepInfra,
    Fal,
    Replicate,
    OpenRouter,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderHealth {
    Healthy,
    Degraded,
    RateLimited,
    AuthenticationFailed,
    QuotaExceeded,
    Unavailable,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderHealthSnapshot {
    pub provider: ProviderKind,
    pub health: ProviderHealth,
    pub last_probe_at: Option<DateTime<Utc>>,
    pub p95_latency_ms: Option<u64>,
    pub failure_rate: f32,
    pub quota_remaining: Option<u64>,
    pub rate_limit_reset_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum GenerationStatus {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
    Retrying,
    Blocked,
    Validated,
    Invalid(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GenerationMetrics {
    pub generation_seconds: Option<f64>,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub retries: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerationError {
    pub code: String,
    pub provider: ProviderKind,
    pub retryable: bool,
    pub retry_after_seconds: Option<u64>,
    pub details: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetProvenance {
    pub provider: Option<ProviderKind>,
    pub model: Option<String>,
    pub prompt: Option<String>,
    pub negative_prompt: Option<String>,
    pub generation_parameters: serde_json::Value,
    pub seed: Option<i64>,
    pub prompt_template_id: Option<String>,
    pub parent_assets: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostEstimate {
    pub estimated_usd: Option<f64>,
    pub actual_usd: Option<f64>,
    pub currency: String,
    pub unit: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub source: Option<String>,
}

impl AppError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            source: None,
        }
    }

    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = Some(source.into());
        self
    }
}

pub type AppResult<T> = Result<T, AppError>;

pub fn generate_id() -> Uuid {
    Uuid::new_v4()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        assert_ne!(generate_id(), generate_id());
    }

    #[test]
    fn timestamped_roundtrip() {
        let ts = chrono::Utc::now();
        let item = Timestamped {
            value: "ok".to_string(),
            created_at: ts,
            updated_at: ts,
        };
        let json = serde_json::to_string(&item).unwrap();
        let parsed: Timestamped<String> = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.value, "ok");
    }

    #[test]
    fn provider_health_serializes() {
        let snapshot = ProviderHealthSnapshot {
            provider: ProviderKind::HuggingFace,
            health: ProviderHealth::Healthy,
            last_probe_at: None,
            p95_latency_ms: Some(120),
            failure_rate: 0.0,
            quota_remaining: Some(950),
            rate_limit_reset_at: None,
        };
        let json = serde_json::to_string(&snapshot).unwrap();
        let parsed: ProviderHealthSnapshot = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.health, ProviderHealth::Healthy);
    }
}
