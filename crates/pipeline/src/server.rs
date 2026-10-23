use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;
use axum::extract::{Path, Query, Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::{self, Next};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, patch, post};
use axum::{Json, Router};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::BroadcastStream;
use tower_http::cors::CorsLayer;
use leadfinder_core::{CoreEvent, Stage, VideoFilter};
use crate::engine::PipelineContext;

#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    pub token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PasteRequest {
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct ImportRequest {
    pub content: String,
    #[serde(default = "default_import_label")]
    pub label: String,
}

fn default_import_label() -> String {
    "api".to_string()
}

#[derive(Debug, Deserialize)]
pub struct DiscoverTriggerRequest {
    #[serde(default = "default_max_per_seed")]
    pub max_per_seed: usize,
}

fn default_max_per_seed() -> usize {
    10
}

#[derive(Debug, Deserialize)]
pub struct VideoListQuery {
    pub lane: Option<String>,
    pub state: Option<String>,
    pub verdict: Option<String>,
    pub label: Option<String>,
    pub creator_id: Option<String>,
    pub q: Option<String>,
    pub min_score: Option<f64>,
    pub min_views: Option<i64>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct DecisionRequest {
    pub action: String,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TranscriptEditRequest {
    pub edited_text: String,
    #[serde(default)]
    pub rerun_claims: bool,
}

#[derive(Debug, Deserialize)]
pub struct SeedUpsertRequest {
    pub kind: String,
    pub value: String,
    pub topic: Option<String>,
    #[serde(default = "default_true_flag")]
    pub active: bool,
}

fn default_true_flag() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub struct WatchlistRequest {
    pub handle: String,
    pub watchlist: bool,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub version: &'static str,
    pub region: String,
    pub automated_discovery_enabled: bool,
    pub discovery_paused: bool,
    pub breakers: Vec<leadfinder_core::HostBreakerStatus>,
    pub queue: serde_json::Value,
    pub capacity_per_day: Option<f64>,
    pub cloud_enabled: bool,
}

pub fn build_router(ctx: Arc<PipelineContext>) -> Router {
    let protected = Router::new()
        .route("/api/health", get(handle_health))
        .route("/api/events", get(handle_events))
        .route("/api/inbox/paste", post(handle_paste))
        .route("/api/inbox/import", post(handle_import))
        .route("/api/discover/run", post(handle_discover_run))
        .route("/api/videos", get(handle_list_videos))
        .route("/api/videos/:id", get(handle_video_detail))
        .route("/api/videos/:id/decision", post(handle_decision))
        .route("/api/videos/:id/transcript", patch(handle_edit_transcript))
        .route("/api/seeds", get(handle_list_seeds).post(handle_upsert_seed))
        .route("/api/seeds/:id", delete(handle_delete_seed))
        .route("/api/creators", get(handle_list_creators))
        .route("/api/creators/watchlist", post(handle_set_watchlist))
        .route("/api/profile", get(handle_get_profile))
        .route("/api/settings", get(handle_list_settings).post(handle_update_settings))
        .route_layer(middleware::from_fn_with_state(ctx.clone(), auth_middleware));

    Router::new()
        .route("/metrics", get(handle_metrics))
        .merge(protected)
        .layer(CorsLayer::permissive())
        .with_state(ctx)
}

async fn auth_middleware(
    State(ctx): State<Arc<PipelineContext>>,
    Query(tq): Query<TokenQuery>,
    req: Request,
    next: Next,
) -> Response {
    let header_token = req
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .map(|s| s.trim());

    let provided = header_token.or_else(|| tq.token.as_deref());
    if provided != Some(ctx.api_token.as_str()) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({ "error": "unauthorized" })),
        )
            .into_response();
    }

    next.run(req).await
}

async fn handle_health(State(ctx): State<Arc<PipelineContext>>) -> impl IntoResponse {
    let cfg = ctx.config.read().await.clone();
    let breakers = ctx.guard.breaker_statuses().await;
    let discovery_paused = breakers.iter().any(|b| b.is_open);
    let queue = ctx.db.queue_counts().await.unwrap_or_else(|_| serde_json::json!({}));
    let capacity_per_day = ctx
        .db
        .get_profile()
        .await
        .ok()
        .flatten()
        .and_then(|p| p.capacity_per_day);

    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        region: cfg.discovery.region,
        automated_discovery_enabled: cfg.discovery.automated_discovery_enabled,
        discovery_paused,
        breakers,
        queue,
        capacity_per_day,
        cloud_enabled: cfg.providers.cloud_enabled,
    })
}

async fn handle_events(
    State(ctx): State<Arc<PipelineContext>>,
) -> Sse<impl futures::Stream<Item = Result<Event, Infallible>>> {
    let rx = ctx.events_tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|msg| async move {
        match msg {
            Ok(ev) => {
                let payload = serde_json::to_string(&ev).unwrap_or_else(|_| "{}".to_string());
                Some(Ok(Event::default().data(payload)))
            }
            Err(_) => None,
        }
    });
    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(10)).text("keepalive"))
}

async fn handle_paste(
    State(ctx): State<Arc<PipelineContext>>,
    Json(body): Json<PasteRequest>,
) -> Response {
    match ctx.paste_urls(&body.text).await {
        Ok(videos) => Json(serde_json::json!({
            "ingested": videos.len(),
            "videos": videos
        }))
        .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn handle_import(
    State(ctx): State<Arc<PipelineContext>>,
    Json(body): Json<ImportRequest>,
) -> Response {
    match ctx.import_partner_content(&body.content, &body.label).await {
        Ok(videos) => Json(serde_json::json!({
            "ingested": videos.len(),
            "videos": videos
        }))
        .into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn handle_discover_run(
    State(ctx): State<Arc<PipelineContext>>,
    Json(body): Json<DiscoverTriggerRequest>,
) -> Response {
    match ctx.run_automated_discovery(body.max_per_seed).await {
        Ok(summary) => Json(summary).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn handle_list_videos(
    State(ctx): State<Arc<PipelineContext>>,
    Query(q): Query<VideoListQuery>,
) -> Response {
    let filter = VideoFilter {
        lane: q.lane,
        state: q.state,
        verdict: q.verdict,
        label: q.label,
        creator_id: q.creator_id,
        search_query: q.q,
        min_score: q.min_score,
        min_views: q.min_views,
        limit: q.limit,
        offset: q.offset,
    };
    match ctx.db.list_lead_cards(&filter).await {
        Ok(cards) => {
            let cap = ctx.config.read().await.funnel.daily_lead_cap;
            Json(serde_json::json!({
                "daily_cap": cap,
                "total": cards.len(),
                "cards": cards
            }))
            .into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn handle_video_detail(
    State(ctx): State<Arc<PipelineContext>>,
    Path(id): Path<String>,
) -> Response {
    match ctx.db.get_video_detail(&id).await {
        Ok(Some(detail)) => Json(detail).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "video not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn handle_decision(
    State(ctx): State<Arc<PipelineContext>>,
    Path(id): Path<String>,
    Json(body): Json<DecisionRequest>,
) -> Response {
    if !matches!(body.action.as_str(), "keep" | "discard" | "research_more" | "undo") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "invalid decision action" })),
        )
            .into_response();
    }
    match ctx.db.log_decision(&id, &body.action, body.note.as_deref()).await {
        Ok(dec) => {
            if body.action == "research_more" {
                if let Ok(job) = ctx.db.enqueue_job(Some(&id), Stage::ResearchMore).await {
                    ctx.emit(CoreEvent::JobUpdated {
                        job_id: job.id,
                        video_id: Some(id.clone()),
                        stage: job.stage,
                        status: job.status,
                    });
                }
            }
            ctx.emit(CoreEvent::DecisionLogged {
                video_id: id,
                action: body.action,
            });
            Json(dec).into_response()
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn handle_edit_transcript(
    State(ctx): State<Arc<PipelineContext>>,
    Path(id): Path<String>,
    Json(body): Json<TranscriptEditRequest>,
) -> Response {
    if let Err(e) = ctx.db.update_transcript_edited_text(&id, &body.edited_text).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response();
    }
    if body.rerun_claims {
        if let Ok(job) = ctx.db.enqueue_job(Some(&id), Stage::Claims).await {
            ctx.emit(CoreEvent::JobUpdated {
                job_id: job.id,
                video_id: Some(id.clone()),
                stage: job.stage,
                status: job.status,
            });
        }
    }
    Json(serde_json::json!({ "ok": true, "video_id": id })).into_response()
}

async fn handle_list_seeds(State(ctx): State<Arc<PipelineContext>>) -> Response {
    match ctx.db.list_seeds(false).await {
        Ok(seeds) => Json(seeds).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn handle_upsert_seed(
    State(ctx): State<Arc<PipelineContext>>,
    Json(body): Json<SeedUpsertRequest>,
) -> Response {
    match ctx
        .db
        .upsert_seed(&body.kind, &body.value, body.topic.as_deref(), body.active)
        .await
    {
        Ok(seed) => Json(seed).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn handle_delete_seed(
    State(ctx): State<Arc<PipelineContext>>,
    Path(id): Path<i64>,
) -> Response {
    match ctx.db.delete_seed(id).await {
        Ok(()) => Json(serde_json::json!({ "deleted": id })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn handle_list_creators(State(ctx): State<Arc<PipelineContext>>) -> Response {
    match ctx.db.list_creators().await {
        Ok(creators) => Json(creators).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn handle_set_watchlist(
    State(ctx): State<Arc<PipelineContext>>,
    Json(body): Json<WatchlistRequest>,
) -> Response {
    match ctx.db.set_creator_watchlist(&body.handle, body.watchlist).await {
        Ok(creator) => Json(creator).into_response(),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn handle_get_profile(State(ctx): State<Arc<PipelineContext>>) -> Response {
    match ctx.db.get_profile().await {
        Ok(prof) => Json(prof).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn handle_list_settings(State(ctx): State<Arc<PipelineContext>>) -> Response {
    match ctx.db.list_settings().await {
        Ok(map) => Json(map).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn handle_update_settings(
    State(ctx): State<Arc<PipelineContext>>,
    Json(body): Json<serde_json::Map<String, serde_json::Value>>,
) -> Response {
    for (k, v) in &body {
        if let Err(e) = ctx.db.set_setting(k, v).await {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
        if k == "cloud_enabled" {
            if let Some(flag) = v.as_bool() {
                ctx.config.write().await.providers.cloud_enabled = flag;
            }
        }
    }
    Json(serde_json::json!({ "updated": body.len() })).into_response()
}

async fn handle_metrics(State(ctx): State<Arc<PipelineContext>>) -> impl IntoResponse {
    let queue = ctx.db.queue_counts().await.unwrap_or_else(|_| serde_json::json!({}));
    let breakers = ctx.guard.breaker_statuses().await;
    let open_breakers = breakers.iter().filter(|b| b.is_open).count();

    let queued = queue.get("queued").and_then(|v| v.as_i64()).unwrap_or(0);
    let running = queue.get("running").and_then(|v| v.as_i64()).unwrap_or(0);
    let succeeded = queue.get("succeeded").and_then(|v| v.as_i64()).unwrap_or(0);
    let failed = queue.get("failed").and_then(|v| v.as_i64()).unwrap_or(0);

    let text = format!(
        "leadfinder_jobs_queued {queued}\nleadfinder_jobs_running {running}\nleadfinder_jobs_succeeded {succeeded}\nleadfinder_jobs_failed {failed}\nleadfinder_circuit_breakers_open {open_breakers}\n"
    );
    ([(header::CONTENT_TYPE, "text/plain; version=0.0.4")], text)
}
