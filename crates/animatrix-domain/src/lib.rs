use animatrix_core::{ChannelId, ProjectId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrandProfile {
    pub name: String,
    pub tagline: String,
    pub tone: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StyleProfile {
    pub art_style: String,
    pub color_palette: Vec<String>,
    pub camera_style: String,
    pub pacing: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Channel {
    pub id: ChannelId,
    pub name: String,
    pub brand: BrandProfile,
    pub style: StyleProfile,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub channel_id: ChannelId,
    pub name: String,
    pub description: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptSection {
    pub id: String,
    pub title: String,
    pub narration: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Script {
    pub id: Uuid,
    pub project_id: ProjectId,
    pub title: String,
    pub hook: String,
    pub sections: Vec<ScriptSection>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use animatrix_core::generate_id;

    #[test]
    fn channel_model_builds() {
        let now = chrono::Utc::now();
        let channel = Channel {
            id: ChannelId(generate_id()),
            name: "Example Channel".to_string(),
            brand: BrandProfile {
                name: "Example Channel".to_string(),
                tagline: "Explain, build, iterate".to_string(),
                tone: "professional".to_string(),
            },
            style: StyleProfile {
                art_style: "stickman explainer".to_string(),
                color_palette: vec!["#111827".to_string(), "#f59e0b".to_string()],
                camera_style: "clean center framing".to_string(),
                pacing: "medium".to_string(),
            },
            created_at: now,
            updated_at: now,
        };

        assert_eq!(channel.name, "Example Channel");
    }
}
