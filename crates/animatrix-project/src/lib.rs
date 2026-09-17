use animatrix_core::{ChannelId, ProjectId};
use animatrix_domain::{Channel, Project};
use chrono::Utc;

pub struct ProjectManager;

impl ProjectManager {
    pub fn create_project(channel: &Channel, name: &str, description: &str) -> Project {
        Project {
            id: ProjectId(uuid::Uuid::new_v4()),
            channel_id: channel.id,
            name: name.to_string(),
            description: description.to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    pub fn new_channel(name: &str, theme: &str) -> Channel {
        let now = Utc::now();
        Channel {
            id: ChannelId(uuid::Uuid::new_v4()),
            name: name.to_string(),
            brand: animatrix_domain::BrandProfile {
                name: name.to_string(),
                tagline: theme.to_string(),
                tone: "professional".to_string(),
            },
            style: animatrix_domain::StyleProfile {
                art_style: "stickman explainer".to_string(),
                color_palette: vec!["#111827".to_string(), "#fbbf24".to_string()],
                camera_style: "center framing".to_string(),
                pacing: "medium".to_string(),
            },
            created_at: now,
            updated_at: now,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_project_from_channel() {
        let channel = ProjectManager::new_channel("Animatrix Studio", "AI video workflows");
        let project = ProjectManager::create_project(&channel, "Pilot Episode", "First long-form video");

        assert_eq!(project.channel_id, channel.id);
        assert_eq!(project.name, "Pilot Episode");
        assert_eq!(channel.brand.name, "Animatrix Studio");
    }
}
