use animatrix_core::{AppResult, ChannelId, ProjectId};
use animatrix_domain::{Channel, Project};
use animatrix_events::{EventRecord, ProjectEventLog};
use animatrix_jobs::{Job, JobManager};
use chrono::Utc;

#[derive(Debug, Clone)]
pub struct ProjectWorkflow {
    pub project: Project,
    pub creation_event: EventRecord,
    pub initial_jobs: Vec<Job>,
}

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

    pub fn create_workflow(channel: &Channel, name: &str, description: &str) -> AppResult<ProjectWorkflow> {
        let project = Self::create_project(channel, name, description);
        let creation_event = ProjectEventLog::project_created(project.id, name)?;
        let initial_jobs = vec![
            JobManager::new_scene_job(project.id),
            JobManager::new_render_job(project.id),
        ];

        Ok(ProjectWorkflow {
            project,
            creation_event,
            initial_jobs,
        })
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

    #[test]
    fn create_project_workflow_generates_event_and_jobs() {
        let channel = ProjectManager::new_channel("Animatrix Studio", "AI video workflows");
        let workflow = ProjectManager::create_workflow(&channel, "Pilot Episode", "First long-form video").unwrap();

        assert_eq!(workflow.project.channel_id, channel.id);
        assert_eq!(workflow.project.name, "Pilot Episode");
        assert_eq!(workflow.creation_event.event_type, animatrix_events::EventType::ProjectCreated);
        assert_eq!(workflow.initial_jobs.len(), 2);
        assert!(workflow.initial_jobs.iter().any(|job| job.job_type == "scene_generation"));
        assert!(workflow.initial_jobs.iter().any(|job| job.job_type == "render_export"));
    }
}
