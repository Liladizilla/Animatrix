use animatrix_core::{AppError, AppResult, ChannelId, ProjectId};
use animatrix_domain::{BrandProfile, Channel, Project, StyleProfile};
use serde::{Deserialize, Serialize};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool, Row};
use std::path::PathBuf;

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
        .execute(&pool)
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
        .execute(&pool)
        .await
        .map_err(|e| AppError::new("db_init", e.to_string()))?;

        Ok(Self {
            pool,
            config: config.clone(),
        })
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
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
    use uuid::Uuid;

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
}
