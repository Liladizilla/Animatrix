use animatrix_core::{AppError, AppResult, ChannelId, ProjectId};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool, Row};
use uuid::Uuid;
use std::path::PathBuf;

pub use animatrix_domain::{BrandProfile, Channel, Project, StyleProfile};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AppState {
    pub projects: Vec<Project>,
    pub channels: Vec<Channel>,
}

impl AppState {
    pub fn new(channels: Vec<Channel>, projects: Vec<Project>) -> Self {
        Self { channels, projects }
    }

    pub fn is_empty(&self) -> bool {
        self.channels.is_empty() && self.projects.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub data_dir: PathBuf,
    pub database_path: PathBuf,
}

impl AppConfig {
    pub fn default() -> AppResult<Self> {
        let home = dirs::home_dir().ok_or_else(|| AppError::new("config_home", "unable to resolve home directory"))?;
        let data_dir = home.join(".animatrix");
        let db_path = data_dir.join("data").join("animatrix.db");

        Ok(Self { data_dir, database_path: db_path })
    }

    pub fn database_url(&self) -> String {
        format!("sqlite://{}", self.database_path.display())
    }
}

pub struct LocalStore {
    pool: SqlitePool,
    config: AppConfig,
}

impl LocalStore {
    pub async fn open(database_url: &str) -> AppResult<Self> {
        let config = AppConfig::default()?;
        Self::open_with_config(&config, database_url).await
    }

    pub async fn open_with_config(config: &AppConfig, database_url: &str) -> AppResult<Self> {
        if let Some(parent) = config.database_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::new("config_dir", e.to_string()))?;
        }

        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect(database_url)
            .await
            .map_err(|e| AppError::new("db_connect", e.to_string()))?;

        Self::init_tables(&pool).await?;

        Ok(Self { pool, config: config.clone() })
    }

    async fn init_tables(pool: &SqlitePool) -> AppResult<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS channels (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                brand_json TEXT NOT NULL,
                style_json TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| AppError::new("db_init", e.to_string()))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS projects (
                id TEXT PRIMARY KEY,
                channel_id TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| AppError::new("db_init", e.to_string()))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS assets (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                channel_id TEXT NOT NULL,
                asset_type TEXT NOT NULL,
                source TEXT NOT NULL,
                provider TEXT,
                model TEXT,
                path TEXT NOT NULL,
                hash TEXT NOT NULL,
                mime_type TEXT NOT NULL,
                size_bytes INTEGER NOT NULL,
                metadata TEXT NOT NULL,
                created_at TEXT NOT NULL,
                parent_asset TEXT
            );
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| AppError::new("db_init", e.to_string()))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS providers (
                id TEXT PRIMARY KEY,
                provider_kind TEXT NOT NULL,
                health TEXT NOT NULL,
                last_probe_at TEXT,
                p95_latency_ms INTEGER,
                failure_rate REAL NOT NULL DEFAULT 0.0,
                quota_remaining INTEGER,
                rate_limit_reset_at TEXT,
                updated_at TEXT NOT NULL
            );
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| AppError::new("db_init", e.to_string()))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS jobs (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                graph_node_id TEXT,
                job_type TEXT NOT NULL,
                status TEXT NOT NULL,
                generation_status TEXT NOT NULL,
                priority INTEGER NOT NULL DEFAULT 5,
                attempts INTEGER NOT NULL DEFAULT 0,
                idempotency_key TEXT NOT NULL,
                correlation_id TEXT NOT NULL,
                inputs TEXT NOT NULL,
                output_requirements TEXT NOT NULL,
                cost_ceiling TEXT,
                approval_gate TEXT,
                progress INTEGER NOT NULL DEFAULT 0,
                error TEXT,
                error_code TEXT,
                retryable INTEGER NOT NULL DEFAULT 0,
                retry_after_seconds INTEGER,
                metrics TEXT,
                started_at TEXT,
                finished_at TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| AppError::new("db_init", e.to_string()))?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS job_attempts (
                id TEXT PRIMARY KEY,
                job_id TEXT NOT NULL,
                worker_id TEXT,
                transport TEXT NOT NULL,
                request TEXT NOT NULL,
                response TEXT,
                logs TEXT NOT NULL DEFAULT '[]',
                metrics TEXT,
                status TEXT NOT NULL,
                started_at TEXT,
                finished_at TEXT,
                created_at TEXT NOT NULL
            );
            "#,
        )
        .execute(pool)
        .await
        .map_err(|e| AppError::new("db_init", e.to_string()))?;

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
        .execute(pool)
        .await
        .map_err(|e| AppError::new("db_init", e.to_string()))?;

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
        .execute(pool)
        .await
        .map_err(|e| AppError::new("db_init", e.to_string()))?;

        Ok(())
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn save_channel(&self, channel: &Channel) -> AppResult<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO channels (id, name, brand_json, style_json, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(channel.id.0.to_string())
        .bind(&channel.name)
        .bind(serde_json::to_string(&channel.brand).unwrap())
        .bind(serde_json::to_string(&channel.style).unwrap())
        .bind(channel.created_at.to_rfc3339())
        .bind(channel.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::new("save_channel", e.to_string()))?;

        Ok(())
    }

    pub async fn save_project(&self, project: &Project) -> AppResult<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO projects (id, channel_id, name, description, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind(project.id.0.to_string())
        .bind(project.channel_id.0.to_string())
        .bind(&project.name)
        .bind(&project.description)
        .bind(project.created_at.to_rfc3339())
        .bind(project.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::new("save_project", e.to_string()))?;

        Ok(())
    }

    pub async fn save_job(&self, job: &animatrix_jobs::Job) -> AppResult<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO jobs (
                id, project_id, graph_node_id, job_type, status, generation_status,
                priority, attempts, idempotency_key, correlation_id, inputs,
                output_requirements, cost_ceiling, approval_gate, progress,
                error, error_code, retryable, retry_after_seconds, metrics,
                started_at, finished_at, created_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(job.id.0.to_string())
        .bind(job.project_id.0.to_string())
        .bind(job.graph_node_id.as_ref().map(|s| s.as_str()))
        .bind(&job.job_type)
        .bind(format!("{:?}", job.status))
        .bind(format!("{:?}", job.generation_status))
        .bind(job.priority)
        .bind(job.attempts as i64)
        .bind(&job.idempotency_key)
        .bind(&job.correlation_id)
        .bind(serde_json::to_string(&job.inputs).unwrap())
        .bind(serde_json::to_string(&job.output_requirements).unwrap())
        .bind(serde_json::to_string(&job.cost_ceiling).unwrap_or_default())
        .bind(serde_json::to_string(&job.approval_gate).unwrap_or_default())
        .bind(job.progress as i64)
        .bind(job.error.as_ref().map(|s| s.as_str()))
        .bind(job.error_code.as_ref().map(|s| s.as_str()))
        .bind(job.retryable)
        .bind(job.retry_after_seconds.map(|s| s as i64))
        .bind(serde_json::to_string(&job.metrics).unwrap_or_default())
        .bind(job.started_at.map(|t| t.to_rfc3339()))
        .bind(job.finished_at.map(|t| t.to_rfc3339()))
        .bind(job.created_at.to_rfc3339())
        .bind(job.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::new("save_job", e.to_string()))?;

        Ok(())
    }

    pub async fn save_asset(&self, asset: &animatrix_assets::Asset) -> AppResult<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO assets (
                id, project_id, channel_id, asset_type, source, provider, model,
                path, hash, mime_type, size_bytes, metadata, created_at, parent_asset
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(asset.id.0.to_string())
        .bind(asset.project_id.0.to_string())
        .bind(asset.channel_id.0.to_string())
        .bind(format!("{:?}", asset.asset_type))
        .bind(format!("{:?}", asset.source))
        .bind(asset.provider.map(|p| format!("{:?}", p)))
        .bind(asset.model.as_ref().map(|s| s.as_str()))
        .bind(&asset.path)
        .bind(&asset.hash)
        .bind(&asset.mime_type)
        .bind(asset.size_bytes as i64)
        .bind(serde_json::to_string(&asset.metadata).unwrap())
        .bind(asset.created_at.to_rfc3339())
        .bind(asset.parent_asset.map(|id| id.0.to_string()))
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::new("save_asset", e.to_string()))?;

        Ok(())
    }

    pub async fn save_provider_health(&self, snapshot: &animatrix_core::ProviderHealthSnapshot) -> AppResult<()> {
        sqlx::query(
            r#"
            INSERT OR REPLACE INTO providers (
                id, provider_kind, health, last_probe_at, p95_latency_ms,
                failure_rate, quota_remaining, rate_limit_reset_at, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(Uuid::new_v4().to_string())
        .bind(format!("{:?}", snapshot.provider))
        .bind(format!("{:?}", snapshot.health))
        .bind(snapshot.last_probe_at.map(|t| t.to_rfc3339()))
        .bind(snapshot.p95_latency_ms.map(|ms| ms as i64))
        .bind(snapshot.failure_rate)
        .bind(snapshot.quota_remaining.map(|q| q as i64))
        .bind(snapshot.rate_limit_reset_at.map(|t| t.to_rfc3339()))
        .bind(Utc::now().to_rfc3339())
        .execute(&self.pool)
        .await
        .map_err(|e| AppError::new("save_provider_health", e.to_string()))?;

        Ok(())
    }

    pub async fn list_channels(&self) -> AppResult<Vec<Channel>> {
        let rows = sqlx::query("SELECT id, name, brand_json, style_json, created_at, updated_at FROM channels")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| AppError::new("list_channels", e.to_string()))?;

        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            let id: String = row.get("id");
            let name: String = row.get("name");
            let brand_json: String = row.get("brand_json");
            let style_json: String = row.get("style_json");
            let created_at: String = row.get("created_at");
            let updated_at: String = row.get("updated_at");

            let brand: BrandProfile = serde_json::from_str(&brand_json)
                .map_err(|e| AppError::new("brand_decode", e.to_string()))?;
            let style: StyleProfile = serde_json::from_str(&style_json)
                .map_err(|e| AppError::new("style_decode", e.to_string()))?;

            result.push(Channel {
                id: ChannelId(id.parse::<uuid::Uuid>().map_err(|e| AppError::new("channel_id", e.to_string()))?),
                name,
                brand,
                style,
                created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                    .map_err(|e| AppError::new("created_at_decode", e.to_string()))?
                    .with_timezone(&chrono::Utc),
                updated_at: chrono::DateTime::parse_from_rfc3339(&updated_at)
                    .map_err(|e| AppError::new("updated_at_decode", e.to_string()))?
                    .with_timezone(&chrono::Utc),
            });
        }

        Ok(result)
    }

    pub async fn list_projects(&self) -> AppResult<Vec<Project>> {
        let rows = sqlx::query("SELECT id, channel_id, name, description, created_at, updated_at FROM projects")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| AppError::new("list_projects", e.to_string()))?;

        let mut result = Vec::with_capacity(rows.len());
        for row in rows {
            let id: String = row.get("id");
            let channel_id: String = row.get("channel_id");
            let name: String = row.get("name");
            let description: String = row.get("description");
            let created_at: String = row.get("created_at");
            let updated_at: String = row.get("updated_at");

            result.push(Project {
                id: ProjectId(id.parse::<uuid::Uuid>().map_err(|e| AppError::new("project_id", e.to_string()))?),
                channel_id: ChannelId(channel_id.parse::<uuid::Uuid>().map_err(|e| AppError::new("channel_id", e.to_string()))?),
                name,
                description,
                created_at: chrono::DateTime::parse_from_rfc3339(&created_at)
                    .map_err(|e| AppError::new("created_at_decode", e.to_string()))?
                    .with_timezone(&chrono::Utc),
                updated_at: chrono::DateTime::parse_from_rfc3339(&updated_at)
                    .map_err(|e| AppError::new("updated_at_decode", e.to_string()))?
                    .with_timezone(&chrono::Utc),
            });
        }

        Ok(result)
    }

    pub async fn load_state(&self) -> AppResult<AppState> {
        let channels = self.list_channels().await?;
        let projects = self.list_projects().await?;

        Ok(AppState::new(channels, projects))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn database_url_uses_absolute_sqlite_uri() {
        let config = AppConfig::default().unwrap();
        let db_url = config.database_url();
        assert!(db_url.starts_with("sqlite:///"), "unexpected sqlite url: {db_url}");
    }

    #[tokio::test]
    async fn can_open_store_with_default_config_path() {
        let config = AppConfig::default().unwrap();
        let url = config.database_url();
        println!("opening sqlite url: {url}");
        let store = LocalStore::open_with_config(&config, &url).await.unwrap();

        let channel = Channel {
            id: ChannelId(Uuid::new_v4()),
            name: "Config Path Channel".to_string(),
            brand: BrandProfile {
                name: "Config Path Channel".to_string(),
                tagline: "AI first".to_string(),
                tone: "professional".to_string(),
            },
            style: StyleProfile {
                art_style: "explainer".to_string(),
                color_palette: vec!["#000000".to_string()],
                camera_style: "center".to_string(),
                pacing: "medium".to_string(),
            },
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        store.save_channel(&channel).await.unwrap();
        let saved = store.list_channels().await.unwrap();
        assert!(!saved.is_empty());
    }

    #[tokio::test]
    async fn can_load_app_state_from_store() {
        let store = LocalStore::open("sqlite::memory:").await.unwrap();
        let now = Utc::now();

        let channel = Channel {
            id: ChannelId(Uuid::new_v4()),
            name: "State Channel".to_string(),
            brand: BrandProfile {
                name: "State Channel".to_string(),
                tagline: "AI first".to_string(),
                tone: "professional".to_string(),
            },
            style: StyleProfile {
                art_style: "explainer".to_string(),
                color_palette: vec!["#000000".to_string()],
                camera_style: "center".to_string(),
                pacing: "medium".to_string(),
            },
            created_at: now,
            updated_at: now,
        };

        let project = Project {
            id: ProjectId(Uuid::new_v4()),
            channel_id: channel.id,
            name: "State Project".to_string(),
            description: "Loads from persisted state".to_string(),
            created_at: now,
            updated_at: now,
        };

        store.save_channel(&channel).await.unwrap();
        store.save_project(&project).await.unwrap();

        let state = store.load_state().await.unwrap();
        assert_eq!(state.channels.len(), 1);
        assert_eq!(state.projects.len(), 1);
        assert_eq!(state.channels[0].name, "State Channel");
        assert_eq!(state.projects[0].name, "State Project");
        assert_eq!(state.projects[0].channel_id, state.channels[0].id);
    }

    #[tokio::test]
    async fn repository_state_round_trips_multiple_records() {
        let store = LocalStore::open("sqlite::memory:").await.unwrap();
        let now = Utc::now();

        let channel_a = Channel {
            id: ChannelId(Uuid::new_v4()),
            name: "Brand One".to_string(),
            brand: BrandProfile {
                name: "Brand One".to_string(),
                tagline: "Bold strategy".to_string(),
                tone: "confident".to_string(),
            },
            style: StyleProfile {
                art_style: "cinematic".to_string(),
                color_palette: vec!["#111111".to_string(), "#F4F4F5".to_string()],
                camera_style: "wide".to_string(),
                pacing: "fast".to_string(),
            },
            created_at: now,
            updated_at: now,
        };

        let project_a = Project {
            id: ProjectId(Uuid::new_v4()),
            channel_id: channel_a.id,
            name: "Launch Film".to_string(),
            description: "Narrative launch work".to_string(),
            created_at: now,
            updated_at: now,
        };

        let channel_b = Channel {
            id: ChannelId(Uuid::new_v4()),
            name: "Brand Two".to_string(),
            brand: BrandProfile {
                name: "Brand Two".to_string(),
                tagline: "Steady clarity".to_string(),
                tone: "calm".to_string(),
            },
            style: StyleProfile {
                art_style: "minimal".to_string(),
                color_palette: vec!["#E2E8F0".to_string()],
                camera_style: "close-up".to_string(),
                pacing: "medium".to_string(),
            },
            created_at: now,
            updated_at: now,
        };

        let project_b = Project {
            id: ProjectId(Uuid::new_v4()),
            channel_id: channel_b.id,
            name: "Social Cut".to_string(),
            description: "Short-form social package".to_string(),
            created_at: now,
            updated_at: now,
        };

        store.save_channel(&channel_a).await.unwrap();
        store.save_channel(&channel_b).await.unwrap();
        store.save_project(&project_a).await.unwrap();
        store.save_project(&project_b).await.unwrap();

        let state = store.load_state().await.unwrap();
        assert_eq!(state.channels.len(), 2);
        assert_eq!(state.projects.len(), 2);
        assert!(state.channels.iter().any(|c| c.name == "Brand One"));
        assert!(state.projects.iter().any(|p| p.name == "Launch Film"));
        assert!(!AppState::new(state.channels.clone(), state.projects.clone()).is_empty());
    }

    #[tokio::test]
    async fn can_open_sqlite_store() {
        let store = LocalStore::open("sqlite::memory:").await.unwrap();
        let now = Utc::now();

        let channel = Channel {
            id: ChannelId(Uuid::new_v4()),
            name: "Test Channel".to_string(),
            brand: BrandProfile {
                name: "Test Channel".to_string(),
                tagline: "AI first".to_string(),
                tone: "professional".to_string(),
            },
            style: StyleProfile {
                art_style: "explainer".to_string(),
                color_palette: vec!["#000000".to_string()],
                camera_style: "center".to_string(),
                pacing: "medium".to_string(),
            },
            created_at: now,
            updated_at: now,
        };

        store.save_channel(&channel).await.unwrap();
        let saved = store.list_channels().await.unwrap();
        assert_eq!(saved.len(), 1);
    }

    #[tokio::test]
    async fn can_persist_job_to_storage() {
        let store = LocalStore::open("sqlite::memory:").await.unwrap();
        let project_id = ProjectId(Uuid::new_v4());
        let job = animatrix_jobs::JobManager::new_scene_job(project_id);

        store.save_job(&job).await.unwrap();

        let row = sqlx::query("SELECT id FROM jobs WHERE id = ?")
            .bind(job.id.0.to_string())
            .fetch_one(&store.pool)
            .await
            .unwrap();

        assert!(row.get::<String, _>("id").len() > 0);
    }

    #[tokio::test]
    async fn can_persist_asset_to_storage() {
        let store = LocalStore::open("sqlite::memory:").await.unwrap();
        let now = Utc::now();

        let asset = animatrix_assets::Asset {
            id: animatrix_assets::AssetId(Uuid::new_v4()),
            project_id: ProjectId(Uuid::new_v4()),
            channel_id: ChannelId(Uuid::new_v4()),
            asset_type: animatrix_assets::AssetType::Image,
            source: animatrix_assets::AssetSource::Generated,
            provider: None,
            model: None,
            path: "/tmp/test.png".to_string(),
            hash: "sha256-test".to_string(),
            mime_type: "image/png".to_string(),
            size_bytes: 1024,
            metadata: serde_json::json!({}),
            created_at: now,
            parent_asset: None,
        };

        store.save_asset(&asset).await.unwrap();

        let row = sqlx::query("SELECT id FROM assets WHERE id = ?")
            .bind(asset.id.0.to_string())
            .fetch_one(&store.pool)
            .await
            .unwrap();

        assert!(row.get::<String, _>("id").len() > 0);
    }
}
