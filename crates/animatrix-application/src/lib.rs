use animatrix_ai::{PolicyMode, TaskKind};
use animatrix_ai_router::RouteRequest;
use animatrix_core::{AppError, AppResult, ChannelId, ProjectId};
use animatrix_events::{EventStore, ProjectEventLog};
use animatrix_jobs::Job;
use animatrix_project::ProjectManager;
use animatrix_storage::LocalStore;
use chrono::Utc;
use std::path::Path;
use std::sync::Arc;
use tracing::info;

pub struct StudioApp {
    store: LocalStore,
    event_store: EventStore,
}

impl StudioApp {
    pub async fn open(database_url: &str) -> AppResult<Self> {
        let store = LocalStore::open(database_url).await?;
        let pool = store.pool();
        let event_store = EventStore::new(pool.clone());
        event_store.init().await?;
        let outbox = event_store.outbox();
        outbox.init().await?;

        Ok(Self { store, event_store })
    }

    pub fn config(&self) -> &animatrix_storage::AppConfig {
        self.store.config()
    }

    pub async fn create_channel(&self, name: &str, theme: &str) -> AppResult<animatrix_domain::Channel> {
        let channel = ProjectManager::new_channel(name, theme);
        self.store.save_channel(&channel).await?;
        info!(channel = %channel.name, "Channel created");
        Ok(channel)
    }

    pub async fn create_project(
        &self,
        channel_id: ChannelId,
        name: &str,
        description: &str,
    ) -> AppResult<animatrix_domain::Project> {
        let channels = self.store.list_channels().await?;
        let channel = channels
            .iter()
            .find(|c| c.id == channel_id)
            .ok_or_else(|| AppError::new("channel_not_found", format!("channel not found: {}", channel_id.0)))?;

        let project = ProjectManager::create_project(channel, name, description);
        self.store.save_project(&project).await?;

        let workflow = ProjectManager::create_workflow(&self.event_store, channel, name, description).await?;

        ProjectEventLog::project_created(&self.event_store, project.id, name).await?;

        for job in &workflow.initial_jobs {
            self.store.save_job(job).await?;
        }

        info!(project = %project.name, "Project created with workflow");
        Ok(project)
    }

    pub async fn queue_generation(
        &self,
        project_id: ProjectId,
        task: &str,
        model: &str,
        prompt: &str,
    ) -> AppResult<Job> {
        let route_request = RouteRequest {
            task: TaskKind::Text,
            preferred_provider: None,
            policy: PolicyMode::Balanced,
            prompt: prompt.to_string(),
            cost_ceiling: None,
            require_online: false,
            created_at: Utc::now(),
        };

        let route = animatrix_ai_router::AiRouter::new().route(&route_request).await?;
        info!(provider = ?route.provider, task = task, "Generation queued");

        let inputs = serde_json::json!({ "prompt": prompt });
        let job = animatrix_jobs::JobManager::new_generation_job(project_id, task, model, inputs);
        self.store.save_job(&job).await?;

        Ok(job)
    }

    pub async fn complete_render(
        &self,
        project_id: ProjectId,
        output_path: impl AsRef<Path>,
    ) -> AppResult<animatrix_project::RenderOutcome> {
        let projects = self.store.list_projects().await?;
        let project = projects
            .iter()
            .find(|p| p.id == project_id)
            .ok_or_else(|| AppError::new("project_not_found", format!("project not found: {}", project_id.0)))?;

        let channels = self.store.list_channels().await?;
        let channel = channels
            .iter()
            .find(|c| c.id == project.channel_id)
            .ok_or_else(|| AppError::new("channel_not_found", "project channel not found"))?;

        let job = animatrix_jobs::JobManager::new_render_job(project_id);
        let outcome = ProjectManager::complete_render(&self.event_store, project, channel, &job, output_path).await?;

        self.store.save_job(&outcome.job).await?;
        self.store.save_asset(&outcome.asset).await?;
        ProjectEventLog::render_completed(&self.event_store, project_id, outcome.job.id, &outcome.asset.path).await?;

        info!(asset = %outcome.asset.path, "Render completed");
        Ok(outcome)
    }

    pub async fn run_worker(&self) -> AppResult<Vec<Job>> {
        let mut queue = animatrix_jobs::JobQueue::new();
        let jobs = self.store.list_projects().await?;

        for project in &jobs {
            let job = animatrix_jobs::JobManager::new_scene_job(project.id);
            queue.enqueue(job);
        }

        let mut completed = Vec::new();
        let queue_arc = Arc::new(std::sync::Mutex::new(queue));

        for project in &jobs {
            let worker = animatrix_jobs::JobWorker::new(project.id);
            if let Some(job) = worker.run_one(Arc::clone(&queue_arc)).await {
                self.store.save_job(&job).await?;
                completed.push(job);
            }
        }

        info!(count = completed.len(), "Worker completed jobs");
        Ok(completed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn studio_app_creates_channel() {
        let app = StudioApp::open("sqlite::memory:").await.unwrap();
        let channel = app.create_channel("Test Channel", "AI video").await.unwrap();

        assert_eq!(channel.name, "Test Channel");
        let channels = app.store.list_channels().await.unwrap();
        assert_eq!(channels.len(), 1);
    }

    #[tokio::test]
    async fn studio_app_creates_project() {
        let app = StudioApp::open("sqlite::memory:").await.unwrap();
        let channel = app.create_channel("Test Channel", "AI video").await.unwrap();
        let project = app.create_project(channel.id, "Test Project", "A test").await.unwrap();

        assert_eq!(project.name, "Test Project");
        assert_eq!(project.channel_id, channel.id);

        let projects = app.store.list_projects().await.unwrap();
        assert_eq!(projects.len(), 1);
    }

    #[tokio::test]
    async fn studio_app_queues_generation() {
        let app = StudioApp::open("sqlite::memory:").await.unwrap();
        let channel = app.create_channel("Test Channel", "AI video").await.unwrap();
        let project = app.create_project(channel.id, "Test Project", "A test").await.unwrap();

        let job = app.queue_generation(project.id, "text", "gpt-4", "Hello world").await.unwrap();
        assert_eq!(job.status, animatrix_core::JobStatus::Queued);
        assert!(job.idempotency_key.len() > 0);
    }

    #[tokio::test]
    async fn studio_app_completes_render() {
        let app = StudioApp::open("sqlite::memory:").await.unwrap();
        let channel = app.create_channel("Test Channel", "AI video").await.unwrap();
        let project = app.create_project(channel.id, "Test Project", "A test").await.unwrap();

        let temp_path = std::env::temp_dir().join(format!("render-{}.mp4", uuid::Uuid::new_v4()));
        std::fs::write(&temp_path, b"output").unwrap();

        let outcome = app.complete_render(project.id, &temp_path).await.unwrap();

        assert_eq!(outcome.job.status, animatrix_core::JobStatus::Completed);
        assert_eq!(outcome.asset.project_id, project.id);

        let _ = std::fs::remove_file(temp_path);
    }

    #[tokio::test]
    async fn studio_app_runs_worker() {
        let app = StudioApp::open("sqlite::memory:").await.unwrap();
        let channel = app.create_channel("Test Channel", "AI video").await.unwrap();
        let _project = app.create_project(channel.id, "Test Project", "A test").await.unwrap();

        let completed = app.run_worker().await.unwrap();
        assert!(!completed.is_empty());
    }
}
