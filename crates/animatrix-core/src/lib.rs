use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct ChannelId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct ProjectId(pub Uuid);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub struct JobId(pub Uuid);

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppError {
    pub code: String,
    pub message: String,
}

impl AppError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
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
}
