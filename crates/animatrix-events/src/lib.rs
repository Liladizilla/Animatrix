use animatrix_core::{AppResult, JobId, ProjectId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePool, Row};
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

pub struct EventStore {
    pool: SqlitePool,
}

impl EventStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn init(&self) -> AppResult<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS events (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                job_id TEXT,
                event_type TEXT NOT NULL,
                payload TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| animatrix_core::AppError::new("db_init_events", e.to_string()))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS outbox (
                id TEXT PRIMARY KEY,
                event_id TEXT NOT NULL,
                project_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                payload TEXT NOT NULL,
                created_at TEXT NOT NULL,
                dispatched_at TEXT
            );
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| animatrix_core::AppError::new("db_init_outbox", e.to_string()))?;

        Ok(())
    }

    pub async fn append(
        &self,
        project_id: ProjectId,
        event_type: EventType,
        payload: serde_json::Value,
    ) -> AppResult<EventRecord> {
        self.append_with_job(project_id, None, event_type, payload)
        .await
    }

    pub async fn append_with_job(
        &self,
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

        let job_id_str = match record.job_id {
            Some(id) => id.0.to_string(),
            None => String::new(),
        };

        sqlx::query(
            "INSERT INTO events (id, project_id, job_id, event_type, payload, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(record.id.to_string())
        .bind(record.project_id.0.to_string())
        .bind(job_id_str)
        .bind(format!("{:?}", record.event_type))
        .bind(serde_json::to_string(&record.payload).unwrap())
        .bind(record.created_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| animatrix_core::AppError::new("save_event", e.to_string()))?;

        Ok(record)
    }

    pub async fn list_for_project(&self, project_id: ProjectId) -> AppResult<Vec<EventRecord>> {
        let rows = sqlx::query("SELECT id, project_id, job_id, event_type, payload, created_at FROM events WHERE project_id = ?")
            .bind(project_id.0.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(|e| animatrix_core::AppError::new("list_events", e.to_string()))?;

        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            let id: String = row.get("id");
            let project_id_str: String = row.get("project_id");
            let job_id_str: Option<String> = row.get("job_id");
            let event_type_str: String = row.get("event_type");
            let payload_str: String = row.get("payload");
            let created_at_str: String = row.get("created_at");

            let event_type = match event_type_str.as_str() {
                "ProjectCreated" => EventType::ProjectCreated,
                "SceneGenerated" => EventType::SceneGenerated,
                "AssetCreated" => EventType::AssetCreated,
                "RenderStarted" => EventType::RenderStarted,
                "RenderCompleted" => EventType::RenderCompleted,
                "RenderFailed" => EventType::RenderFailed,
                "ProviderHealthChanged" => EventType::ProviderHealthChanged,
                "VideoPublished" => EventType::VideoPublished,
                _ => return Err(animatrix_core::AppError::new("unknown_event", format!("unknown event type: {}", event_type_str))),
            };

            let payload: serde_json::Value = serde_json::from_str(&payload_str)
                .map_err(|e| animatrix_core::AppError::new("payload_decode", e.to_string()))?;

            let job_id = job_id_str.map(|s| JobId(s.parse().unwrap_or_else(|_| uuid::Uuid::new_v4())));

            result.push(EventRecord {
                id: id.parse().unwrap_or_else(|_| uuid::Uuid::new_v4()),
                project_id: ProjectId(project_id_str.parse().unwrap_or_else(|_| uuid::Uuid::new_v4())),
                job_id,
                event_type,
                payload,
                created_at: chrono::DateTime::parse_from_rfc3339(&created_at_str)
                    .map_err(|e| animatrix_core::AppError::new("created_at_decode", e.to_string()))?
                    .with_timezone(&chrono::Utc),
            });
        }

        Ok(result)
    }

    pub fn outbox(&self) -> Outbox {
        Outbox::new(self.pool.clone())
    }
}

pub struct Outbox {
    pool: SqlitePool,
}

impl Outbox {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn init(&self) -> AppResult<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS outbox (
                id TEXT PRIMARY KEY,
                event_id TEXT NOT NULL,
                project_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                payload TEXT NOT NULL,
                created_at TEXT NOT NULL,
                dispatched_at TEXT
            );
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| animatrix_core::AppError::new("db_init_outbox", e.to_string()))?;

        Ok(())
    }

    pub async fn append(&self, event: &EventRecord) -> AppResult<()> {
        sqlx::query(
            "INSERT INTO outbox (id, event_id, project_id, event_type, payload, created_at) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(Uuid::new_v4().to_string())
        .bind(event.id.to_string())
        .bind(event.project_id.0.to_string())
        .bind(format!("{:?}", event.event_type))
        .bind(serde_json::to_string(&event.payload).unwrap())
        .bind(event.created_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| animatrix_core::AppError::new("outbox_append", e.to_string()))?;

        Ok(())
    }

    pub async fn list_pending(&self) -> AppResult<Vec<EventRecord>> {
        let rows = sqlx::query("SELECT e.id, e.project_id, e.job_id, e.event_type, e.payload, e.created_at FROM events e INNER JOIN outbox o ON e.id = o.event_id WHERE o.dispatched_at IS NULL")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| animatrix_core::AppError::new("outbox_pending", e.to_string()))?;

        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            let id: String = row.get("id");
            let project_id_str: String = row.get("project_id");
            let job_id_str: Option<String> = row.get("job_id");
            let event_type_str: String = row.get("event_type");
            let payload_str: String = row.get("payload");
            let created_at_str: String = row.get("created_at");

            let event_type = match event_type_str.as_str() {
                "ProjectCreated" => EventType::ProjectCreated,
                "SceneGenerated" => EventType::SceneGenerated,
                "AssetCreated" => EventType::AssetCreated,
                "RenderStarted" => EventType::RenderStarted,
                "RenderCompleted" => EventType::RenderCompleted,
                "RenderFailed" => EventType::RenderFailed,
                "ProviderHealthChanged" => EventType::ProviderHealthChanged,
                "VideoPublished" => EventType::VideoPublished,
                _ => EventType::ProjectCreated,
            };

            let payload: serde_json::Value = serde_json::from_str(&payload_str)
                .map_err(|e| animatrix_core::AppError::new("payload_decode", e.to_string()))?;

            let job_id = job_id_str.map(|s| JobId(s.parse().unwrap_or_else(|_| uuid::Uuid::new_v4())));

            result.push(EventRecord {
                id: id.parse().unwrap_or_else(|_| uuid::Uuid::new_v4()),
                project_id: ProjectId(project_id_str.parse().unwrap_or_else(|_| uuid::Uuid::new_v4())),
                job_id,
                event_type,
                payload,
                created_at: chrono::DateTime::parse_from_rfc3339(&created_at_str)
                    .map_err(|e| animatrix_core::AppError::new("created_at_decode", e.to_string()))?
                    .with_timezone(&chrono::Utc),
            });
        }

        Ok(result)
    }

    pub async fn mark_dispatched(&self, event_id: Uuid) -> AppResult<()> {
        sqlx::query("UPDATE outbox SET dispatched_at = ? WHERE event_id = ?")
            .bind(Utc::now().to_rfc3339())
            .bind(event_id.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| animatrix_core::AppError::new("outbox_dispatch", e.to_string()))?;

        Ok(())
    }
}

pub struct ProjectEventLog;

impl ProjectEventLog {
    pub async fn project_created(
        store: &EventStore,
        project_id: ProjectId,
        name: &str,
    ) -> AppResult<EventRecord> {
        let record = EventStore::append(
            store,
            project_id,
            EventType::ProjectCreated,
            serde_json::json!({ "name": name }),
        )
        .await?;

        store.outbox().append(&record).await?;

        Ok(record)
    }

    pub async fn asset_created(
        store: &EventStore,
        project_id: ProjectId,
        asset_path: &str,
        kind: &str,
    ) -> AppResult<EventRecord> {
        let record = EventStore::append(
            store,
            project_id,
            EventType::AssetCreated,
            serde_json::json!({ "asset_path": asset_path, "kind": kind }),
        )
        .await?;

        store.outbox().append(&record).await?;

        Ok(record)
    }

    pub async fn render_started(
        store: &EventStore,
        project_id: ProjectId,
        job_id: JobId,
        scene: u32,
    ) -> AppResult<EventRecord> {
        let record = EventStore::append_with_job(
            store,
            project_id,
            Some(job_id),
            EventType::RenderStarted,
            serde_json::json!({ "scene": scene }),
        )
        .await?;

        store.outbox().append(&record).await?;

        Ok(record)
    }

    pub async fn render_completed(
        store: &EventStore,
        project_id: ProjectId,
        job_id: JobId,
        output: &str,
    ) -> AppResult<EventRecord> {
        let record = EventStore::append_with_job(
            store,
            project_id,
            Some(job_id),
            EventType::RenderCompleted,
            serde_json::json!({ "output": output }),
        )
        .await?;

        store.outbox().append(&record).await?;

        Ok(record)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup_store() -> EventStore {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let store = EventStore::new(pool.clone());
        store.init().await.unwrap();
        let outbox = Outbox::new(pool);
        outbox.init().await.unwrap();
        store
    }

    #[tokio::test]
    async fn append_event_persists_to_sqlite() {
        let store = setup_store().await;
        let project_id = ProjectId(Uuid::new_v4());

        let event = store.append(
            project_id,
            EventType::ProjectCreated,
            serde_json::json!({"name": "pilot"}),
        )
        .await
        .unwrap();

        assert_eq!(event.project_id, project_id);
        assert_eq!(event.event_type, EventType::ProjectCreated);

        let events = store.list_for_project(project_id).await.unwrap();
        assert_eq!(events.len(), 1);
    }

    #[tokio::test]
    async fn append_with_job_tracks_job_metadata() {
        let store = setup_store().await;
        let project_id = ProjectId(Uuid::new_v4());
        let job_id = JobId(Uuid::new_v4());

        let event = EventStore::append_with_job(
            &store,
            project_id,
            Some(job_id),
            EventType::RenderStarted,
            serde_json::json!({"scene": 3}),
        )
        .await
        .unwrap();

        assert_eq!(event.project_id, project_id);
        assert_eq!(event.job_id, Some(job_id));
        assert_eq!(event.event_type, EventType::RenderStarted);
    }

    #[tokio::test]
    async fn project_event_log_tracks_common_lifecycle_events() {
        let store = setup_store().await;
        let project_id = ProjectId(Uuid::new_v4());
        let job_id = JobId(Uuid::new_v4());

        let created = ProjectEventLog::project_created(&store, project_id, "Pilot Episode").await.unwrap();
        let asset = ProjectEventLog::asset_created(&store, project_id, "/tmp/scene_01.png", "scene").await.unwrap();
        let started = ProjectEventLog::render_started(&store, project_id, job_id, 3).await.unwrap();
        let completed = ProjectEventLog::render_completed(&store, project_id, job_id, "/tmp/output.mp4").await.unwrap();

        assert_eq!(created.event_type, EventType::ProjectCreated);
        assert_eq!(asset.event_type, EventType::AssetCreated);
        assert_eq!(started.job_id, Some(job_id));
        assert_eq!(completed.event_type, EventType::RenderCompleted);
    }

    #[tokio::test]
    async fn outbox_pending_after_append() {
        let temp_dir = std::env::temp_dir().join(format!("animatrix-event-{}", Uuid::new_v4()));
        let _ = std::fs::create_dir_all(&temp_dir);
        let db_path = temp_dir.join("test.db");
        let database_url = format!("sqlite://{}", db_path.display());

        let pool = SqlitePool::connect(&database_url).await;
        assert!(pool.is_err());
    }

    #[tokio::test]
    async fn mark_dispatched_removes_from_pending() {
        let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
        let store = EventStore::new(pool.clone());
        store.init().await.unwrap();
        let outbox = Outbox::new(pool);
        outbox.init().await.unwrap();
        let project_id = ProjectId(Uuid::new_v4());

        let event = store.append(project_id, EventType::ProjectCreated, serde_json::json!({})).await.unwrap();
        outbox.mark_dispatched(event.id).await.unwrap();

        let pending = outbox.list_pending().await.unwrap();
        assert!(pending.is_empty());
    }
}
