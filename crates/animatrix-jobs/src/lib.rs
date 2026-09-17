use animatrix_core::{JobId, JobStatus, ProjectId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: JobId,
    pub project_id: ProjectId,
    pub job_type: String,
    pub status: JobStatus,
    pub priority: u8,
    pub attempts: u32,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub progress: u8,
}

impl Job {
    pub fn new(project_id: ProjectId, job_type: impl Into<String>) -> Self {
        Self {
            id: JobId(Uuid::new_v4()),
            project_id,
            job_type: job_type.into(),
            status: JobStatus::Queued,
            priority: 5,
            attempts: 0,
            created_at: Utc::now(),
            started_at: None,
            finished_at: None,
            progress: 0,
        }
    }

    pub fn start(&mut self) {
        self.status = JobStatus::Running;
        self.started_at = Some(Utc::now());
        self.progress = 25;
    }

    pub fn advance(&mut self, percent: u8) {
        if self.status == JobStatus::Running || self.status == JobStatus::Queued {
            self.status = JobStatus::Running;
            self.progress = percent.min(99);
        }
    }

    pub fn complete(&mut self) {
        self.status = JobStatus::Completed;
        self.finished_at = Some(Utc::now());
        self.progress = 100;
    }

    pub fn fail(&mut self) {
        self.status = JobStatus::Failed;
        self.finished_at = Some(Utc::now());
        self.progress = 0;
    }
}

pub struct JobManager;

impl JobManager {
    pub fn new_scene_job(project_id: ProjectId) -> Job {
        Job::new(project_id, "scene_generation")
    }

    pub fn new_render_job(project_id: ProjectId) -> Job {
        Job::new(project_id, "render_export")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_starts_queued() {
        let project_id = ProjectId(Uuid::new_v4());
        let job = Job::new(project_id, "generate_scene");
        assert_eq!(job.status, JobStatus::Queued);
        assert_eq!(job.progress, 0);
    }

    #[test]
    fn job_lifecycle_transitions_state() {
        let project_id = ProjectId(Uuid::new_v4());
        let mut job = JobManager::new_scene_job(project_id);

        job.start();
        assert_eq!(job.status, JobStatus::Running);
        assert_eq!(job.progress, 25);

        job.advance(80);
        assert_eq!(job.progress, 80);

        job.complete();
        assert_eq!(job.status, JobStatus::Completed);
        assert_eq!(job.progress, 100);
    }
}
