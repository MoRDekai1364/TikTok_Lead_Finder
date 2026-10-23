pub mod engine;
pub mod janitor;
pub mod server;
pub mod spike;

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
}
