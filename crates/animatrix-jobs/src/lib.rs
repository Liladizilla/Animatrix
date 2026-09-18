use animatrix_core::{JobId, JobStatus, ProjectId, GenerationStatus, GenerationError};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: JobId,
    pub project_id: ProjectId,
    pub graph_node_id: Option<String>,
    pub job_type: String,
    pub status: JobStatus,
    pub generation_status: GenerationStatus,
    pub priority: i32,
    pub attempts: u32,
    pub idempotency_key: String,
    pub correlation_id: String,
    pub inputs: serde_json::Value,
    pub output_requirements: serde_json::Value,
    pub cost_ceiling: Option<serde_json::Value>,
    pub approval_gate: Option<serde_json::Value>,
    pub progress: u8,
    pub error: Option<String>,
    pub error_code: Option<String>,
    pub retryable: bool,
    pub retry_after_seconds: Option<u64>,
    pub metrics: Option<serde_json::Value>,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Job {
    pub fn new(project_id: ProjectId, job_type: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: JobId(Uuid::new_v4()),
            project_id,
            graph_node_id: None,
            job_type: job_type.into(),
            status: JobStatus::Queued,
            generation_status: GenerationStatus::Queued,
            priority: 5,
            attempts: 0,
            idempotency_key: format!("{}-{}", Uuid::new_v4(), now.timestamp_nanos_opt().unwrap_or(0)),
            correlation_id: format!("corr-{}", Uuid::new_v4()),
            inputs: serde_json::json!({}),
            output_requirements: serde_json::json!({}),
            cost_ceiling: None,
            approval_gate: None,
            progress: 0,
            error: None,
            error_code: None,
            retryable: false,
            retry_after_seconds: None,
            metrics: None,
            started_at: None,
            finished_at: None,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn start(&mut self) {
        let now = Utc::now();
        self.status = JobStatus::Running;
        self.generation_status = GenerationStatus::Running;
        self.started_at = Some(now);
        self.progress = 25;
        self.updated_at = now;
    }

    pub fn advance(&mut self, percent: u8) {
        let now = Utc::now();
        if self.status == JobStatus::Running || self.status == JobStatus::Queued {
            self.status = JobStatus::Running;
            self.generation_status = GenerationStatus::Running;
            self.progress = percent.min(99);
            self.updated_at = now;
        }
    }

    pub fn complete(&mut self) {
        let now = Utc::now();
        self.status = JobStatus::Completed;
        self.generation_status = GenerationStatus::Completed;
        self.finished_at = Some(now);
        self.progress = 100;
        self.updated_at = now;
    }

    pub fn fail(&mut self, error: impl Into<String>, code: impl Into<String>) {
        let now = Utc::now();
        self.status = JobStatus::Failed;
        self.generation_status = GenerationStatus::Invalid(error.into());
        self.error = Some(format!("{:?}", self.generation_status));
        self.error_code = Some(code.into());
        self.finished_at = Some(now);
        self.updated_at = now;
    }

    pub fn retry(&mut self) {
        let now = Utc::now();
        self.attempts += 1;
        self.status = JobStatus::Retrying;
        self.generation_status = GenerationStatus::Retrying;
        self.progress = 0;
        self.retryable = true;
        self.finished_at = None;
        self.updated_at = now;
    }

    pub fn cancel(&mut self) {
        let now = Utc::now();
        self.status = JobStatus::Cancelled;
        self.generation_status = GenerationStatus::Cancelled;
        self.finished_at = Some(now);
        self.updated_at = now;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobAttempt {
    pub id: Uuid,
    pub job_id: JobId,
    pub worker_id: Option<String>,
    pub transport: String,
    pub request: serde_json::Value,
    pub response: Option<serde_json::Value>,
    pub logs: Vec<String>,
    pub metrics: Option<serde_json::Value>,
    pub status: GenerationStatus,
    pub started_at: Option<DateTime<Utc>>,
    pub finished_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

impl JobAttempt {
    pub fn new(job_id: JobId, transport: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            job_id,
            worker_id: None,
            transport: transport.into(),
            request: serde_json::json!({}),
            response: None,
            logs: Vec::new(),
            metrics: None,
            status: GenerationStatus::Queued,
            started_at: None,
            finished_at: None,
            created_at: now,
        }
    }

    pub fn add_log(&mut self, message: impl Into<String>) {
        self.logs.push(format!("[{}] {}", Utc::now(), message.into()));
    }

    pub fn start(&mut self, worker_id: impl Into<String>) {
        let now = Utc::now();
        self.worker_id = Some(worker_id.into());
        self.started_at = Some(now);
        self.status = GenerationStatus::Running;
    }

    pub fn complete(&mut self, metrics: Option<serde_json::Value>) {
        let now = Utc::now();
        self.finished_at = Some(now);
        self.status = GenerationStatus::Completed;
        self.metrics = metrics;
    }

    pub fn fail(&mut self, error: GenerationError) {
        let now = Utc::now();
        self.finished_at = Some(now);
        self.status = GenerationStatus::Invalid(format!(
            "{}: {}",
            error.code, error.details
        ));
    }
}

#[derive(Debug, Clone, Default)]
pub struct JobQueue {
    jobs: Arc<Mutex<Vec<Job>>>,
}

impl JobQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enqueue(&mut self, job: Job) {
        self.jobs.lock().expect("job queue lock poisoned").push(job);
    }

    pub fn next_pending(&mut self) -> Option<Job> {
        let mut guard = self.jobs.lock().expect("job queue lock poisoned");
        let index = guard.iter().position(|job| job.status == JobStatus::Queued)?;
        Some(guard.remove(index))
    }

    pub fn len(&self) -> usize {
        self.jobs.lock().expect("job queue lock poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.jobs.lock().expect("job queue lock poisoned").is_empty()
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

    pub fn new_generation_job(
        project_id: ProjectId,
        task: &str,
        model: &str,
        inputs: serde_json::Value,
    ) -> Job {
        let mut job = Job::new(project_id, "generation");
        job.job_type = format!("generation:{}", task);
        job.inputs = inputs;
        job.output_requirements = serde_json::json!({
            "model": model,
            "task": task
        });
        job
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
        assert!(!job.idempotency_key.is_empty());
        assert!(!job.correlation_id.is_empty());
    }

    #[test]
    fn job_lifecycle_transitions_state() {
        let project_id = ProjectId(Uuid::new_v4());
        let mut job = JobManager::new_scene_job(project_id);

        job.start();
        assert_eq!(job.status, JobStatus::Running);
        assert_eq!(job.generation_status, GenerationStatus::Running);
        assert_eq!(job.progress, 25);

        job.advance(80);
        assert_eq!(job.progress, 80);

        job.complete();
        assert_eq!(job.status, JobStatus::Completed);
        assert_eq!(job.generation_status, GenerationStatus::Completed);
        assert_eq!(job.progress, 100);
    }

    #[test]
    fn job_retry_increments_attempts() {
        let project_id = ProjectId(Uuid::new_v4());
        let mut job = JobManager::new_scene_job(project_id);
        job.fail("rate limited", "RATE_LIMIT");
        assert_eq!(job.status, JobStatus::Failed);

        job.retry();
        assert_eq!(job.status, JobStatus::Retrying);
        assert_eq!(job.attempts, 1);
        assert!(job.retryable);
    }

    #[test]
    fn job_cancel_sets_terminal_state() {
        let project_id = ProjectId(Uuid::new_v4());
        let mut job = JobManager::new_scene_job(project_id);
        job.start();
        job.cancel();
        assert_eq!(job.status, JobStatus::Cancelled);
        assert!(job.finished_at.is_some());
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

    #[test]
    fn job_attempt_tracks_worker_and_logs() {
        let job_id = JobId(Uuid::new_v4());
        let mut attempt = JobAttempt::new(job_id, "sqlite");
        attempt.add_log("starting");
        attempt.start("worker-1");
        attempt.add_log("processing");
        assert_eq!(attempt.worker_id, Some("worker-1".to_string()));
        assert_eq!(attempt.status, GenerationStatus::Running);
        assert_eq!(attempt.logs.len(), 2);

        let metrics = Some(serde_json::json!({"duration_ms": 120}));
        attempt.complete(metrics);
        assert_eq!(attempt.status, GenerationStatus::Completed);
    }

    #[test]
    fn job_attempt_fails_with_generation_error() {
        let job_id = JobId(Uuid::new_v4());
        let mut attempt = JobAttempt::new(job_id, "sqlite");
        let error = GenerationError {
            code: "RATE_LIMIT".to_string(),
            provider: animatrix_core::ProviderKind::HuggingFace,
            retryable: true,
            retry_after_seconds: Some(42),
            details: "Rate limit exceeded".to_string(),
        };
        attempt.fail(error);
        assert_eq!(attempt.status, GenerationStatus::Invalid("RATE_LIMIT: Rate limit exceeded".to_string()));
        assert!(attempt.finished_at.is_some());
    }

    #[test]
    fn generation_error_is_retryable() {
        let error = GenerationError {
            code: "TIMEOUT".to_string(),
            provider: animatrix_core::ProviderKind::Groq,
            retryable: true,
            retry_after_seconds: Some(10),
            details: "Request timed out".to_string(),
        };
        assert!(error.retryable);
        assert_eq!(error.retry_after_seconds, Some(10));
    }
}
