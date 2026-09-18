use animatrix_ai::{PolicyMode, Provider, ProviderRegistry, TaskKind};
use animatrix_core::{AppResult, ProviderKind, ProviderHealthSnapshot};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use tracing::{debug, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteRequest {
    pub task: TaskKind,
    pub preferred_provider: Option<ProviderKind>,
    pub policy: PolicyMode,
    pub prompt: String,
    pub cost_ceiling: Option<f64>,
    pub require_online: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteResult {
    pub provider: ProviderKind,
    pub provider_kind_str: String,
    pub task: TaskKind,
    pub confidence: f64,
    pub policy: PolicyMode,
}

pub struct ProviderHealthRegistry {
    snapshots: Arc<RwLock<Vec<ProviderHealthSnapshot>>>,
}

impl ProviderHealthRegistry {
    pub fn new() -> Self {
        Self {
            snapshots: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn with_snapshots(snapshots: Vec<ProviderHealthSnapshot>) -> Self {
        Self {
            snapshots: Arc::new(RwLock::new(snapshots)),
        }
    }

    pub fn update_snapshot(&self, snapshot: ProviderHealthSnapshot) {
        let mut guard = self.snapshots.write().unwrap();
        if let Some(existing) = guard.iter_mut().find(|s| s.provider == snapshot.provider) {
            *existing = snapshot;
        } else {
            guard.push(snapshot);
        }
    }

    pub fn get_snapshot(&self, provider: ProviderKind) -> Option<ProviderHealthSnapshot> {
        let guard = self.snapshots.read().unwrap();
        guard.iter().find(|s| s.provider == provider).cloned()
    }

    pub fn healthy_providers(&self) -> Vec<ProviderKind> {
        let guard = self.snapshots.read().unwrap();
        guard
            .iter()
            .filter(|s| matches!(s.health, animatrix_core::ProviderHealth::Healthy))
            .map(|s| s.provider)
            .collect()
    }
}

pub struct AiRouter {
    registry: ProviderRegistry,
    health: ProviderHealthRegistry,
}

impl AiRouter {
    pub fn new() -> Self {
        Self {
            registry: ProviderRegistry::default(),
            health: ProviderHealthRegistry::new(),
        }
    }

    pub fn with_health(registry: ProviderRegistry, health: ProviderHealthRegistry) -> Self {
        Self {
            registry,
            health,
        }
    }

    pub fn registry(&self) -> &ProviderRegistry {
        &self.registry
    }

    pub fn health(&self) -> &ProviderHealthRegistry {
        &self.health
    }

    pub async fn route(&self, request: &RouteRequest) -> AppResult<RouteResult> {
        let candidates = self.find_candidates(request.task.clone(), request.preferred_provider);

        if candidates.is_empty() {
            return Err(animatrix_core::AppError::new(
                "no_provider",
                format!("no provider supports task: {:?}", request.task),
            ));
        }

        let task = request.task.clone();
        let scored: Vec<(RouteResult, f64)> = candidates
            .iter()
            .map(|provider| {
                let score = self.score_provider(provider, request);
                let result = RouteResult {
                    provider: provider.provider_kind(),
                    provider_kind_str: format!("{:?}", provider.provider_kind()),
                    task: task.clone(),
                    confidence: score.max(0.0).min(100.0),
                    policy: request.policy.clone(),
                };
                (result, score)
            })
            .filter(|(_, score)| *score > 0.0)
            .collect();

        if scored.is_empty() {
            return Err(animatrix_core::AppError::new(
                "no_provider",
                format!("no provider passes constraints for task: {:?}", request.task),
            ));
        }

        let best = scored
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(r, _)| r.clone())
            .unwrap();

        debug!(
            provider = ?best.provider,
            confidence = best.confidence,
            "Routed request to provider"
        );

        Ok(best)
    }

    pub fn resolve(&self, preferred: Option<ProviderKind>) -> Box<dyn Provider + Send + Sync> {
        self.registry.resolve(preferred)
    }

    fn find_candidates(
        &self,
        task: TaskKind,
        preferred: Option<ProviderKind>,
    ) -> Vec<Box<dyn Provider + Send + Sync>> {
        let mut candidates = Vec::new();

        let provider_kinds = [
            ProviderKind::Local,
            ProviderKind::HuggingFace,
            ProviderKind::OpenRouter,
            ProviderKind::ComfyUi,
            ProviderKind::Groq,
            ProviderKind::Cerebras,
            ProviderKind::Together,
            ProviderKind::Fireworks,
            ProviderKind::DeepInfra,
            ProviderKind::Fal,
            ProviderKind::Replicate,
            ProviderKind::Custom,
        ];

        if let Some(provider_kind) = preferred {
            if let Some(provider) = self.try_resolve_with_health(provider_kind, task.clone()) {
                candidates.push(provider);
            }
            if candidates.is_empty() {
                for kind in provider_kinds {
                    if kind == provider_kind {
                        continue;
                    }
                    if let Some(provider) = self.try_resolve_with_health(kind, task.clone()) {
                        candidates.push(provider);
                    }
                }
            }
        } else {
            for kind in provider_kinds {
                if let Some(provider) = self.try_resolve_with_health(kind, task.clone()) {
                    candidates.push(provider);
                }
            }
        }

        candidates
    }

    fn try_resolve_with_health(
        &self,
        provider_kind: ProviderKind,
        task: TaskKind,
    ) -> Option<Box<dyn Provider + Send + Sync>> {
        let provider = self.registry.resolve(Some(provider_kind));
        if !provider.supports(task) {
            return None;
        }

        if let Some(health) = self.health.get_snapshot(provider_kind) {
            if health.health == animatrix_core::ProviderHealth::Unavailable
                || health.health == animatrix_core::ProviderHealth::AuthenticationFailed
            {
                warn!(provider = ?provider_kind, "Skipping unhealthy provider");
                return None;
            }
        }

        Some(provider)
    }

    fn score_provider(
        &self,
        provider: &Box<dyn Provider + Send + Sync>,
        request: &RouteRequest,
    ) -> f64 {
        let mut score = 50.0;
        let provider_kind = provider.provider_kind();

        if let Some(health) = self.health.get_snapshot(provider_kind) {
            match health.health {
                animatrix_core::ProviderHealth::Healthy => score += 20.0,
                animatrix_core::ProviderHealth::Degraded => score += 5.0,
                animatrix_core::ProviderHealth::RateLimited => score -= 20.0,
                _ => score -= 10.0,
            }
            if let Some(quota) = health.quota_remaining {
                if quota == 0 {
                    score -= 30.0;
                } else if quota < 100 {
                    score -= 10.0;
                }
            }
        }

        match request.policy {
            PolicyMode::LocalFirst => {
                if provider_kind == ProviderKind::Local {
                    score += 30.0;
                } else {
                    score -= 20.0;
                }
            }
            PolicyMode::Cheapest => {
                if provider_kind == ProviderKind::Local {
                    score += 25.0;
                }
                if provider_kind == ProviderKind::HuggingFace {
                    score += 10.0;
                }
            }
            PolicyMode::Fastest => {
                if let Some(latency) = self.health.get_snapshot(provider_kind).and_then(|s| s.p95_latency_ms) {
                    score += (500.0 - latency as f64).min(20.0).max(-20.0);
                }
            }
            PolicyMode::QualityFirst => {
                if provider_kind == ProviderKind::ComfyUi || provider_kind == ProviderKind::HuggingFace {
                    score += 15.0;
                }
            }
            PolicyMode::Balanced => {
                score += 5.0;
            }
            PolicyMode::UserSelected => {
                if provider_kind == request.preferred_provider.unwrap_or(ProviderKind::Local) {
                    score += 40.0;
                }
            }
            PolicyMode::FreeFirst => {
                if provider_kind == ProviderKind::Local || provider_kind == ProviderKind::HuggingFace {
                    score += 20.0;
                }
            }
        }

        if let Some(ceiling) = request.cost_ceiling {
            if ceiling < 0.01 {
                if provider_kind != ProviderKind::Local {
                    score -= 25.0;
                }
            }
        }

        score
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn router_defaults_to_local_for_unknown_task() {
        let router = AiRouter::new();
        let request = RouteRequest {
            task: TaskKind::Text,
            preferred_provider: None,
            policy: PolicyMode::Balanced,
            prompt: "Test".to_string(),
            cost_ceiling: None,
            require_online: false,
            created_at: Utc::now(),
        };

        let result = router.route(&request).await.unwrap();
        assert!(result.provider != ProviderKind::ComfyUi);
    }

    #[tokio::test]
    async fn router_returns_error_when_all_unhealthy() {
        let registry = ProviderRegistry::default();
        let health = ProviderHealthRegistry::with_snapshots(vec![
            ProviderHealthSnapshot { provider: ProviderKind::Local, health: animatrix_core::ProviderHealth::Unavailable, last_probe_at: Some(Utc::now()), p95_latency_ms: None, failure_rate: 1.0, quota_remaining: None, rate_limit_reset_at: None },
            ProviderHealthSnapshot { provider: ProviderKind::HuggingFace, health: animatrix_core::ProviderHealth::Unavailable, last_probe_at: Some(Utc::now()), p95_latency_ms: None, failure_rate: 1.0, quota_remaining: None, rate_limit_reset_at: None },
            ProviderHealthSnapshot { provider: ProviderKind::OpenRouter, health: animatrix_core::ProviderHealth::Unavailable, last_probe_at: Some(Utc::now()), p95_latency_ms: None, failure_rate: 1.0, quota_remaining: None, rate_limit_reset_at: None },
            ProviderHealthSnapshot { provider: ProviderKind::ComfyUi, health: animatrix_core::ProviderHealth::Unavailable, last_probe_at: Some(Utc::now()), p95_latency_ms: None, failure_rate: 1.0, quota_remaining: None, rate_limit_reset_at: None },
            ProviderHealthSnapshot { provider: ProviderKind::Groq, health: animatrix_core::ProviderHealth::Unavailable, last_probe_at: Some(Utc::now()), p95_latency_ms: None, failure_rate: 1.0, quota_remaining: None, rate_limit_reset_at: None },
            ProviderHealthSnapshot { provider: ProviderKind::Cerebras, health: animatrix_core::ProviderHealth::Unavailable, last_probe_at: Some(Utc::now()), p95_latency_ms: None, failure_rate: 1.0, quota_remaining: None, rate_limit_reset_at: None },
            ProviderHealthSnapshot { provider: ProviderKind::Together, health: animatrix_core::ProviderHealth::Unavailable, last_probe_at: Some(Utc::now()), p95_latency_ms: None, failure_rate: 1.0, quota_remaining: None, rate_limit_reset_at: None },
            ProviderHealthSnapshot { provider: ProviderKind::Fireworks, health: animatrix_core::ProviderHealth::Unavailable, last_probe_at: Some(Utc::now()), p95_latency_ms: None, failure_rate: 1.0, quota_remaining: None, rate_limit_reset_at: None },
            ProviderHealthSnapshot { provider: ProviderKind::DeepInfra, health: animatrix_core::ProviderHealth::Unavailable, last_probe_at: Some(Utc::now()), p95_latency_ms: None, failure_rate: 1.0, quota_remaining: None, rate_limit_reset_at: None },
            ProviderHealthSnapshot { provider: ProviderKind::Fal, health: animatrix_core::ProviderHealth::Unavailable, last_probe_at: Some(Utc::now()), p95_latency_ms: None, failure_rate: 1.0, quota_remaining: None, rate_limit_reset_at: None },
            ProviderHealthSnapshot { provider: ProviderKind::Replicate, health: animatrix_core::ProviderHealth::Unavailable, last_probe_at: Some(Utc::now()), p95_latency_ms: None, failure_rate: 1.0, quota_remaining: None, rate_limit_reset_at: None },
            ProviderHealthSnapshot { provider: ProviderKind::Custom, health: animatrix_core::ProviderHealth::Unavailable, last_probe_at: Some(Utc::now()), p95_latency_ms: None, failure_rate: 1.0, quota_remaining: None, rate_limit_reset_at: None },
        ]);

        let router = AiRouter::with_health(registry, health);
        let request = RouteRequest {
            task: TaskKind::Text,
            preferred_provider: None,
            policy: PolicyMode::Balanced,
            prompt: "Test".to_string(),
            cost_ceiling: None,
            require_online: false,
            created_at: Utc::now(),
        };

        let result = router.route(&request).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn router_respects_preferred_provider() {
        let router = AiRouter::new();
        let request = RouteRequest {
            task: TaskKind::Image,
            preferred_provider: Some(ProviderKind::ComfyUi),
            policy: PolicyMode::QualityFirst,
            prompt: "Test image".to_string(),
            cost_ceiling: None,
            require_online: false,
            created_at: Utc::now(),
        };

        let result = router.route(&request).await.unwrap();
        assert_eq!(result.provider, ProviderKind::ComfyUi);
    }

    #[tokio::test]
    async fn router_avoids_unhealthy_provider() {
        let registry = ProviderRegistry::default();
        let health = ProviderHealthRegistry::with_snapshots(vec![ProviderHealthSnapshot {
            provider: ProviderKind::ComfyUi,
            health: animatrix_core::ProviderHealth::Unavailable,
            last_probe_at: Some(Utc::now()),
            p95_latency_ms: None,
            failure_rate: 1.0,
            quota_remaining: None,
            rate_limit_reset_at: None,
        }]);

        let router = AiRouter::with_health(registry, health);
        let request = RouteRequest {
            task: TaskKind::Image,
            preferred_provider: Some(ProviderKind::ComfyUi),
            policy: PolicyMode::QualityFirst,
            prompt: "Test image".to_string(),
            cost_ceiling: None,
            require_online: false,
            created_at: Utc::now(),
        };

        let result = router.route(&request).await.unwrap();
        assert!(result.provider != ProviderKind::ComfyUi);
    }

    #[test]
    fn health_registry_tracks_snapshots() {
        let health = ProviderHealthRegistry::new();
        let snapshot = ProviderHealthSnapshot {
            provider: ProviderKind::HuggingFace,
            health: animatrix_core::ProviderHealth::Healthy,
            last_probe_at: Some(Utc::now()),
            p95_latency_ms: Some(120),
            failure_rate: 0.0,
            quota_remaining: Some(950),
            rate_limit_reset_at: None,
        };

        health.update_snapshot(snapshot.clone());
        let fetched = health.get_snapshot(ProviderKind::HuggingFace);
        assert_eq!(fetched, Some(snapshot));
    }

    #[tokio::test]
    async fn route_result_has_confidence() {
        let router = AiRouter::new();
        let request = RouteRequest {
            task: TaskKind::Text,
            preferred_provider: Some(ProviderKind::Local),
            policy: PolicyMode::LocalFirst,
            prompt: "Test".to_string(),
            cost_ceiling: None,
            require_online: false,
            created_at: Utc::now(),
        };

        let result = router.route(&request).await.unwrap();
        assert!(result.confidence > 0.0);
        assert!(result.confidence <= 100.0);
        assert_eq!(result.policy, PolicyMode::LocalFirst);
    }
}
