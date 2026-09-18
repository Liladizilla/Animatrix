use animatrix_core::{JobId, JobStatus, ProjectId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
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

#[derive(Debug, Clone, Default)]
pub struct JobQueue {
    jobs: Vec<Job>,
}

impl JobQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enqueue(&mut self, job: Job) {
        self.jobs.push(job);
    }

    pub fn next_pending(&mut self) -> Option<Job> {
        if self.jobs.is_empty() {
            return None;
        }

        let index = self.jobs.iter().position(|job| job.status == JobStatus::Queued)?;
        let job = self.jobs.remove(index);
        Some(job)
    }

    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.jobs.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct JobWorker {
    project_id: ProjectId,
}

impl JobWorker {
    pub fn new(project_id: ProjectId) -> Self {
        Self { project_id }
    }

    pub async fn run_one(&self, queue: Arc<Mutex<JobQueue>>) -> Option<Job> {
        let mut queue_guard = queue.lock().expect("job queue lock poisoned");
        let mut job = queue_guard.next_pending()?;

        job.attempts += 1;
        job.start();
        job.advance(45);
        job.advance(75);
        job.advance(95);
        job.complete();

        Some(job)
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

    #[test]
    fn job_queue_tracks_pending_work() {
        let mut queue = JobQueue::new();
        let project_id = ProjectId(Uuid::new_v4());
        queue.enqueue(JobManager::new_scene_job(project_id));
        assert_eq!(queue.len(), 1);

        let next = queue.next_pending().unwrap();
        assert_eq!(next.job_type, "scene_generation");
        assert!(queue.is_empty());
    }

    #[tokio::test]
    async fn job_worker_processes_queued_job() {
        let project_id = ProjectId(Uuid::new_v4());
        let queue = Arc::new(Mutex::new(JobQueue::new()));
        {
            let mut queue_guard = queue.lock().expect("lock queue");
            queue_guard.enqueue(JobManager::new_render_job(project_id));
        }

        let worker = JobWorker::new(project_id);
        let completed = worker.run_one(queue.clone()).await.expect("queue should contain a job");
        assert_eq!(completed.status, JobStatus::Completed);
        assert_eq!(completed.progress, 100);
        assert_eq!(completed.attempts, 1);
    }

    #[tokio::test]
    async fn job_worker_queues_render_progress_steps() {
        let project_id = ProjectId(Uuid::new_v4());
        let queue = Arc::new(Mutex::new(JobQueue::new()));
        {
            let mut queue_guard = queue.lock().expect("lock queue");
            queue_guard.enqueue(JobManager::new_render_job(project_id));
        }

        let worker = JobWorker::new(project_id);
        let completed = worker.run_one(queue.clone()).await.expect("queue should contain a job");

        assert!(matches!(completed.status, JobStatus::Completed));
        assert!(completed.progress >= 90);
        assert_eq!(completed.attempts, 1);
    }
}
