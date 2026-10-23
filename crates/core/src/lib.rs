pub mod config;
pub mod db;
pub mod db_funnel;
pub mod models;
pub mod politeness;
pub mod traits;

pub use config::*;
pub use db::*;
pub use models::*;
pub use politeness::*;
pub use traits::*;

pub fn crate_ready() -> &'static str {
    "leadfinder-core"
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn sqlite_wal_and_durable_jobs_lifecycle() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let db = Database::connect(&db_path).await.unwrap();

        assert_eq!(db.journal_mode().await.unwrap().to_lowercase(), "wal");

        let vref = VideoRef {
            id: "7300000000000000001".to_string(),
            url: "https://www.tiktok.com/@testuser/video/7300000000000000001".to_string(),
            creator_id: Some("creator_1".to_string()),
            creator_handle: Some("testuser".to_string()),
            creator_display_name: Some("Test User".to_string()),
            creator_follower_count: Some(12500),
            creator_region: Some("BG".to_string()),
            caption: Some("Тестово видео #българия".to_string()),
            hashtags: vec!["българия".to_string()],
            posted_at: Some("2026-10-20T12:00:00Z".to_string()),
            views: Some(15000),
            likes: Some(400),
            shares: Some(80),
            comments: Some(55),
            duration_s: Some(32.5),
            sound_title: None,
            subtitle_urls: Vec::new(),
            discovered_via: "paste".to_string(),
        };

        let (stored, created) = db.upsert_video_discovered(&vref, 0).await.unwrap();
        assert!(created);
        assert_eq!(stored.state, "DISCOVERED");

        let job1 = db.enqueue_job(Some(&stored.id), Stage::Acquire).await.unwrap();
        let job_dup = db.enqueue_job(Some(&stored.id), Stage::Acquire).await.unwrap();
        assert_eq!(job1.id, job_dup.id);

        let claimed = db.claim_next_job(Some(Stage::Acquire)).await.unwrap().unwrap();
        assert_eq!(claimed.attempts, 1);
        assert_eq!(claimed.status, "running");

        let recovered = db.recover_running_jobs().await.unwrap();
        assert_eq!(recovered, 1);

        let claimed_again = db.claim_next_job(Some(Stage::Acquire)).await.unwrap().unwrap();
        assert_eq!(claimed_again.attempts, 1);

        let retried = db
            .fail_or_retry_job(&claimed_again, "network hiccup", 1, true)
            .await
            .unwrap();
        assert!(!retried);

        let failed_video = db.get_video(&stored.id).await.unwrap().unwrap();
        assert_eq!(failed_video.state, "FAILED");
        assert_eq!(failed_video.state_reason.as_deref(), Some("network hiccup"));
    }

    #[tokio::test]
    async fn circuit_breaker_opens_after_configured_failures() {
        let dir = tempfile::tempdir().unwrap();
        let policy = PolitenessPolicy {
            requests_per_minute: 600,
            breaker_failures: 3,
            breaker_cooldown: Duration::from_secs(60),
            cache_ttl: Duration::from_secs(300),
        };
        let guard = PolitenessGuard::new(policy, dir.path().to_path_buf());

        assert!(!guard.record_failure("www.tiktok.com", "HTTP 403").await);
        assert!(!guard.record_failure("www.tiktok.com", "HTTP 403").await);
        assert!(guard.record_failure("www.tiktok.com", "HTTP 429").await);

        let err = guard.acquire_slot("www.tiktok.com", "test_source").await.unwrap_err();
        assert_eq!(err.kind, ProviderErrorKind::CircuitOpen);
    }
}
