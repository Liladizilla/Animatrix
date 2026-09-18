use animatrix_assets::{AssetRegistry, AssetSource, AssetType};
use animatrix_core::{AppResult, ChannelId, ProjectId, ProviderKind};
use animatrix_domain::{Channel, Project};
use animatrix_events::{EventRecord, EventStore, ProjectEventLog};
use animatrix_jobs::{Job, JobManager};
use chrono::Utc;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct ProjectWorkflow {
    pub project: Project,
    pub creation_event: EventRecord,
    pub initial_jobs: Vec<Job>,
}

#[derive(Debug, Clone)]
pub struct RenderOutcome {
    pub job: Job,
    pub asset: animatrix_assets::Asset,
    pub event: EventRecord,
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

    pub async fn create_workflow(
        event_store: &EventStore,
        channel: &Channel,
        name: &str,
        description: &str,
    ) -> AppResult<ProjectWorkflow> {
        let project = Self::create_project(channel, name, description);
        let creation_event = ProjectEventLog::project_created(event_store, project.id, name).await?;
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

    pub async fn complete_render(
        event_store: &EventStore,
        project: &Project,
        channel: &Channel,
        job: &Job,
        output_path: impl AsRef<Path>,
    ) -> AppResult<RenderOutcome> {
        let output_path = output_path.as_ref();
        let mut completed_job = job.clone();
        completed_job.complete();

        let asset = AssetRegistry::register_asset_with_context(
            project.id,
            channel.id,
            AssetType::Video,
            AssetSource::Generated,
            Some(ProviderKind::Local),
            Some("animatrix-render".to_string()),
            None,
            output_path,
        )?;

        let event = ProjectEventLog::render_completed(event_store, project.id, job.id, &asset.path).await?;

        Ok(RenderOutcome {
            job: completed_job,
            asset,
            event,
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
    use animatrix_events::EventStore;
    use sqlx::SqlitePool;

    async fn setup_event_store() -> EventStore {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let store = EventStore::new(pool);
        store.init().await.unwrap();
        store
    }

    #[tokio::test]
    async fn create_project_from_channel() {
        let channel = ProjectManager::new_channel("Animatrix Studio", "AI video workflows");
        let project = ProjectManager::create_project(&channel, "Pilot Episode", "First long-form video");

        assert_eq!(project.channel_id, channel.id);
        assert_eq!(project.name, "Pilot Episode");
        assert_eq!(channel.brand.name, "Animatrix Studio");
    }

    #[tokio::test]
    async fn create_project_workflow_generates_event_and_jobs() {
        let channel = ProjectManager::new_channel("Animatrix Studio", "AI video workflows");
        let store = setup_event_store().await;
        let workflow = ProjectManager::create_workflow(&store, &channel, "Pilot Episode", "First long-form video").await.unwrap();

        assert_eq!(workflow.project.channel_id, channel.id);
        assert_eq!(workflow.project.name, "Pilot Episode");
        assert_eq!(workflow.creation_event.event_type, animatrix_events::EventType::ProjectCreated);
        assert_eq!(workflow.initial_jobs.len(), 2);
        assert!(workflow.initial_jobs.iter().any(|job| job.job_type == "scene_generation"));
        assert!(workflow.initial_jobs.iter().any(|job| job.job_type == "render_export"));
    }

    #[tokio::test]
    async fn complete_render_creates_output_asset_and_event() {
        let channel = ProjectManager::new_channel("Animatrix Studio", "AI video workflows");
        let project = ProjectManager::create_project(&channel, "Pilot Episode", "First long-form video");
        let store = setup_event_store().await;
        let job = animatrix_jobs::Job::new(project.id, "render_export");
        let temp_path = std::env::temp_dir().join(format!("animatrix-render-{}.mp4", uuid::Uuid::new_v4()));
        std::fs::write(&temp_path, b"fake-render").unwrap();

        let outcome = ProjectManager::complete_render(&store, &project, &channel, &job, &temp_path).await.unwrap();

        assert_eq!(outcome.event.event_type, animatrix_events::EventType::RenderCompleted);
        assert_eq!(outcome.asset.project_id, project.id);
        assert_eq!(outcome.asset.channel_id, channel.id);
        assert_eq!(outcome.asset.asset_type, AssetType::Video);
        assert_eq!(outcome.job.status, animatrix_core::JobStatus::Completed);

        let _ = std::fs::remove_file(temp_path);
    }
}
