use animatrix_core::{AppResult, JobId, ProjectId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventType {
    ProjectCreated,
    SceneGenerated,
    AssetCreated,
    RenderStarted,
    RenderCompleted,
    RenderFailed,
    ProviderHealthChanged,
    VideoPublished,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRecord {
    pub id: Uuid,
    pub project_id: ProjectId,
    pub job_id: Option<JobId>,
    pub event_type: EventType,
    pub payload: serde_json::Value,
    pub created_at: DateTime<Utc>,
}

pub struct EventStore;

impl EventStore {
    pub fn append(
        project_id: ProjectId,
        event_type: EventType,
        payload: serde_json::Value,
    ) -> AppResult<EventRecord> {
        Self::append_with_job(project_id, None, event_type, payload)
    }

    pub fn append_with_job(
        project_id: ProjectId,
        job_id: Option<JobId>,
        event_type: EventType,
        payload: serde_json::Value,
    ) -> AppResult<EventRecord> {
        let record = EventRecord {
            id: Uuid::new_v4(),
            project_id,
            job_id,
            event_type,
            payload,
            created_at: Utc::now(),
        };
        Ok(record)
    }
}

pub struct ProjectEventLog;

impl ProjectEventLog {
    pub fn project_created(project_id: ProjectId, name: &str) -> AppResult<EventRecord> {
        EventStore::append(
            project_id,
            EventType::ProjectCreated,
            serde_json::json!({ "name": name }),
        )
    }

    pub fn asset_created(project_id: ProjectId, asset_path: &str, kind: &str) -> AppResult<EventRecord> {
        EventStore::append(
            project_id,
            EventType::AssetCreated,
            serde_json::json!({ "asset_path": asset_path, "kind": kind }),
        )
    }

    pub fn render_started(project_id: ProjectId, job_id: JobId, scene: u32) -> AppResult<EventRecord> {
        EventStore::append_with_job(
            project_id,
            Some(job_id),
            EventType::RenderStarted,
            serde_json::json!({ "scene": scene }),
        )
    }

    pub fn render_completed(project_id: ProjectId, job_id: JobId, output: &str) -> AppResult<EventRecord> {
        EventStore::append_with_job(
            project_id,
            Some(job_id),
            EventType::RenderCompleted,
            serde_json::json!({ "output": output }),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_event_creates_record() {
        let project_id = ProjectId(Uuid::new_v4());
        let event = EventStore::append(
            project_id,
            EventType::ProjectCreated,
            serde_json::json!({"name": "pilot"}),
        )
        .unwrap();

        assert_eq!(event.project_id, project_id);
        assert_eq!(event.event_type, EventType::ProjectCreated);
    }

    #[test]
    fn append_event_can_capture_job_metadata() {
        let project_id = ProjectId(Uuid::new_v4());
        let job_id = JobId(Uuid::new_v4());
        let event = EventStore::append_with_job(
            project_id,
            Some(job_id),
            EventType::RenderStarted,
            serde_json::json!({"scene": 3}),
        )
        .unwrap();

        assert_eq!(event.project_id, project_id);
        assert_eq!(event.job_id, Some(job_id));
        assert_eq!(event.event_type, EventType::RenderStarted);
    }

    #[test]
    fn project_event_log_tracks_common_lifecycle_events() {
        let project_id = ProjectId(Uuid::new_v4());
        let job_id = JobId(Uuid::new_v4());

        let created = ProjectEventLog::project_created(project_id, "Pilot Episode").unwrap();
        let asset = ProjectEventLog::asset_created(project_id, "/tmp/scene_01.png", "scene").unwrap();
        let started = ProjectEventLog::render_started(project_id, job_id, 3).unwrap();
        let completed = ProjectEventLog::render_completed(project_id, job_id, "/tmp/output.mp4").unwrap();

        assert_eq!(created.event_type, EventType::ProjectCreated);
        assert_eq!(asset.event_type, EventType::AssetCreated);
        assert_eq!(started.job_id, Some(job_id));
        assert_eq!(completed.event_type, EventType::RenderCompleted);
    }
}
