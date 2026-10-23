pub mod claims;
pub mod concurrency;
pub mod engine;
pub mod funnel;
pub mod gold;
pub mod janitor;
pub mod quota;
pub mod server;
pub mod soak;
pub mod spike;
pub mod verify;

pub use claims::*;
pub use concurrency::*;
pub use engine::*;
pub use funnel::*;
pub use gold::*;
pub use janitor::*;
pub use quota::*;
pub use server::*;
pub use soak::*;
pub use spike::*;
pub use verify::*;

pub fn crate_ready() -> &'static str {
    leadfinder_probe::crate_ready()
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use leadfinder_core::RuntimePaths;
    use tokio::net::TcpListener;

    #[tokio::test]
    async fn axum_api_enforces_token_auth_and_serves_health_and_sse() {
        let temp = tempfile::tempdir().unwrap();
        let config_dir = temp.path().join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        let repo_cfg = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("config")
            .join("default.toml");
        std::fs::copy(&repo_cfg, config_dir.join("default.toml")).unwrap();

        let paths = RuntimePaths {
            static_root: temp.path().to_path_buf(),
            mutable_root: temp.path().to_path_buf(),
            config_dir,
            data_dir: temp.path().join("data"),
            db_path: temp.path().join("data").join("test.db"),
            log_dir: temp.path().join("logs"),
            cache_dir: temp.path().join("cache"),
            prompts_dir: temp.path().join("prompts"),
            assets_dir: temp.path().join("assets"),
            tools_dir: temp.path().join("tools"),
            bundled_tools_dir: temp.path().join("tools"),
            models_dir: temp.path().join("models"),
        };

        let ctx = std::sync::Arc::new(PipelineContext::init_with_paths(paths).await.unwrap());
        let app = build_router(ctx.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let client = reqwest::Client::new();
        let unauth = client
            .get(format!("http://{addr}/api/health"))
            .send()
            .await
            .unwrap();
        assert_eq!(unauth.status(), StatusCode::UNAUTHORIZED);

        let auth = client
            .get(format!("http://{addr}/api/health"))
            .bearer_auth(&ctx.api_token)
            .send()
            .await
            .unwrap();
        assert_eq!(auth.status(), StatusCode::OK);
        let body: serde_json::Value = auth.json().await.unwrap();
        assert_eq!(body.get("status").and_then(|v| v.as_str()), Some("ok"));
    }

    #[test]
    fn splits_every_attribution_trap_and_excludes_opinions_from_checkworthy() {
        use leadfinder_core::{load_app_config, ClaimInput, ClaimType, RuntimePaths};
        use leadfinder_providers::global_lexicon;

        let paths = RuntimePaths::discover().unwrap();
        let cfg = load_app_config(&paths.config_dir).unwrap();
        let lex = global_lexicon(&paths, &cfg.normalisation.lexicon_config_file);
        let verbs = lex.attribution_verbs();

        let traps = [
            ("Бойко Борисов се съгласи България да приеме 50 000 мигранти.", "agreed"),
            ("Румен Радев заяви вчера, че България напуска НАТО.", "said"),
            ("Министърът на финансите предложи нов 30-процентов данък върху спестяванията.", "proposed"),
            ("Народното събрание гласува за забрана на домашните градини.", "voted"),
            ("Димитър Радев отрече левовете да се обменят по фиксирания курс.", "denied"),
            ("Костадин Костадинов се бил договорил с посолството да оттегли искането.", "agreed"),
        ];

        for (sentence, expected_act) in traps {
            let mut claim = ClaimInput {
                text_bg: sentence.to_string(),
                claim_type: ClaimType::Factual,
                speaker: None,
                attribution: Vec::new(),
                start_ms: 1000,
                end_ms: 6000,
                checkworthiness: 0.85,
            };
            split_and_normalize_attribution(&mut claim, Some("creator_x"), &verbs);
            assert_eq!(claim.claim_type, ClaimType::Attribution);
            assert_eq!(claim.attribution.len(), 1);
            assert_eq!(claim.attribution[0].act, expected_act);
            assert!(claim.attribution[0].asserted_part.as_deref().unwrap_or("").contains("creator_x"));
            assert!(claim.attribution[0].attributed_part.as_deref().unwrap_or("").contains(expected_act));
        }

        let op_score = compute_combined_checkworthiness(
            0.90,
            "Според мен това е пълна глупост!",
            "opinion",
            "Споделете преди да го изтрият!",
            true,
            &cfg.ranking,
        );
        assert!(op_score < cfg.ranking.checkworthy_threshold);

        let stat_score = compute_combined_checkworthiness(
            0.80,
            "Инфлацията при лекарствата скочи с 25 процента през март.",
            "statistical",
            "Споделете това видео!",
            true,
            &cfg.ranking,
        );
        assert!(stat_score >= cfg.ranking.checkworthy_threshold);
    }

    #[tokio::test]
    async fn crash_recovery_resumes_without_duplicate_work() {
        let temp = tempfile::tempdir().unwrap();
        let db_path = temp.path().join("test.db");
        let db = leadfinder_core::Database::connect(&db_path).await.unwrap();

        for i in 0..5 {
            let vid = format!("test_vid_{i}");
            let vref = leadfinder_core::VideoRef {
                id: vid.clone(),
                url: format!("https://www.tiktok.com/@user/video/{vid}"),
                creator_id: Some("user".to_string()),
                creator_handle: Some("user".to_string()),
                creator_display_name: Some("User".to_string()),
                creator_follower_count: Some(1000),
                creator_region: Some("BG".to_string()),
                caption: Some("Тест #българия".to_string()),
                hashtags: vec!["българия".to_string()],
                posted_at: Some("2026-10-23T10:00:00Z".to_string()),
                views: Some(15000),
                likes: Some(100),
                shares: Some(10),
                comments: Some(5),
                duration_s: Some(15.0),
                sound_title: None,
                subtitle_urls: Vec::new(),
                discovered_via: "paste".to_string(),
            };
            db.upsert_video_discovered(&vref, 0).await.unwrap();
            let job = db.enqueue_job(Some(&vid), leadfinder_core::Stage::ScreenCheap).await.unwrap();
            if i < 3 {
                sqlx::query("UPDATE jobs SET status = 'running', attempts = 1 WHERE id = ?1")
                    .bind(job.id)
                    .execute(db.pool())
                    .await
                    .unwrap();
            }
        }

        let recovered = db.recover_running_jobs().await.unwrap();
        assert_eq!(recovered, 3);

        let counts = db.queue_counts().await.unwrap();
        assert_eq!(counts.get("queued").and_then(|v| v.as_i64()), Some(5));
        assert_eq!(counts.get("running").and_then(|v| v.as_i64()), None);

        for _ in 0..5 {
            let claimed = db.claim_next_job(None).await.unwrap().unwrap();
            assert_eq!(claimed.attempts, 1);
            db.complete_job(claimed.id).await.unwrap();
        }

        let final_counts = db.queue_counts().await.unwrap();
        assert_eq!(final_counts.get("succeeded").and_then(|v| v.as_i64()), Some(5));
        assert_eq!(final_counts.get("queued").and_then(|v| v.as_i64()), None);
    }

    #[tokio::test]
    async fn metrics_endpoint_exposes_aimd_and_scale_stats() {
        let temp = tempfile::tempdir().unwrap();
        let config_dir = temp.path().join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        let repo_cfg = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("config")
            .join("default.toml");
        std::fs::copy(&repo_cfg, config_dir.join("default.toml")).unwrap();

        let paths = RuntimePaths {
            static_root: temp.path().to_path_buf(),
            mutable_root: temp.path().to_path_buf(),
            config_dir,
            data_dir: temp.path().join("data"),
            db_path: temp.path().join("data").join("test.db"),
            log_dir: temp.path().join("logs"),
            cache_dir: temp.path().join("cache"),
            prompts_dir: temp.path().join("prompts"),
            assets_dir: temp.path().join("assets"),
            tools_dir: temp.path().join("tools"),
            bundled_tools_dir: temp.path().join("tools"),
            models_dir: temp.path().join("models"),
        };

        let ctx = std::sync::Arc::new(PipelineContext::init_with_paths(paths).await.unwrap());
        let app = build_router(ctx.clone());
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        let client = reqwest::Client::new();
        let resp = client.get(format!("http://{addr}/metrics")).send().await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let text = resp.text().await.unwrap();
        assert!(text.contains("leadfinder_jobs_queued"));
        assert!(text.contains("leadfinder_aimd_concurrency"));
        assert!(text.contains("leadfinder_process_memory_rss_bytes"));
        assert!(text.contains("leadfinder_disk_free_bytes"));
        assert!(text.contains("leadfinder_backpressure_active"));

        let scale_resp = client
            .get(format!("http://{addr}/api/scale/status"))
            .bearer_auth(&ctx.api_token)
            .send()
            .await
            .unwrap();
        assert_eq!(scale_resp.status(), StatusCode::OK);
        let scale_json: serde_json::Value = scale_resp.json().await.unwrap();
        assert!(scale_json.get("aimd").is_some());
        assert!(scale_json.get("rss_bytes").is_some());
    }
}
