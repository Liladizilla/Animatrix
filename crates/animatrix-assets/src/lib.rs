use animatrix_core::{AppResult, ChannelId, ProjectId, ProviderKind};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetId(pub Uuid);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetType {
    Image,
    Video,
    Audio,
    Voice,
    Music,
    Sfx,
    Thumbnail,
    Scene,
    Project,
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AssetSource {
    Local,
    Generated,
    Imported,
    Remote,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Asset {
    pub id: AssetId,
    pub project_id: ProjectId,
    pub channel_id: ChannelId,
    pub asset_type: AssetType,
    pub source: AssetSource,
    pub provider: Option<ProviderKind>,
    pub model: Option<String>,
    pub path: String,
    pub hash: String,
    pub mime_type: String,
    pub size_bytes: u64,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub parent_asset: Option<AssetId>,
}

pub struct AssetRegistry;

impl AssetRegistry {
    fn infer_mime_type(path: &Path) -> String {
        let extension = path.extension().and_then(|ext| ext.to_str()).unwrap_or("").to_ascii_lowercase();
        match extension.as_str() {
            "png" => "image/png".to_string(),
            "jpg" | "jpeg" => "image/jpeg".to_string(),
            "gif" => "image/gif".to_string(),
            "webp" => "image/webp".to_string(),
            "mp4" => "video/mp4".to_string(),
            "mov" => "video/quicktime".to_string(),
            "mp3" => "audio/mpeg".to_string(),
            "wav" => "audio/wav".to_string(),
            "txt" => "text/plain".to_string(),
            "json" => "application/json".to_string(),
            _ => "application/octet-stream".to_string(),
        }
    }

    pub fn register_asset(
        project_id: ProjectId,
        channel_id: ChannelId,
        asset_type: AssetType,
        source: AssetSource,
        path: impl AsRef<Path>,
    ) -> AppResult<Asset> {
        let path_buf = path.as_ref();
        let path_string = path_buf.to_string_lossy().to_string();
        let file_name = path_buf
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        if let Ok(metadata) = std::fs::metadata(path_buf) {
            let size_bytes = metadata.len();
            let mime_type = Self::infer_mime_type(path_buf);

            let asset = Asset {
                id: AssetId(Uuid::new_v4()),
                project_id,
                channel_id,
                asset_type,
                source,
                provider: None,
                model: None,
                path: path_string,
                hash: format!("sha256:{}", file_name),
                mime_type: mime_type.clone(),
                size_bytes,
                metadata: serde_json::json!({
                    "file_name": file_name,
                    "path": path_buf.display().to_string(),
                    "exists": true,
                    "mime_type": mime_type,
                    "size_bytes": size_bytes,
                }),
                created_at: Utc::now(),
                parent_asset: None,
            };
            return Ok(asset);
        }

        let asset = Asset {
            id: AssetId(Uuid::new_v4()),
            project_id,
            channel_id,
            asset_type,
            source,
            provider: None,
            model: None,
            path: path_string,
            hash: "sha256-placeholder".to_string(),
            mime_type: Self::infer_mime_type(path_buf),
            size_bytes: 0,
            metadata: serde_json::json!({
                "file_name": file_name,
                "path": path_buf.display().to_string(),
                "exists": false,
            }),
            created_at: Utc::now(),
            parent_asset: None,
        };
        Ok(asset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn asset_registration_builds_provenance() {
        let project_id = ProjectId(Uuid::new_v4());
        let channel_id = ChannelId(Uuid::new_v4());

        let asset = AssetRegistry::register_asset(
            project_id,
            channel_id,
            AssetType::Image,
            AssetSource::Generated,
            "/tmp/storyboard.png",
        )
        .unwrap();

        assert_eq!(asset.project_id, project_id);
        assert_eq!(asset.channel_id, channel_id);
        assert_eq!(asset.asset_type, AssetType::Image);
        assert_eq!(asset.source, AssetSource::Generated);
    }

    #[test]
    fn asset_registration_infers_file_metadata() {
        let project_id = ProjectId(Uuid::new_v4());
        let channel_id = ChannelId(Uuid::new_v4());
        let temp_path = std::env::temp_dir().join(format!("animatrix-asset-{}.txt", Uuid::new_v4()));
        std::fs::write(&temp_path, b"hello world").unwrap();

        let asset = AssetRegistry::register_asset(
            project_id,
            channel_id,
            AssetType::Scene,
            AssetSource::Generated,
            &temp_path,
        )
        .unwrap();

        assert_eq!(asset.size_bytes, 11);
        assert_eq!(asset.mime_type, "text/plain");
        assert_eq!(asset.metadata["file_name"], temp_path.file_name().unwrap().to_string_lossy().as_ref());

        let _ = std::fs::remove_file(temp_path);
    }
}
