use std::collections::HashMap;
use anyhow::Result;
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use leadfinder_core::{Database, Job, ProviderError, ProviderErrorKind};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuotaDecision {
    Proceed,
    Fallback { next_provider: String },
    Defer { run_after: String, reason: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCooldown {
    pub provider: String,
    pub consecutive_429s: u32,
    pub cooldown_until: DateTime<Utc>,
}

pub struct ProviderQuotaGuard {
    cooldowns: RwLock<HashMap<String, ProviderCooldown>>,
}

impl ProviderQuotaGuard {
    pub fn new() -> Self {
        Self {
            cooldowns: RwLock::new(HashMap::new()),
        }
    }

    pub async fn is_in_cooldown(&self, provider: &str) -> bool {
        let now = Utc::now();
        let map = self.cooldowns.read().await;
        if let Some(cd) = map.get(provider) {
            cd.cooldown_until > now
        } else {
            false
        }
    }

    pub async fn record_success(&self, provider: &str) {
        let mut map = self.cooldowns.write().await;
        map.remove(provider);
    }

    pub async fn handle_error(
        &self,
        provider: &str,
        model: &str,
        err: &ProviderError,
        chain: &[String],
        db: &Database,
        job: &Job,
    ) -> Result<QuotaDecision> {
        let is_quota = matches!(
            err.kind,
            ProviderErrorKind::RateLimited | ProviderErrorKind::QuotaExhausted
        );

        if !is_quota {
            return Ok(QuotaDecision::Proceed);
        }

        let _ = db
            .record_provider_usage(provider, model, 0, 0, true)
            .await;

        let now = Utc::now();
        let mut map = self.cooldowns.write().await;
        let entry = map.entry(provider.to_string()).or_insert_with(|| ProviderCooldown {
            provider: provider.to_string(),
            consecutive_429s: 0,
            cooldown_until: now,
        });

        entry.consecutive_429s += 1;
        let backoff_secs = match entry.consecutive_429s {
            1 => 30,
            2 => 120,
            3 => 300,
            _ => 900,
        };
        entry.cooldown_until = now + ChronoDuration::seconds(backoff_secs);

        tracing::warn!(
            event = "provider_quota_exhausted",
            provider = provider,
            model = model,
            consecutive_429s = entry.consecutive_429s,
            cooldown_seconds = backoff_secs,
            error = %err.message
        );

        let current_pos = chain.iter().position(|p| p == provider).unwrap_or(0);
        for next in &chain[current_pos + 1..] {
            if let Some(cd) = map.get(next) {
                if cd.cooldown_until > now {
                    continue;
                }
            }
            return Ok(QuotaDecision::Fallback {
                next_provider: next.clone(),
            });
        }

        let reset_at = calculate_next_reset_time();
        let reason = format!(
            "All LLM providers exhausted or rate-limited ({provider}: {err}). Deferred until reset."
        );

        db.defer_job_to(job.id, &reset_at, &reason).await?;

        tracing::info!(
            event = "job_deferred",
            job_id = job.id,
            run_after = %reset_at,
            reason = %reason
        );

        Ok(QuotaDecision::Defer {
            run_after: reset_at,
            reason,
        })
    }
}

pub fn calculate_next_reset_time() -> String {
    let now = Utc::now();
    let tomorrow = (now + ChronoDuration::days(1)).date_naive();
    let reset = tomorrow.and_hms_opt(0, 5, 0).unwrap();
    DateTime::<Utc>::from_naive_utc_and_offset(reset, Utc).to_rfc3339()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn quota_guard_tracks_cooldown_and_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::connect(&db_path).await.unwrap();

        let job = db
            .enqueue_job(None, leadfinder_core::Stage::Claims)
            .await
            .unwrap();

        let guard = ProviderQuotaGuard::new();
        let chain = vec![
            "local".to_string(),
            "gemini".to_string(),
            "heuristics".to_string(),
        ];

        let err = ProviderError::new(
            ProviderErrorKind::RateLimited,
            "local",
            "HTTP 429 Too Many Requests",
            true,
        );

        let decision = guard
            .handle_error("local", "model-bg", &err, &chain, &db, &job)
            .await
            .unwrap();

        assert_eq!(
            decision,
            QuotaDecision::Fallback {
                next_provider: "gemini".to_string()
            }
        );
        assert!(guard.is_in_cooldown("local").await);
    }

    #[tokio::test]
    async fn quota_guard_defers_when_chain_is_exhausted() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::connect(&db_path).await.unwrap();

        let job = db
            .enqueue_job(None, leadfinder_core::Stage::Claims)
            .await
            .unwrap();

        let guard = ProviderQuotaGuard::new();
        let chain = vec!["local".to_string()];

        let err = ProviderError::new(
            ProviderErrorKind::QuotaExhausted,
            "local",
            "daily quota exceeded",
            false,
        );

        let decision = guard
            .handle_error("local", "model-bg", &err, &chain, &db, &job)
            .await
            .unwrap();

        match decision {
            QuotaDecision::Defer { run_after, reason } => {
                assert!(!run_after.is_empty());
                assert!(reason.contains("Deferred until reset"));
            }
            _ => panic!("Expected deferral"),
        }

        let updated_job = db.get_job(job.id).await.unwrap().unwrap();
        assert_eq!(updated_job.status, "deferred");
        assert!(updated_job.run_after.is_some());
    }
}
