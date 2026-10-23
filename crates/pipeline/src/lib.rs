pub mod claims;
pub mod engine;
pub mod janitor;
pub mod server;
pub mod spike;

pub use claims::*;
pub use engine::*;
pub use janitor::*;
pub use server::*;
pub use spike::*;

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
}
