use std::convert::Infallible;
use std::sync::atomic::Ordering;
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
use tokio::io::AsyncWriteExt;
use tokio_stream::wrappers::BroadcastStream;
use tower_http::cors::CorsLayer;
use leadfinder_core::{CoreEvent, Search, Stage, VideoFilter};
use leadfinder_probe::{apply_calibration_overrides, CalibrationOverrides};
use crate::engine::{CustomScanConfig, PipelineContext};

#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    pub token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ExportQuery {
    pub format: Option<String>,
    pub lane: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct PasteRequest {
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct OpenBrowserRequest {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct ListPeopleQuery {
    pub query: Option<String>,
    pub tag: Option<String>,
    pub category: Option<String>,
    pub sort_by: Option<String>,
    pub entity_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePersonRequest {
    pub canonical_name: String,
    pub role: Option<String>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub rating: Option<i64>,
    pub notes: Option<String>,
    pub public_figure: Option<bool>,
    pub entity_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePersonRequest {
    pub canonical_name: String,
    pub role: Option<String>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub rating: Option<i64>,
    pub notes: Option<String>,
    pub public_figure: Option<bool>,
    pub summary: Option<String>,
    pub detailed_profile_json: Option<String>,
    pub entity_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReassignVideoPersonRequest {
    pub new_person_id: Option<i64>,
    pub canonical_name: Option<String>,
    pub role: Option<String>,
    pub category: Option<String>,
    pub tags: Option<Vec<String>>,
    pub entity_type: Option<String>,
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
    #[serde(alias = "text")]
    pub edited_text: String,
    #[serde(default)]
    pub rerun_claims: bool,
}

#[derive(Debug, Deserialize)]
pub struct RetranscribeRequest {
    #[serde(default = "default_retranscribe_warm")]
    pub warm_start: bool,
    #[serde(default = "default_retranscribe_claims")]
    pub rerun_claims: bool,
}

fn default_retranscribe_warm() -> bool {
    true
}

fn default_retranscribe_claims() -> bool {
    true
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

#[derive(Debug, Deserialize, Default)]
pub struct CalibrateRequest {
    #[serde(default)]
    pub force_cpu: bool,
    pub power_source: Option<String>,
    #[serde(default)]
    pub force_rerun: bool,
}

#[derive(Debug, Serialize)]
pub struct CalibrateStatusResponse {
    pub is_running: bool,
    pub progress: leadfinder_probe::BenchmarkState,
    pub power: leadfinder_probe::PowerInfo,
    pub ac_profile: Option<leadfinder_core::HardwareProfile>,
    pub battery_profile: Option<leadfinder_core::HardwareProfile>,
    pub active_profile: Option<leadfinder_core::HardwareProfile>,
}

#[derive(Debug, Deserialize)]
pub struct SwitchPowerRequest {
    pub power_source: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeminiKeyInfo {
    pub index: usize,
    pub masked: String,
    pub is_active: bool,
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
    pub lane_counts: std::collections::HashMap<String, i64>,
}

pub fn build_router(ctx: Arc<PipelineContext>) -> Router {
    let protected = Router::new()
        .route("/api/health", get(handle_health))
        .route("/api/events", get(handle_events))
        .route("/api/inbox/paste", post(handle_paste))
        .route("/api/inbox/import", post(handle_import))
        .route("/api/discover/run", post(handle_discover_run))
        .route("/api/discover/scan", post(handle_discover_scan))
        .route("/api/rank/run", post(handle_rank_run))
        .route("/api/rank/pull-more", post(handle_pull_more))
        .route("/api/rank/tune", post(handle_tune))
        .route("/api/videos", get(handle_list_videos))
        .route("/api/videos/:id", get(handle_video_detail).delete(handle_delete_video))
        .route("/api/videos/:id/decision", post(handle_decision))
        .route("/api/videos/:id/transcript", patch(handle_edit_transcript))
        .route("/api/videos/:id/retranscribe", post(handle_retranscribe))
        .route("/api/videos/:id/ai-check", post(handle_ai_check_transcript))
        .route("/api/videos/:id/google-ai-fix", post(handle_google_ai_fix))
        .route("/api/videos/:id/google-audio-transcribe", post(handle_google_audio_transcribe))
        .route("/api/google-ai/query", post(handle_google_ai_query))
        .route("/api/evidence/preview", get(handle_evidence_preview))
        .route("/api/evidence/:id", delete(handle_delete_evidence))
        .route("/api/open-browser", post(handle_open_browser))
        .route("/api/people", get(handle_list_people).post(handle_create_person))
        .route("/api/people/auto-classify", post(handle_auto_classify_people))
        .route("/api/people/:id", get(handle_get_person).put(handle_update_person).delete(handle_delete_person))
        .route("/api/people/:id/dossier", get(handle_get_person_dossier))
        .route("/api/people/:id/research", post(handle_research_person))
        .route("/api/videos/:video_id/people/:person_id/reassign", post(handle_reassign_video_person))
        .route("/api/videos/:video_id/people/:person_id", delete(handle_remove_video_person))
        .route("/api/seeds", get(handle_list_seeds).post(handle_upsert_seed))
        .route("/api/seeds/:id", delete(handle_delete_seed))
        .route("/api/creators", get(handle_list_creators))
        .route("/api/creators/watchlist", post(handle_set_watchlist))
        .route("/api/creators/:handle/assess-risk", post(handle_assess_creator_risk))
        .route("/api/profile", get(handle_get_profile))
        .route("/api/calibrate", post(handle_calibrate))
        .route("/api/calibrate/status", get(handle_calibrate_status))
        .route("/api/calibrate/switch-power", post(handle_calibrate_switch_power))
        .route("/api/calibrate/overrides", post(handle_calibrate_overrides))
        .route("/api/settings", get(handle_list_settings).post(handle_update_settings))
        .route("/api/settings/gemini-status", get(handle_gemini_status))
        .route("/api/settings/gemini-key", post(handle_set_gemini_key).delete(handle_delete_gemini_key))
        .route("/api/settings/gemini-keys/rotate", post(handle_rotate_gemini_key))
        .route("/api/models", get(handle_list_models))
        .route("/api/models/catalog", get(handle_get_model_catalog))
        .route("/api/models/download", post(handle_download_model))
        .route("/api/models/download/status", get(handle_download_model_status))
        .route("/api/models/download/cancel", post(handle_download_model_cancel))
        .route("/api/models/:kind/:filename", delete(handle_delete_model))
        .route("/api/models/scan-folder", post(handle_scan_models_folder))
        .route("/api/models/external/scan", post(handle_scan_external_ai_servers))
        .route("/api/models/roles", get(handle_get_model_roles).post(handle_update_model_roles))
        .route("/api/models/test", post(handle_test_model))
        .route("/api/export", get(handle_export))
        .route("/api/scale/status", get(handle_scale_status))
        .route("/api/system/shutdown", post(handle_system_shutdown))
        .route("/api/system/shutdown-when-idle", post(handle_system_shutdown_when_idle))
        .route("/api/system/exit-status", get(handle_system_exit_status))
        .route_layer(middleware::from_fn_with_state(ctx.clone(), auth_middleware));

    tokio::spawn({
        let ctx = ctx.clone();
        async move {
            let mut last_source = leadfinder_probe::detect_power_info().power_source;
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                let current = leadfinder_probe::detect_power_info();
                if current.power_source != last_source {
                    let old = last_source.clone();
                    last_source = current.power_source.clone();
                    let prof = ctx.db.get_profile_for_power(&current.power_source).await.ok().flatten();
                    let has_profile = prof.is_some();
                    if let Some(ref p) = prof {
                        let _ = ctx.db.save_profile(p).await;
                    }
                    let _ = ctx.events_tx.send(leadfinder_core::CoreEvent::PowerChanged(serde_json::json!({
                        "power_source": current.power_source,
                        "old_source": old,
                        "new_source": current.power_source,
                        "ac_online": current.ac_online,
                        "battery_percent": current.battery_percent,
                        "has_profile": has_profile,
                        "capacity_per_day": prof.and_then(|p| p.capacity_per_day),
                    })));
                }
            }
        }
    });

    Router::new()
        .route("/metrics", get(handle_metrics))
        .nest_service("/api/media", tower_http::services::ServeDir::new(ctx.media_root()))
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
    let lane_counts = ctx.db.get_lane_counts().await.unwrap_or_default();

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
        lane_counts,
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
        Ok(videos) => {
            ctx.spawn_metadata_enrichment(&videos);
            Json(serde_json::json!({
                "ingested": videos.len(),
                "videos": videos
            }))
            .into_response()
        }
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

#[derive(Debug, Deserialize)]
pub struct PullMoreRequest {
    pub count: Option<usize>,
}

#[derive(Debug, Deserialize)]
pub struct TuneRequest {
    pub confirm: Option<bool>,
}

async fn handle_rank_run(State(ctx): State<Arc<PipelineContext>>) -> Response {
    match ctx.rank_all().await {
        Ok(summary) => Json(summary).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn handle_pull_more(
    State(ctx): State<Arc<PipelineContext>>,
    Json(body): Json<PullMoreRequest>,
) -> Response {
    let count = body.count.unwrap_or(10).clamp(1, 200);
    match ctx.pull_more(count).await {
        Ok(promoted) => Json(serde_json::json!({
            "promoted": promoted,
            "pulled_count": promoted
        }))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn handle_tune(
    State(ctx): State<Arc<PipelineContext>>,
    Json(body): Json<TuneRequest>,
) -> Response {
    match ctx.tune_ranking(body.confirm.unwrap_or(false)).await {
        Ok(v) => Json(v).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
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

async fn handle_discover_scan(
    State(ctx): State<Arc<PipelineContext>>,
    Json(body): Json<CustomScanConfig>,
) -> Response {
    match ctx.scan_tiktok_custom(&body).await {
        Ok(result) => Json(result).into_response(),
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
            let lane_counts = ctx.db.get_lane_counts().await.unwrap_or_default();
            Json(serde_json::json!({
                "daily_cap": cap,
                "total": cards.len(),
                "lane_counts": lane_counts,
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

async fn handle_delete_video(
    State(ctx): State<Arc<PipelineContext>>,
    Path(id): Path<String>,
) -> Response {
    match ctx.db.delete_video(&id).await {
        Ok(()) => {
            ctx.emit(CoreEvent::VideoUpdated {
                video_id: id.clone(),
                state: "DELETED".to_string(),
                lane: None,
            });
            Json(serde_json::json!({ "ok": true, "deleted": id })).into_response()
        }
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
    if !matches!(body.action.as_str(), "keep" | "discard" | "research_more" | "undo" | "rerun_claims" | "rerun_extract" | "rerun_verify") {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "invalid decision action" })),
        )
            .into_response();
    }
    match ctx.db.log_decision(&id, &body.action, body.note.as_deref()).await {
        Ok(dec) => {
            if matches!(body.action.as_str(), "research_more" | "rerun_verify") {
                if let Ok(job) = ctx.db.enqueue_job(Some(&id), Stage::ResearchMore).await {
                    ctx.emit(CoreEvent::JobUpdated {
                        job_id: job.id,
                        video_id: Some(id.clone()),
                        stage: job.stage,
                        status: job.status,
                    });
                }
            } else if body.action == "rerun_claims" {
                if let Ok(job) = ctx.db.enqueue_job(Some(&id), Stage::Claims).await {
                    ctx.emit(CoreEvent::JobUpdated {
                        job_id: job.id,
                        video_id: Some(id.clone()),
                        stage: job.stage,
                        status: job.status,
                    });
                }
            } else if body.action == "rerun_extract" {
                if let Ok(job) = ctx.db.enqueue_job(Some(&id), Stage::Extract).await {
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
    if let Ok(Some(detail)) = ctx.db.get_video_detail(&id).await {
        Json(serde_json::to_value(detail).unwrap_or(serde_json::json!({ "ok": true, "video_id": id }))).into_response()
    } else {
        Json(serde_json::json!({ "ok": true, "video_id": id })).into_response()
    }
}

async fn handle_retranscribe(
    State(ctx): State<Arc<PipelineContext>>,
    Path(id): Path<String>,
    Json(body): Json<RetranscribeRequest>,
) -> Response {
    if !body.warm_start {
        match ctx.db.enqueue_job(Some(&id), Stage::Extract).await {
            Ok(job) => {
                ctx.emit(CoreEvent::JobUpdated {
                    job_id: job.id,
                    video_id: Some(id.clone()),
                    stage: job.stage,
                    status: job.status,
                });
                Json(serde_json::json!({
                    "ok": true,
                    "video_id": id,
                    "warm_start": false,
                    "enqueued": true
                }))
                .into_response()
            }
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response(),
        }
    } else {
        let tr_opt = match ctx.db.get_transcript(&id).await {
            Ok(t) => t,
            Err(e) => {
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(serde_json::json!({ "error": e.to_string() })),
                )
                    .into_response();
            }
        };

        let Some(tr) = tr_opt else {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({ "error": "Transcript not found" })),
            )
                .into_response();
        };

        let base_text = tr.edited_text.unwrap_or(tr.text);
        let cfg = ctx.config.read().await.clone();
        let lex = leadfinder_providers::global_lexicon(
            &ctx.paths,
            &cfg.normalisation.lexicon_config_file,
        );

        let stage1 = lex.normalize_text(&base_text);

        let system_prompt = "You are a specialized Bulgarian language transcription editor and fact-checking proofreader. You are given an automatic speech recognition (ASR) transcript in Bulgarian with phonetic errors, misrecognized Bulgarian names, distorted courtroom/political terminology, or dropped sounds. Restore the correct Bulgarian text: fix misheard names (e.g. Милен Цветков, Кристиан Николов, Софийски градски съд, прокуратура, затворническа администрация), correct phonetic misspellings, restore proper Bulgarian grammar, punctuation, and capitalization, while keeping the original spoken speech flow intact. Return JSON with a single field 'corrected_text' containing the clean corrected Bulgarian text.".to_string();
        let user_prompt = format!(" what would the fixed text look like?\n{stage1}");

        let runner = leadfinder_providers::LlmChainRunner::from_runtime(&ctx.paths, &cfg, &ctx.db).await;
        let schema_val = serde_json::json!({
            "type": "object",
            "properties": {
                "corrected_text": { "type": "string" }
            },
            "required": ["corrected_text"]
        });

        let llm_req = leadfinder_core::LlmJsonRequest {
            system_prompt,
            user_prompt,
            json_schema: Some(schema_val),
            max_tokens: 1024,
            temperature: 0.1,
        };

        let corrected = match runner.complete_validated(&llm_req, &ctx.db).await {
            Ok(val) => {
                val.response.parsed_json
                    .get("corrected_text")
                    .and_then(|v| v.as_str())
                    .filter(|s| !s.trim().is_empty())
                    .map(|s| s.trim().to_string())
                    .unwrap_or(stage1)
            }
            Err(_) => stage1,
        };

        let _ = ctx.db.update_transcript_edited_text(&id, &corrected).await;

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

        Json(serde_json::json!({
            "ok": true,
            "video_id": id,
            "corrected_text": corrected,
            "warm_start": true
        }))
        .into_response()
    }
}

async fn handle_ai_check_transcript(
    State(ctx): State<Arc<PipelineContext>>,
    Path(id): Path<String>,
) -> Response {
    let tr_opt = match ctx.db.get_transcript(&id).await {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    let Some(tr) = tr_opt else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Transcript not found" })),
        )
            .into_response();
    };

    let base_text = tr.edited_text.unwrap_or(tr.text);
    let cfg = ctx.config.read().await.clone();
    let lex = leadfinder_providers::global_lexicon(
        &ctx.paths,
        &cfg.normalisation.lexicon_config_file,
    );

    let stage1 = lex.normalize_text(&base_text);

    let system_prompt = "You are a specialized Bulgarian language transcription editor and fact-checking proofreader. You are given an automatic speech recognition (ASR) transcript in Bulgarian with phonetic errors, misrecognized Bulgarian names, distorted courtroom/political terminology, or dropped sounds. Restore the correct Bulgarian text: fix misheard names (e.g. Милен Цветков, Кристиан Николов, Софийски градски съд, прокуратура, затворническа администрация), correct phonetic misspellings, restore proper Bulgarian grammar, punctuation, and capitalization, while keeping the original spoken speech flow intact. Return JSON with a single field 'corrected_text' containing the clean corrected Bulgarian text.".to_string();
    let user_prompt = format!(" what would the fixed text look like?\n{stage1}");

    let runner = leadfinder_providers::LlmChainRunner::from_runtime(&ctx.paths, &cfg, &ctx.db).await;
    let schema_val = serde_json::json!({
        "type": "object",
        "properties": {
            "corrected_text": { "type": "string" }
        },
        "required": ["corrected_text"]
    });

    let llm_req = leadfinder_core::LlmJsonRequest {
        system_prompt,
        user_prompt,
        json_schema: Some(schema_val),
        max_tokens: 1024,
        temperature: 0.1,
    };

    let corrected = match runner.complete_validated(&llm_req, &ctx.db).await {
        Ok(val) => {
            val.response.parsed_json
                .get("corrected_text")
                .and_then(|v| v.as_str())
                .filter(|s| !s.trim().is_empty())
                .map(|s| s.trim().to_string())
                .unwrap_or(stage1)
        }
        Err(_) => stage1,
    };

    let _ = ctx.db.update_transcript_edited_text(&id, &corrected).await;

    Json(serde_json::json!({
        "ok": true,
        "video_id": id,
        "corrected_text": corrected
    }))
    .into_response()
}

fn resolve_headless_scraping_browser() -> Option<String> {
    if cfg!(target_os = "windows") {
        let candidates = [
            r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
            r"C:\Program Files\Google\Chrome\Application\chrome.exe",
            r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
        ];
        for c in candidates {
            if std::path::Path::new(c).exists() {
                return Some(c.to_string());
            }
        }
    }
    None
}

async fn run_google_ai_mode_fix(raw_text: &str) -> Option<String> {
    let prompt = format!("what would the fixed text look like? Output the edited result only:{}", raw_text.trim());

    let py_script = if std::path::Path::new("scripts/google_ai_search.py").exists() {
        Some("scripts/google_ai_search.py")
    } else if std::path::Path::new("tools/google_ai_search.py").exists() {
        Some("tools/google_ai_search.py")
    } else {
        None
    };

    if let Some(script_path) = py_script {
        if let Ok(py_out) = tokio::process::Command::new("python")
            .env("PYTHONIOENCODING", "utf-8")
            .arg(script_path)
            .arg("--json")
            .arg("--mode")
            .arg("refine")
            .arg(&prompt)
            .output()
            .await
        {
            if py_out.status.success() {
                if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&py_out.stdout) {
                    if val.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                        if let Some(t) = val.get("text").and_then(|v| v.as_str()) {
                            let clean = t.trim();
                            if !clean.is_empty() {
                                return Some(clean.to_string());
                            }
                        }
                    }
                }
            }
        }
    }

    let encoded_query: String = url::form_urlencoded::byte_serialize(prompt.as_bytes()).collect();
    let url = format!("https://www.google.com/search?udm=50&q={}", encoded_query);

    let Some(bin) = resolve_headless_scraping_browser() else {
        return None;
    };

    let app_data = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| "data".to_string());
    let session_dir = std::path::PathBuf::from(app_data).join("LeadFinder").join("google_session");
    let _ = std::fs::create_dir_all(&session_dir);
    let session_arg = format!("--user-data-dir={}", session_dir.display());

    let output = tokio::process::Command::new(&bin)
        .arg("--headless=new")
        .arg("--disable-gpu")
        .arg("--no-sandbox")
        .arg("--no-first-run")
        .arg("--no-default-browser-check")
        .arg("--disable-blink-features=AutomationControlled")
        .arg(&session_arg)
        .arg("--virtual-time-budget=10000")
        .arg("--dump-dom")
        .arg(&url)
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let dom = String::from_utf8_lossy(&output.stdout);
    extract_fixed_text_from_google_dom(&dom)
}

fn extract_fixed_text_from_google_dom(dom: &str) -> Option<String> {
    if let Ok(re_quotes) = regex::Regex::new(r#"[„"“]([А-ЯA-Z][^"“”]{15,})[“"”]"#) {
        if let Some(caps) = re_quotes.captures(dom) {
            if let Some(m) = caps.get(1) {
                let res = m.as_str().trim();
                if !res.is_empty() {
                    return Some(res.to_string());
                }
            }
        }
    }

    let re_block = regex::Regex::new(r#"data-processed="true">(.*?)</div>"#).ok()?;
    let blocks: Vec<&str> = re_block.find_iter(dom).map(|m| m.as_str()).collect();
    let re_tag = regex::Regex::new(r#"<[^>]+>"#).ok()?;
    for (i, b) in blocks.iter().enumerate() {
        let clean = re_tag.replace_all(b, " ");
        let s = clean.trim();
        if s.contains("би изглеждал така") || s.contains("вариант на текста") {
            if i + 1 < blocks.len() {
                let nxt_clean = re_tag.replace_all(blocks[i + 1], " ");
                let nxt = nxt_clean.trim().trim_matches(|c| c == '„' || c == '"' || c == '”' || c == '“');
                if nxt.chars().count() > 10 {
                    return Some(nxt.to_string());
                }
            }
        }
    }
    None
}

fn format_timed_segments_for_dialogue(
    lex: &leadfinder_providers::LexiconNormalizer,
    segments: &[leadfinder_core::TranscriptSegment],
    fallback: &str,
) -> String {
    let normalized_fallback = lex.normalize_text(fallback);
    if segments.is_empty() {
        return normalized_fallback;
    }

    let mut out = String::new();
    let mut current_sec_bucket: i64 = -1;

    for seg in segments {
        let sec = seg.start_ms / 1000;
        let bucket = (sec / 45) * 45;
        if bucket != current_sec_bucket {
            current_sec_bucket = bucket;
            let m = sec / 60;
            let s = sec % 60;
            if !out.is_empty() {
                out.push_str("\n\n");
            }
            out.push_str(&format!("**{:02}:{:02}**\n", m, s));
        }

        let cleaned = lex.normalize_text(&seg.text);
        let trimmed = cleaned.trim();
        if trimmed.is_empty() {
            continue;
        }

        out.push_str(&format!("{}\n", trimmed));
    }

    if out.trim().is_empty() {
        normalized_fallback
    } else {
        out.trim().to_string()
    }
}

async fn handle_google_ai_fix(
    State(ctx): State<Arc<PipelineContext>>,
    Path(id): Path<String>,
) -> Response {
    let tr_opt = match ctx.db.get_transcript(&id).await {
        Ok(t) => t,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    let Some(tr) = tr_opt else {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Transcript not found" })),
        )
            .into_response();
    };

    let cfg = ctx.config.read().await.clone();
    let lex = leadfinder_providers::global_lexicon(
        &ctx.paths,
        &cfg.normalisation.lexicon_config_file,
    );

    let base_text = tr.edited_text.unwrap_or(tr.text);
    let timed_raw = if !tr.segments.is_empty() {
        let mut s = String::new();
        let mut cur_bucket: i64 = -1;
        for seg in &tr.segments {
            let sec = seg.start_ms / 1000;
            let bucket = (sec / 45) * 45;
            if bucket != cur_bucket {
                cur_bucket = bucket;
                let m = sec / 60;
                let s_sec = sec % 60;
                if !s.is_empty() {
                    s.push_str("\n\n");
                }
                s.push_str(&format!("**{:02}:{:02}**\n", m, s_sec));
            }
            let t = seg.text.trim();
            if !t.is_empty() {
                s.push_str(t);
                s.push(' ');
            }
        }
        s.trim().to_string()
    } else {
        base_text.clone()
    };

    let maybe_fixed = run_google_ai_mode_fix(&timed_raw).await;

    let final_text = match maybe_fixed {
        Some(t) if !t.trim().is_empty() => t,
        _ => {
            let stage1 = lex.normalize_text(&timed_raw);
            let runner = leadfinder_providers::LlmChainRunner::from_runtime(&ctx.paths, &cfg, &ctx.db).await;
            let schema_val = serde_json::json!({
                "type": "object",
                "properties": {
                    "corrected_text": { "type": "string" }
                },
                "required": ["corrected_text"]
            });
            let llm_req = leadfinder_core::LlmJsonRequest {
                system_prompt: "You are an expert Bulgarian transcript editor and proofreader. Given an automatic speech recognition (ASR) transcript in Bulgarian with timestamps and phonetic errors, restore the clean Bulgarian text formatted in flowing paragraphs with bold timestamps (**MM:SS**). Fix all misheard Bulgarian words (e.g. алкогол -> алкохол, кота -> колата, полезай -> полицаи, и 12 -> 112, бърза полнош -> Бърза помощ, нуа нуа -> 0.0), restore Bulgarian grammar, punctuation, and question marks. Do not invent fake or arbitrary speaker names. Return JSON with 'corrected_text'.".to_string(),
                user_prompt: format!("what would the fixed text look like? Output the edited result only:\n{}", stage1),
                json_schema: Some(schema_val),
                max_tokens: 8192,
                temperature: 0.1,
            };
            match runner.complete_validated(&llm_req, &ctx.db).await {
                Ok(val) => val.response.parsed_json.get("corrected_text").and_then(|v| v.as_str()).map(|s| s.to_string()).unwrap_or_else(|| format_timed_segments_for_dialogue(&lex, &tr.segments, &base_text)),
                Err(_) => format_timed_segments_for_dialogue(&lex, &tr.segments, &base_text),
            }
        }
    };

    let _ = ctx.db.update_transcript_edited_text(&id, &final_text).await;

    Json(serde_json::json!({
        "ok": true,
        "video_id": id,
        "corrected_text": final_text,
        "source": "google_ai_mode"
    }))
    .into_response()
}

async fn handle_google_audio_transcribe(
    State(ctx): State<Arc<PipelineContext>>,
    Path(id): Path<String>,
) -> Response {
    let py_script = if std::path::Path::new("scripts/google_audio_transcribe.py").exists() {
        Some("scripts/google_audio_transcribe.py")
    } else if std::path::Path::new("tools/google_audio_transcribe.py").exists() {
        Some("tools/google_audio_transcribe.py")
    } else {
        None
    };

    let Some(script_path) = py_script else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": "Transcription script not found" })),
        )
            .into_response();
    };

    let cfg = ctx.config.read().await.clone();
    let lex = leadfinder_providers::global_lexicon(
        &ctx.paths,
        &cfg.normalisation.lexicon_config_file,
    );

    let py_res = tokio::process::Command::new("python")
        .env("PYTHONIOENCODING", "utf-8")
        .arg(script_path)
        .arg("--video-id")
        .arg(&id)
        .output()
        .await;

    let output = match py_res {
        Ok(o) if o.status.success() => o,
        Ok(o) => {
            let err_msg = String::from_utf8_lossy(&o.stderr);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Script failed: {err_msg}") })),
            )
                .into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    };

    let val: serde_json::Value = match serde_json::from_slice(&output.stdout) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Invalid JSON output: {e}") })),
            )
                .into_response();
        }
    };

    let corrected = val
        .get("corrected_text")
        .and_then(|v| v.as_str())
        .unwrap_or_default();

    let normalized = lex.normalize_text(corrected);

    let parsed_segs = val
        .get("segments")
        .and_then(|v| v.as_array())
        .and_then(|arr| serde_json::from_value::<Vec<leadfinder_core::TranscriptSegment>>(serde_json::Value::Array(arr.clone())).ok())
        .unwrap_or_default();

    if let Ok(Some(mut tr)) = ctx.db.get_transcript(&id).await {
        tr.edited_text = Some(normalized.clone());
        if !parsed_segs.is_empty() {
            tr.segments = parsed_segs;
        }
        let _ = ctx.db.save_transcript(&tr).await;
    } else {
        let _ = ctx.db.update_transcript_edited_text(&id, &normalized).await;
    }

    Json(serde_json::json!({
        "ok": true,
        "video_id": id,
        "corrected_text": normalized,
        "source": val.get("source").and_then(|v| v.as_str()).unwrap_or("google_audio")
    }))
    .into_response()
}

#[derive(Debug, Deserialize)]
pub struct GoogleAiQueryRequest {
    pub query: String,
    pub mode: Option<String>,
}

async fn handle_google_ai_query(
    State(ctx): State<Arc<PipelineContext>>,
    Json(body): Json<GoogleAiQueryRequest>,
) -> Response {
    let mode = body.mode.unwrap_or_else(|| "research".to_string());
    let query = body.query.trim().to_string();
    if query.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Query cannot be empty" })),
        )
            .into_response();
    }

    let py_script = if std::path::Path::new("scripts/google_ai_search.py").exists() {
        Some("scripts/google_ai_search.py")
    } else if std::path::Path::new("tools/google_ai_search.py").exists() {
        Some("tools/google_ai_search.py")
    } else {
        None
    };

    if let Some(script_path) = py_script {
        if let Ok(py_out) = tokio::process::Command::new("python")
            .arg(script_path)
            .arg("--json")
            .arg("--mode")
            .arg(&mode)
            .arg(&query)
            .output()
            .await
        {
            if py_out.status.success() {
                if let Ok(mut val) = serde_json::from_slice::<serde_json::Value>(&py_out.stdout) {
                    if val.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
                        if let Some(obj) = val.as_object_mut() {
                            obj.insert("source".to_string(), serde_json::json!("google_ai_mode"));
                        }
                        return Json(val).into_response();
                    }
                }
            }
        }
    }

    let cfg = ctx.config.read().await.clone();
    let runner = leadfinder_providers::LlmChainRunner::from_runtime(&ctx.paths, &cfg, &ctx.db).await;

    if mode == "refine" {
        let lex = leadfinder_providers::global_lexicon(
            &ctx.paths,
            &cfg.normalisation.lexicon_config_file,
        );
        let stage1 = lex.normalize_text(&query);
        let schema_val = serde_json::json!({
            "type": "object",
            "properties": {
                "corrected_text": { "type": "string" }
            },
            "required": ["corrected_text"]
        });
        let llm_req = leadfinder_core::LlmJsonRequest {
            system_prompt: "You are a specialized Bulgarian language transcription editor and fact-checking proofreader. You are given an automatic speech recognition (ASR) transcript in Bulgarian with phonetic errors, misrecognized Bulgarian names, distorted courtroom/political terminology, or dropped sounds. Restore the correct Bulgarian text: fix misheard names (e.g. Милен Цветков, Кристиан Николов, Софийски градски съд, прокуратура, затворническа администрация), correct phonetic misspellings, restore proper Bulgarian grammar, punctuation, and capitalization, while keeping the original spoken speech flow intact. Return JSON with a single field 'corrected_text' containing the clean corrected Bulgarian text.".to_string(),
            user_prompt: format!("what would the fixed text look like? Output the edited result only:{stage1}"),
            json_schema: Some(schema_val),
            max_tokens: 1024,
            temperature: 0.1,
        };
        let fixed = match runner.complete_validated(&llm_req, &ctx.db).await {
            Ok(v) => v.response.parsed_json.get("corrected_text").and_then(|s| s.as_str()).map(|s| s.to_string()).unwrap_or(stage1),
            Err(_) => stage1,
        };
        return Json(serde_json::json!({
            "ok": true,
            "query": query,
            "mode": mode,
            "text": fixed,
            "citations": [],
            "source": "local_llm_fallback"
        })).into_response();
    }

    let primary_entries = leadfinder_core::load_primary_sources(&ctx.paths.config_dir).unwrap_or_default();
    let search_engine = leadfinder_providers::WebAndPrimarySearch::new(
        &cfg.providers.searxng_url,
        primary_entries,
        cfg.factcheck_archives.clone(),
        cfg.verification.news_domains.clone(),
        ctx.guard.clone(),
    );
    let lex = leadfinder_providers::global_lexicon(&ctx.paths, &cfg.normalisation.lexicon_config_file);
    let ranker = leadfinder_providers::PassageExtractorAndRanker::new(ctx.guard.clone(), lex);
    let search_q = leadfinder_core::SearchQuery {
        query: format!("{} факти новини проверка източници", query),
        restrict_domains: Vec::new(),
        date_from: None,
        date_to: None,
        max_results: 5,
    };
    let mut citations = Vec::new();
    let mut text_snippets = Vec::new();
    if let Ok(hits) = search_engine.search(&search_q).await {
        for hit in hits {
            citations.push(serde_json::json!({ "title": hit.title, "url": hit.url }));
            let html = ranker.fetch_html_cached(&hit.url).await.unwrap_or_default();
            let txt = ranker.extract_main_text(&html);
            if !txt.is_empty() {
                text_snippets.push(format!("Източник {}: {}\n{}", hit.title, hit.url, txt.chars().take(600).collect::<String>()));
            }
        }
    }

    let prompt_research = format!(
        "Тема за проучване: {}\n\nНамерени първични източници:\n{}\n\nСинтезирай изчерпателен обективен аналитичен конспект на български език с цитиране на източниците.",
        query,
        text_snippets.join("\n\n")
    );
    let schema_val = serde_json::json!({
        "type": "object",
        "properties": {
            "summary": { "type": "string" }
        },
        "required": ["summary"]
    });
    let llm_req = leadfinder_core::LlmJsonRequest {
        system_prompt: "Ти си обективен журналистически анализатор и фактчекинг изследовател. Синтезирай точен конспект с факти, контекст и източници.".to_string(),
        user_prompt: prompt_research,
        json_schema: Some(schema_val),
        max_tokens: 1500,
        temperature: 0.1,
    };
    let summary = match runner.complete_validated(&llm_req, &ctx.db).await {
        Ok(v) => v.response.parsed_json.get("summary").and_then(|s| s.as_str()).map(|s| s.to_string()).unwrap_or_else(|| "Няма допълнителни данни.".to_string()),
        Err(_) => "Няма допълнителни данни.".to_string(),
    };

    Json(serde_json::json!({
        "ok": true,
        "query": query,
        "mode": mode,
        "text": summary,
        "citations": citations,
        "source": "searxng_llm_fallback"
    })).into_response()
}

#[derive(Debug, Deserialize)]
pub struct EvidencePreviewQuery {
    pub url: String,
}

#[derive(Debug, Deserialize)]
pub struct ResearchPersonRequest {
    pub name: Option<String>,
}

async fn handle_evidence_preview(
    State(ctx): State<Arc<PipelineContext>>,
    Query(q): Query<EvidencePreviewQuery>,
) -> Response {
    let cfg = ctx.config.read().await.clone();
    let lex = leadfinder_providers::global_lexicon(&ctx.paths, &cfg.normalisation.lexicon_config_file);
    let ranker = leadfinder_providers::PassageExtractorAndRanker::new(ctx.guard.clone(), lex);
    let body = ranker.fetch_html_cached(&q.url).await.unwrap_or_default();
    let text = ranker.extract_main_text(&body);
    let title = if let Some(start) = body.to_lowercase().find("<title>") {
        if let Some(end) = body.to_lowercase().find("</title>") {
            if end > start + 7 {
                body[start + 7..end].trim().to_string()
            } else {
                q.url.clone()
            }
        } else {
            q.url.clone()
        }
    } else {
        q.url.clone()
    };
    let domain = url::Url::parse(&q.url).ok().and_then(|u| u.domain().map(|d| d.to_string())).unwrap_or_default();
    Json(serde_json::json!({
        "url": q.url,
        "title": title,
        "text": text,
        "domain": domain
    })).into_response()
}

async fn handle_delete_evidence(
    State(ctx): State<Arc<PipelineContext>>,
    Path(id): Path<i64>,
) -> Response {
    match ctx.db.delete_evidence(id).await {
        Ok(true) => Json(serde_json::json!({ "ok": true, "deleted_id": id })).into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "evidence not found" })),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

async fn handle_open_browser(
    Json(body): Json<OpenBrowserRequest>,
) -> Response {
    #[cfg(windows)]
    {
        let _ = std::process::Command::new("rundll32")
            .args(["url.dll,FileProtocolHandler", &body.url])
            .spawn();
    }
    #[cfg(not(windows))]
    {
        let _ = std::process::Command::new("xdg-open").arg(&body.url).spawn();
    }
    Json(serde_json::json!({ "ok": true })).into_response()
}

async fn handle_research_person(
    State(ctx): State<Arc<PipelineContext>>,
    Path(id): Path<i64>,
    Json(body): Json<ResearchPersonRequest>,
) -> Response {
    let name = body.name.unwrap_or_default();
    let cfg = ctx.config.read().await.clone();
    let lex = leadfinder_providers::global_lexicon(&ctx.paths, &cfg.normalisation.lexicon_config_file);

    let archive_ep = cfg.factcheck_archives.clone();
    let primary_entries = leadfinder_core::load_primary_sources(&ctx.paths.config_dir).unwrap_or_default();
    let search_engine = leadfinder_providers::WebAndPrimarySearch::new(
        &cfg.providers.searxng_url,
        primary_entries,
        archive_ep,
        cfg.verification.news_domains.clone(),
        ctx.guard.clone(),
    );
    let ranker = leadfinder_providers::PassageExtractorAndRanker::new(ctx.guard.clone(), lex);
    let mut hits_text = Vec::new();

    let py_script = if std::path::Path::new("scripts/google_ai_search.py").exists() {
        Some("scripts/google_ai_search.py")
    } else if std::path::Path::new("tools/google_ai_search.py").exists() {
        Some("tools/google_ai_search.py")
    } else {
        None
    };

    if let Some(script_path) = py_script {
        if let Ok(py_out) = tokio::process::Command::new("python")
            .arg(script_path)
            .arg("--json")
            .arg("--mode")
            .arg("research")
            .arg(&name)
            .output()
            .await
        {
            if py_out.status.success() {
                if let Ok(val) = serde_json::from_slice::<serde_json::Value>(&py_out.stdout) {
                    if let Some(ai_txt) = val.get("text").and_then(|v| v.as_str()) {
                        let clean = ai_txt.trim();
                        if !clean.is_empty() {
                            hits_text.push(format!("Google AI Mode конспект:\n{}", clean));
                        }
                    }
                }
            }
        }
    }

    let (existing_cat, existing_ent) = if let Ok(Some(p)) = ctx.db.get_person_by_id(id).await {
        (p.category, Some(p.entity_type))
    } else {
        (None, None)
    };
    let entity_type = existing_ent.unwrap_or_else(|| leadfinder_providers::detect_entity_type(&name, None).to_string());

    let mut video_contexts: Vec<String> = Vec::new();
    if let Ok(vids) = ctx.db.get_videos_for_person(id).await {
        for (vid_id, caption, handle) in vids {
            let mut line = format!("Видео ID: {}", vid_id);
            if let Some(h) = handle {
                if !h.is_empty() {
                    line.push_str(&format!(" (Автор: @{})", h));
                }
            }
            if let Some(c) = caption {
                if !c.is_empty() {
                    line.push_str(&format!(" Описание: {}", c));
                }
            }
            video_contexts.push(line);
        }
    }

    let queries = match entity_type.as_str() {
        "institution" => [
            format!("{} институция ръководство структура правомощия дейност", name),
            format!("{} бюджет финансиране обществени поръчки одит проверки", name),
            format!("{} решения съдебни дела скандали проверки критики", name),
        ],
        "country" => [
            format!("{} държавно управление институции правителство структура", name),
            format!("{} икономика геополитика международни договори санкции", name),
            format!("{} кризи сигурност доклади права факти", name),
        ],
        "channel" => [
            format!("{} автор водещ канал предаване аудитория последователи", name),
            format!("{} медия продуцент собственост съдържание финансиране", name),
            format!("{} дезинформация проверка на факти скандали критики", name),
        ],
        "group" => [
            format!("{} партия коалиция движение ръководство лидери членове", name),
            format!("{} финансиране дарения активи свързани лица фирми", name),
            format!("{} платформа позиции избори скандали разследвания", name),
        ],
        _ => [
            format!("{} биография произход образование кариера длъжност", name),
            format!("{} фирми собственост имущество съдружници капитал", name),
            format!("{} съд дела разследвания скандали проверка факти", name),
        ],
    };

    let name_low = name.to_lowercase();
    let name_tokens: Vec<&str> = name_low.split_whitespace().filter(|t| t.chars().count() >= 3).collect();

    for q_str in queries {
        let sq = leadfinder_core::SearchQuery {
            query: q_str,
            restrict_domains: Vec::new(),
            date_from: None,
            date_to: None,
            max_results: 4,
        };
        if let Ok(hits) = search_engine.search(&sq).await {
            for hit in hits.into_iter().take(3) {
                if !hits_text.iter().any(|h| h.contains(&hit.url)) {
                    let html = ranker.fetch_html_cached(&hit.url).await.unwrap_or_default();
                    let txt = ranker.extract_main_text(&html);
                    if !txt.is_empty() {
                        let combined = format!("{} {}", hit.title.to_lowercase(), txt.to_lowercase());
                        let name_present = if name_tokens.len() >= 2 {
                            combined.contains(&name_low) || name_tokens.iter().all(|t| combined.contains(t))
                        } else {
                            combined.contains(&name_low)
                        };
                        if name_present {
                            hits_text.push(format!("Източник {}: {}\n{}", hit.title, hit.url, txt.chars().take(1500).collect::<String>()));
                        }
                    }
                }
            }
        }
    }

    let summary_context = hits_text.join("\n\n---\n\n");
    let runner = leadfinder_providers::LlmChainRunner::from_runtime(&ctx.paths, &cfg, &ctx.db).await;
    let schema_val = serde_json::json!({
        "type": "object",
        "properties": {
            "role": { "type": "string" },
            "summary": { "type": "string" },
            "public_figure": { "type": "boolean" },
            "category": { "type": "string" },
            "entity_type": {
                "type": "string",
                "enum": ["person", "institution", "channel", "country", "group"]
            },
            "tags": {
                "type": "array",
                "items": { "type": "string" }
            },
            "full_dossier_report_markdown": { "type": "string" },
            "profile": {
                "type": "object",
                "properties": {
                    "identity": { "type": "string" },
                    "contact_locations": { "type": "string" },
                    "network_associates": { "type": "string" },
                    "education_career": { "type": "string" },
                    "financial_assets": { "type": "string" },
                    "legal_regulatory": { "type": "string" },
                    "digital_media_presence": { "type": "string" },
                    "affiliations_beliefs": { "type": "string" },
                    "psychographic_behavioral": { "type": "string" }
                }
            }
        },
        "required": ["role", "summary", "public_figure", "tags", "full_dossier_report_markdown"]
    });

    let system_prompt_str = match entity_type.as_str() {
        "institution" => "You are an expert institutional intelligence analyst and investigative journalist. Generate an exhaustive, deep OSINT due-diligence report on the named institution or government body in Bulgarian (~1,500 - 2,500 words). The 'full_dossier_report_markdown' must cover institutional mandate, governing leadership, budget, public contracts, audits, key decisions, regulatory actions, and controversies. Output strictly valid JSON in Bulgarian.",
        "country" => "You are a senior geopolitical intelligence analyst. Generate an exhaustive OSINT country brief on the subject in Bulgarian (~1,500 - 2,500 words). Cover state structure, government institutions, geopolitical alignments, macroeconomic profile, international agreements, security, and verified key events. Output strictly valid JSON in Bulgarian.",
        "channel" => "You are an OSINT media intelligence analyst. Generate an exhaustive due-diligence report on the media channel, show, or creator brand in Bulgarian (~1,500 - 2,500 words). Cover channel hosts/creators, corporate ownership, monetization, content focus, audience reach, disinformation factchecks, and controversies. Output strictly valid JSON in Bulgarian.",
        "group" => "You are an expert political and group due-diligence investigator. Generate an exhaustive OSINT dossier on the political party, movement, or group in Bulgarian (~1,500 - 2,500 words). Cover leadership, organizational structure, donors, funding, assets, political alliances, policy platforms, and scandals. Output strictly valid JSON in Bulgarian.",
        _ => "You are a master intelligence analyst and investigative journalist. Generate an exhaustive, deep, and complete due-diligence OSINT intelligence dossier on the subject in Bulgarian (target length: ~1,500 - 2,500 words). The field 'full_dossier_report_markdown' must be an extensive, highly detailed investigative dossier formatted with Markdown headers, full paragraphs, dates, institutions, verified facts, associates, companies, lawsuits, controversies, and narrative profiling covering all 9 OSINT categories:\n# ОСИНТ РАЗУЗНАВАТЕЛНО ДОСИЕ: {NAME}\n## 1. Идентичност и основни лични данни\n## 2. Локации, адреси и комуникационни канали\n## 3. Семейна среда, мрежа и ключови съдружници\n## 4. Образование и професионална хронология\n## 5. Финансов профил, фирми и имуществени интереси\n## 6. Правен, съдебен и регулаторен статус (PEP)\n## 7. Медийно присъствие, публикации и дигитален отпечатък\n## 8. Политически принадлежности, убеждения и лобистки връзки\n## 9. Психографски и поведенчески профил\n## 10. Оценка на риска и заключителни изводи\nDo not write a brief summary. Write extensive, multi-paragraph, professional analysis for every section. Output valid JSON in Bulgarian.",
    };

    let vid_context_block = if video_contexts.is_empty() {
        String::new()
    } else {
        format!("\n\nСвързан контекст от TikTok видеа:\n{}", video_contexts.join("\n"))
    };
    let user_prompt = format!("Субект за пълно OSINT разследване: {} (Тип: {}){}{}\n\nСъбрани разузнавателни данни и източници от мрежата:\n{}", name, entity_type, vid_context_block, if hits_text.is_empty() { "\n(Няма намерени открити регистрови уеб страници. Направи профилен анализ на база видео контекста и идентифицираната тема.)" } else { "" }, summary_context);

    let llm_req = leadfinder_core::LlmJsonRequest {
        system_prompt: system_prompt_str.to_string(),
        user_prompt,
        json_schema: Some(schema_val),
        max_tokens: 4096,
        temperature: 0.1,
    };

    let mut res_role = None;
    let mut res_summary = None;
    let mut res_pub = false;
    let mut res_tags: Vec<String> = Vec::new();
    let mut res_detailed_json: Option<String> = None;
    let mut res_category: Option<String> = existing_cat;
    let mut res_entity_type = entity_type;
    let mut report_md = String::new();

    if let Ok(val) = runner.complete_validated(&llm_req, &ctx.db).await {
        res_role = val.response.parsed_json.get("role").and_then(|v| v.as_str()).map(|s| s.to_string());
        res_summary = val.response.parsed_json.get("summary").and_then(|v| v.as_str()).map(|s| s.to_string());
        res_pub = val.response.parsed_json.get("public_figure").and_then(|v| v.as_bool()).unwrap_or(false);
        if let Some(cat) = val.response.parsed_json.get("category").and_then(|v| v.as_str()) {
            res_category = Some(cat.to_string());
        }
        if let Some(ent) = val.response.parsed_json.get("entity_type").and_then(|v| v.as_str()) {
            res_entity_type = ent.to_string();
        }
        if let Some(arr) = val.response.parsed_json.get("tags").and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(t) = item.as_str() {
                    let clean = t.trim();
                    if !clean.is_empty() && !res_tags.iter().any(|x| x.eq_ignore_ascii_case(clean)) {
                        res_tags.push(clean.to_string());
                    }
                }
            }
        }
        report_md = val.response.parsed_json.get("full_dossier_report_markdown")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_default();
        let full_obj = serde_json::json!({
            "report_markdown": report_md,
            "profile": val.response.parsed_json.get("profile"),
        });
        res_detailed_json = Some(serde_json::to_string(&full_obj).unwrap_or_default());
    }

    if report_md.is_empty() {
        let clean_summary = res_summary.clone().unwrap_or_else(|| {
            format!("Към момента за субекта '{}' няма достатъчно публични регистрови данни или официални досиета в проверените източници.", name)
        });
        let mut clean_srcs = Vec::new();
        for h in &hits_text {
            let first_line = h.lines().next().unwrap_or("");
            if !first_line.is_empty() {
                clean_srcs.push(format!("- {}", first_line));
            }
        }
        let sources_block = if clean_srcs.is_empty() {
            "Няма намерени потвърдени публични външни уеб публикации.".to_string()
        } else {
            clean_srcs.join("\n")
        };
        let vid_block = if video_contexts.is_empty() {
            "Няма свързани видеа в системата.".to_string()
        } else {
            video_contexts.join("\n")
        };
        report_md = format!(
            "# OSINT ДОСИЕ: {}\n\n## 1. Основни данни и класификация\n- **Субект**: {}\n- **Тип на субекта**: {}\n- **Роля**: {}\n- **Публична фигура**: {}\n\n## 2. Оперативно обобщение\n{}\n\n## 3. Свързан контекст от анализирани видеа\n{}\n\n## 4. Проверени източници и регистри\n{}\n\n## 5. Заключение и оценка на риска\nАнализът не установява официални съдебни присъди или компрометиращи регистрови вписвания в проверените бази.",
            name,
            name,
            res_entity_type,
            res_role.as_deref().unwrap_or("Неуточнена"),
            if res_pub { "Да" } else { "Не" },
            clean_summary,
            vid_block,
            sources_block
        );
        let full_obj = serde_json::json!({
            "report_markdown": report_md,
            "profile": serde_json::Value::Null,
        });
        res_detailed_json = Some(serde_json::to_string(&full_obj).unwrap_or_default());
    }

    if res_summary.is_none() && !summary_context.is_empty() {
        res_summary = Some(summary_context.chars().take(250).collect());
    }

    let osint_dir = std::path::PathBuf::from("data/osint");
    let _ = std::fs::create_dir_all(&osint_dir);
    let safe_name: String = name.chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
    let report_filename = format!("{}_{}_dossier.md", id, safe_name);
    let report_path = osint_dir.join(&report_filename);
    let _ = std::fs::write(&report_path, &report_md);

    let _ = ctx.db.update_person_research_data(
        id,
        res_role.as_deref(),
        res_summary.as_deref(),
        &res_tags,
        res_detailed_json.as_deref(),
        res_pub,
        res_category.as_deref(),
        Some(&res_entity_type),
    ).await;
    let (total_claims, contradicted, supported) = ctx.db.get_person_claims_stats(id).await.unwrap_or((0, 0, 0));

    Json(serde_json::json!({
        "id": id,
        "canonical_name": name,
        "role": res_role,
        "summary": res_summary,
        "public_figure": res_pub,
        "category": res_category,
        "entity_type": res_entity_type,
        "tags": res_tags,
        "detailed_profile_json": res_detailed_json,
        "dossier_report_markdown": report_md,
        "report_file_path": format!("data/osint/{}", report_filename),
        "total_claims": total_claims,
        "contradicted_count": contradicted,
        "supported_count": supported
    })).into_response()
}

async fn handle_get_person_dossier(
    State(ctx): State<Arc<PipelineContext>>,
    Path(id): Path<i64>,
) -> Response {
    if let Ok(Some(p)) = ctx.db.get_person_by_id(id).await {
        if let Some(json_str) = p.detailed_profile_json {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&json_str) {
                if let Some(md) = val.get("report_markdown").and_then(|v| v.as_str()) {
                    return (
                        [(axum::http::header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
                        md.to_string(),
                    ).into_response();
                }
            }
        }
        if let Some(sum) = p.summary {
            return (
                [(axum::http::header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
                format!("# Досие: {}\n\n{}", p.canonical_name, sum),
            ).into_response();
        }
    }
    (StatusCode::NOT_FOUND, "Dossier not found").into_response()
}

async fn handle_list_people(
    State(ctx): State<Arc<PipelineContext>>,
    Query(q): Query<ListPeopleQuery>,
) -> Response {
    match ctx.db.list_all_people(
        q.query.as_deref(),
        q.tag.as_deref(),
        q.category.as_deref(),
        q.sort_by.as_deref(),
        q.entity_type.as_deref(),
    ).await {
        Ok(people) => Json(people).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ).into_response(),
    }
}

async fn handle_get_person(
    State(ctx): State<Arc<PipelineContext>>,
    Path(id): Path<i64>,
) -> Response {
    match ctx.db.get_person_by_id(id).await {
        Ok(Some(p)) => {
            let (total, contradicted, supported) = ctx.db.get_person_claims_stats(id).await.unwrap_or((0, 0, 0));
            Json(serde_json::json!({
                "person": p,
                "claims_stats": {
                    "total": total,
                    "contradicted": contradicted,
                    "supported": supported,
                }
            })).into_response()
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "error": "Person not found" })),
        ).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ).into_response(),
    }
}

async fn handle_create_person(
    State(ctx): State<Arc<PipelineContext>>,
    Json(body): Json<CreatePersonRequest>,
) -> Response {
    let name = body.canonical_name.trim();
    if name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "canonical_name is required" })),
        ).into_response();
    }
    let tags = body.tags.unwrap_or_default();
    let rating = body.rating.unwrap_or(50);
    let pub_fig = body.public_figure.unwrap_or(true);
    let detected_type = leadfinder_providers::detect_entity_type(name, body.role.as_deref());
    let ent_type = body.entity_type.as_deref().unwrap_or(detected_type);
    match ctx.db.create_person(
        name,
        body.role.as_deref(),
        body.category.as_deref(),
        &tags,
        rating,
        body.notes.as_deref(),
        pub_fig,
        Some(ent_type),
    ).await {
        Ok(id) => Json(serde_json::json!({ "ok": true, "id": id })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ).into_response(),
    }
}

async fn handle_update_person(
    State(ctx): State<Arc<PipelineContext>>,
    Path(id): Path<i64>,
    Json(body): Json<UpdatePersonRequest>,
) -> Response {
    let name = body.canonical_name.trim();
    if name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "canonical_name is required" })),
        ).into_response();
    }
    let tags = body.tags.unwrap_or_default();
    let rating = body.rating.unwrap_or(50);
    let pub_fig = body.public_figure.unwrap_or(true);
    match ctx.db.update_person_full(
        id,
        name,
        body.role.as_deref(),
        body.category.as_deref(),
        &tags,
        rating,
        body.notes.as_deref(),
        pub_fig,
        body.summary.as_deref(),
        body.detailed_profile_json.as_deref(),
        body.entity_type.as_deref(),
    ).await {
        Ok(()) => Json(serde_json::json!({ "ok": true, "id": id })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ).into_response(),
    }
}

async fn handle_delete_person(
    State(ctx): State<Arc<PipelineContext>>,
    Path(id): Path<i64>,
) -> Response {
    match ctx.db.delete_person(id).await {
        Ok(()) => Json(serde_json::json!({ "ok": true, "deleted": id })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ).into_response(),
    }
}

async fn handle_auto_classify_people(
    State(ctx): State<Arc<PipelineContext>>,
) -> Response {
    let all = match ctx.db.list_all_people(None, None, None, None, None).await {
        Ok(list) => list,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            ).into_response();
        }
    };

    let mut updated = 0usize;
    let mut pruned = 0usize;
    let total = all.len();

    for p in all {
        if leadfinder_providers::is_extracted_noise(&p.canonical_name) {
            let _ = ctx.db.delete_person(p.id).await;
            pruned += 1;
            continue;
        }

        let detected = leadfinder_providers::detect_entity_type(&p.canonical_name, p.role.as_deref());
        let current_type = p.entity_type.as_str();

        let tag_to_add = match detected {
            "institution" => Some("институция"),
            "channel" => Some("канал"),
            "country" => Some("държава"),
            "group" => Some("политическа партия"),
            "event" => Some("събитие"),
            _ => None,
        };

        let cat_to_set = match detected {
            "institution" => Some("институция"),
            "channel" => Some("медия"),
            "country" => Some("държава"),
            "group" => Some("политика"),
            "event" => Some("събития"),
            _ => None,
        };

        if detected != current_type || p.category.is_none() {
            let _ = ctx.db.update_person_classification(
                p.id,
                detected,
                cat_to_set,
                tag_to_add,
            ).await;
            updated += 1;
        }
    }

    Json(serde_json::json!({
        "ok": true,
        "total": total,
        "updated": updated,
        "pruned": pruned,
    })).into_response()
}

async fn handle_reassign_video_person(
    State(ctx): State<Arc<PipelineContext>>,
    Path((video_id, person_id)): Path<(String, i64)>,
    Json(body): Json<ReassignVideoPersonRequest>,
) -> Response {
    let target_person_id = if let Some(new_id) = body.new_person_id {
        new_id
    } else if let Some(name) = body.canonical_name.filter(|n| !n.trim().is_empty()) {
        let tags = body.tags.unwrap_or_default();
        let detected = leadfinder_providers::detect_entity_type(&name, body.role.as_deref());
        let ent = body.entity_type.as_deref().unwrap_or(detected);
        match ctx.db.create_person(
            &name,
            body.role.as_deref(),
            body.category.as_deref(),
            &tags,
            75,
            None,
            true,
            Some(ent),
        ).await {
            Ok(new_id) => new_id,
            Err(e) => return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            ).into_response(),
        }
    } else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Either new_person_id or canonical_name must be provided" })),
        ).into_response();
    };

    match ctx.db.reassign_video_person(&video_id, person_id, target_person_id).await {
        Ok(()) => Json(serde_json::json!({ "ok": true, "video_id": video_id, "old_person_id": person_id, "new_person_id": target_person_id })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ).into_response(),
    }
}

async fn handle_remove_video_person(
    State(ctx): State<Arc<PipelineContext>>,
    Path((video_id, person_id)): Path<(String, i64)>,
) -> Response {
    match ctx.db.remove_person_from_video(&video_id, person_id).await {
        Ok(()) => Json(serde_json::json!({ "ok": true, "video_id": video_id, "removed_person_id": person_id })).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ).into_response(),
    }
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

#[derive(Debug, Deserialize, Default)]
struct AssessRiskRequest {
    display_name: Option<String>,
    force_web: Option<bool>,
}

async fn handle_assess_creator_risk(
    State(ctx): State<Arc<PipelineContext>>,
    Path(handle): Path<String>,
    body: Option<Json<AssessRiskRequest>>,
) -> Response {
    let req = body.map(|Json(b)| b).unwrap_or_default();
    let force = req.force_web.unwrap_or(true);
    match crate::creator_risk::assess_creator_risk(&ctx, &handle, req.display_name.as_deref(), force).await {
        Ok(assessment) => Json(assessment).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
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

async fn handle_calibrate(
    State(ctx): State<Arc<PipelineContext>>,
    body: Option<Json<CalibrateRequest>>,
) -> Response {
    let req = body.map(|Json(b)| b).unwrap_or_default();
    let power_info = leadfinder_probe::detect_power_info();
    let power_source = req
        .power_source
        .unwrap_or(power_info.power_source)
        .to_lowercase();

    {
        let state = ctx.benchmark_state.read().await;
        if state.is_running && !req.force_rerun {
            return Json(serde_json::json!({
                "already_running": true,
                "progress": state.clone(),
            }))
            .into_response();
        }
    }

    {
        let mut state = ctx.benchmark_state.write().await;
        state.is_running = true;
        state.power_source = power_source.clone();
        state.current_stage = "detect_hw".to_string();
        state.stage_index = 1;
        state.total_stages = 4;
        state.progress_pct = 5;
        state.status_text = "Стартиране на калибриране...".to_string();
        state.status_text_en = "Starting calibration...".to_string();
        state.current_model = None;
        state.asr_results.clear();
        state.llm_results.clear();
        state.final_capacity = None;
        state.error = None;
        state.started_at = Some(chrono::Utc::now().to_rfc3339());
        state.completed_at = None;
    }

    let initial_payload = serde_json::to_value(&*ctx.benchmark_state.read().await).unwrap_or_default();
    let _ = ctx.events_tx.send(leadfinder_core::CoreEvent::BenchmarkProgress(initial_payload));

    let ctx_clone = ctx.clone();
    let power_source_clone = power_source.clone();
    tokio::spawn(async move {
        let cfg = ctx_clone.config.read().await.clone();
        let bstate = ctx_clone.benchmark_state.clone();
        let ev_tx = ctx_clone.events_tx.clone();

        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<leadfinder_probe::BenchmarkProgressUpdate>();
        let bstate_worker = bstate.clone();
        let ev_tx_worker = ev_tx.clone();
        let updater = tokio::spawn(async move {
            while let Some(update) = rx.recv().await {
                let mut st = bstate_worker.write().await;
                st.current_stage = update.stage.clone();
                st.stage_index = update.stage_index;
                st.total_stages = update.total_stages;
                st.progress_pct = update.percent;
                st.status_text = update.message_bg.clone();
                st.status_text_en = update.message_en.clone();
                if let Some(ref m) = update.current_model {
                    st.current_model = Some(m.clone());
                }
                if let Some(ref asr) = update.asr_benchmark {
                    if !st.asr_results.iter().any(|b| b.model == asr.model) {
                        st.asr_results.push(asr.clone());
                    }
                }
                if let Some(ref llm) = update.llm_benchmark {
                    if !st.llm_results.iter().any(|b| b.model == llm.model) {
                        st.llm_results.push(llm.clone());
                    }
                }
                let payload = serde_json::to_value(&*st).unwrap_or_default();
                let _ = ev_tx_worker.send(leadfinder_core::CoreEvent::BenchmarkProgress(payload));
            }
        });

        let progress_cb: leadfinder_probe::BenchmarkProgressCallback = {
            let tx = tx.clone();
            Arc::new(move |update: leadfinder_probe::BenchmarkProgressUpdate| {
                let _ = tx.send(update);
            })
        };

        let cal_res = leadfinder_probe::run_calibration_for_power(
            &ctx_clone.paths,
            &cfg,
            &ctx_clone.db,
            req.force_cpu,
            Some(&power_source_clone),
            Some(progress_cb),
        )
        .await;

        drop(tx);
        let _ = updater.await;

        match cal_res {
            Ok(profile) => {
                let mut st = bstate.write().await;
                st.is_running = false;
                st.current_stage = "done".to_string();
                st.progress_pct = 100;
                st.final_capacity = profile.capacity_per_day;
                st.completed_at = Some(chrono::Utc::now().to_rfc3339());
                st.status_text = format!(
                    "Готово! Измерен капацитет: {:.0} видеа/ден ({})",
                    profile.capacity_per_day.unwrap_or(0.0),
                    power_source_clone.to_uppercase()
                );
                st.status_text_en = format!(
                    "Complete! Measured capacity: {:.0} videos/day ({})",
                    profile.capacity_per_day.unwrap_or(0.0),
                    power_source_clone.to_uppercase()
                );
                let payload = serde_json::to_value(&*st).unwrap_or_default();
                let _ = ev_tx.send(leadfinder_core::CoreEvent::BenchmarkProgress(payload));
                let _ = ev_tx.send(leadfinder_core::CoreEvent::CalibrationUpdated {
                    capacity_per_day: profile.capacity_per_day,
                });
            }
            Err(e) => {
                let mut st = bstate.write().await;
                st.is_running = false;
                st.current_stage = "failed".to_string();
                st.error = Some(e.to_string());
                st.status_text = format!("Грешка при калибриране: {e}");
                st.status_text_en = format!("Calibration failed: {e}");
                let payload = serde_json::to_value(&*st).unwrap_or_default();
                let _ = ev_tx.send(leadfinder_core::CoreEvent::BenchmarkProgress(payload));
            }
        }
    });

    let current = ctx.benchmark_state.read().await.clone();
    Json(serde_json::json!({
        "started": true,
        "progress": current,
    }))
    .into_response()
}

async fn handle_calibrate_status(State(ctx): State<Arc<PipelineContext>>) -> Response {
    let progress = ctx.benchmark_state.read().await.clone();
    let power = leadfinder_probe::detect_power_info();
    let (ac, batt) = ctx.db.get_power_profiles().await.unwrap_or((None, None));
    let active = ctx.db.get_profile().await.ok().flatten();
    Json(CalibrateStatusResponse {
        is_running: progress.is_running,
        progress,
        power,
        ac_profile: ac,
        battery_profile: batt,
        active_profile: active,
    })
    .into_response()
}

async fn handle_calibrate_switch_power(
    State(ctx): State<Arc<PipelineContext>>,
    Json(body): Json<SwitchPowerRequest>,
) -> Response {
    let prof = ctx.db.get_profile_for_power(&body.power_source).await.ok().flatten();
    if let Some(ref p) = prof {
        let _ = ctx.db.save_profile(p).await;
        let _ = ctx.events_tx.send(leadfinder_core::CoreEvent::CalibrationUpdated {
            capacity_per_day: p.capacity_per_day,
        });
    }
    Json(serde_json::json!({
        "power_source": body.power_source,
        "profile": prof,
    }))
    .into_response()
}

async fn handle_calibrate_overrides(
    State(ctx): State<Arc<PipelineContext>>,
    Json(overrides): Json<CalibrationOverrides>,
) -> Response {
    let cfg = ctx.config.read().await.clone();
    match apply_calibration_overrides(&ctx.db, &cfg, overrides).await {
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

#[derive(Debug, Serialize)]
pub struct GeminiStatusResponse {
    pub configured: bool,
    pub masked_key: Option<String>,
    pub cloud_enabled: bool,
    pub model: String,
    pub total_keys: usize,
    pub active_key_index: usize,
    pub keys: Vec<GeminiKeyInfo>,
}

#[derive(Debug, Deserialize)]
pub struct SetGeminiKeyRequest {
    pub key: Option<String>,
    pub keys: Option<Vec<String>>,
}

fn mask_api_key(key: &str) -> String {
    let trimmed = key.trim();
    if trimmed.len() <= 8 {
        return "********".to_string();
    }
    format!("{}...{}", &trimmed[..6], &trimmed[trimmed.len().saturating_sub(4)..])
}

async fn handle_gemini_status(
    State(ctx): State<Arc<PipelineContext>>,
) -> Response {
    let cfg = ctx.config.read().await;
    let resolved_key = leadfinder_core::resolve_secret("GEMINI_API_KEY");
    let pool_val = ctx.db.get_setting("gemini_keys_pool").await.ok().flatten();

    let mut keys_list: Vec<String> = Vec::new();
    let mut active_idx = 0usize;

    if let Some(val) = pool_val {
        if let Some(arr) = val.get("keys").and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(s) = item.as_str() {
                    let t = s.trim().to_string();
                    if !t.is_empty() && !keys_list.contains(&t) {
                        keys_list.push(t);
                    }
                }
            }
        }
        active_idx = val.get("active_index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    }

    if keys_list.is_empty() {
        if let Some(ref k) = resolved_key {
            keys_list.push(k.clone());
        }
    }

    if active_idx >= keys_list.len() {
        active_idx = 0;
    }

    let configured = !keys_list.is_empty();
    let active_key = keys_list.get(active_idx).cloned().or(resolved_key);
    let masked_key = active_key.as_deref().map(mask_api_key);
    let cloud_enabled = cfg.providers.cloud_enabled;
    let model = cfg.providers.gemini_model.clone();

    let key_infos: Vec<GeminiKeyInfo> = keys_list
        .iter()
        .enumerate()
        .map(|(i, k)| GeminiKeyInfo {
            index: i,
            masked: mask_api_key(k),
            is_active: i == active_idx,
        })
        .collect();

    Json(GeminiStatusResponse {
        configured,
        masked_key,
        cloud_enabled,
        model,
        total_keys: keys_list.len(),
        active_key_index: active_idx,
        keys: key_infos,
    })
    .into_response()
}

async fn handle_set_gemini_key(
    State(ctx): State<Arc<PipelineContext>>,
    Json(body): Json<SetGeminiKeyRequest>,
) -> Response {
    let mut raw_keys = Vec::new();
    if let Some(ref k) = body.key {
        for part in k.split(|c| c == '\n' || c == ',' || c == ';' || c == '\r') {
            let t = part.trim().to_string();
            if !t.is_empty() && !raw_keys.contains(&t) {
                raw_keys.push(t);
            }
        }
    }
    if let Some(ref list) = body.keys {
        for k in list {
            let t = k.trim().to_string();
            if !t.is_empty() && !raw_keys.contains(&t) {
                raw_keys.push(t);
            }
        }
    }

    if raw_keys.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "API key cannot be empty" })),
        )
            .into_response();
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .unwrap_or_default();

    let mut valid_keys = Vec::new();
    let mut errors = Vec::new();

    for key in raw_keys {
        let test_url = format!("https://generativelanguage.googleapis.com/v1beta/models?key={}", key);
        match client.get(&test_url).send().await {
            Ok(resp) => {
                if resp.status().is_success() {
                    valid_keys.push(key);
                } else {
                    let st = resp.status();
                    let err = resp.text().await.unwrap_or_default();
                    errors.push(format!("{}: HTTP {st} - {err}", mask_api_key(&key)));
                }
            }
            Err(e) => {
                errors.push(format!("{}: {e}", mask_api_key(&key)));
            }
        }
    }

    if valid_keys.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": format!("Validation failed for all keys: {}", errors.join("; "))
            })),
        )
            .into_response();
    }

    let active_key = valid_keys[0].clone();
    std::env::set_var("GEMINI_API_KEY", &active_key);
    let _ = leadfinder_core::store_secret_in_os_credential_store("GEMINI_API_KEY", &active_key);

    let pool = serde_json::json!({
        "keys": valid_keys,
        "active_index": 0,
    });
    let _ = ctx.db.set_setting("gemini_keys_pool", &pool).await;
    let _ = ctx.db.set_setting("cloud_enabled", &serde_json::json!(true)).await;
    let _ = ctx.db.set_setting("gemini_configured", &serde_json::json!(true)).await;
    ctx.config.write().await.providers.cloud_enabled = true;

    let key_infos: Vec<GeminiKeyInfo> = valid_keys
        .iter()
        .enumerate()
        .map(|(i, k)| GeminiKeyInfo {
            index: i,
            masked: mask_api_key(k),
            is_active: i == 0,
        })
        .collect();

    Json(serde_json::json!({
        "ok": true,
        "total_keys": valid_keys.len(),
        "masked_key": mask_api_key(&active_key),
        "keys": key_infos,
        "cloud_enabled": true,
        "validation_warnings": if errors.is_empty() { None } else { Some(errors) }
    }))
    .into_response()
}

async fn handle_rotate_gemini_key(State(ctx): State<Arc<PipelineContext>>) -> Response {
    let pool_val = ctx.db.get_setting("gemini_keys_pool").await.ok().flatten();
    let Some(mut pool) = pool_val else {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "No alternate keys available in pool" })),
        )
            .into_response();
    };

    let keys = match pool.get("keys").and_then(|v| v.as_array()) {
        Some(k) if k.len() > 1 => k.clone(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": "Pool has only 1 key or empty" })),
            )
                .into_response();
        }
    };

    let cur_idx = pool.get("active_index").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
    let next_idx = (cur_idx + 1) % keys.len();
    let next_key = keys.get(next_idx).and_then(|v| v.as_str()).unwrap_or("").to_string();

    pool["active_index"] = serde_json::json!(next_idx);
    let _ = ctx.db.set_setting("gemini_keys_pool", &pool).await;
    std::env::set_var("GEMINI_API_KEY", &next_key);
    let _ = leadfinder_core::store_secret_in_os_credential_store("GEMINI_API_KEY", &next_key);

    Json(serde_json::json!({
        "ok": true,
        "rotated": true,
        "active_index": next_idx,
        "masked_key": mask_api_key(&next_key),
    }))
    .into_response()
}

async fn handle_delete_gemini_key(
    State(ctx): State<Arc<PipelineContext>>,
) -> Response {
    std::env::remove_var("GEMINI_API_KEY");
    let _ = leadfinder_core::store_secret_in_os_credential_store("GEMINI_API_KEY", "");
    let _ = ctx.db.set_setting("gemini_keys_pool", &serde_json::json!({ "keys": [], "active_index": 0 })).await;
    let _ = ctx.db.set_setting("cloud_enabled", &serde_json::json!(false)).await;
    let _ = ctx.db.set_setting("gemini_configured", &serde_json::json!(false)).await;
    ctx.config.write().await.providers.cloud_enabled = false;

    Json(serde_json::json!({
        "ok": true,
        "configured": false,
        "cloud_enabled": false
    }))
    .into_response()
}

async fn handle_system_shutdown(State(_ctx): State<Arc<PipelineContext>>) -> Response {
    tokio::spawn(async {
        tokio::time::sleep(std::time::Duration::from_millis(150)).await;
        std::process::exit(0);
    });
    Json(serde_json::json!({
        "ok": true,
        "terminating": true
    }))
    .into_response()
}

async fn handle_system_shutdown_when_idle(State(ctx): State<Arc<PipelineContext>>) -> Response {
    ctx.auto_terminate_when_idle
        .store(true, std::sync::atomic::Ordering::Relaxed);
    let counts = ctx
        .db
        .queue_counts()
        .await
        .unwrap_or_else(|_| serde_json::json!({}));
    Json(serde_json::json!({
        "ok": true,
        "auto_terminate_enabled": true,
        "queue_counts": counts
    }))
    .into_response()
}

async fn handle_system_exit_status(State(ctx): State<Arc<PipelineContext>>) -> Response {
    let auto_term = ctx
        .auto_terminate_when_idle
        .load(std::sync::atomic::Ordering::Relaxed);
    let counts = ctx
        .db
        .queue_counts()
        .await
        .unwrap_or_else(|_| serde_json::json!({}));
    Json(serde_json::json!({
        "auto_terminate_when_idle": auto_term,
        "queue_counts": counts
    }))
    .into_response()
}

async fn handle_scale_status(State(ctx): State<Arc<PipelineContext>>) -> impl IntoResponse {
    let aimd = ctx.concurrency.snapshot().await;
    let mut sys = sysinfo::System::new();
    let pid = sysinfo::get_current_pid().ok();
    let rss = if let Some(p) = pid {
        sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[p]), true);
        sys.process(p).map(|proc| proc.memory()).unwrap_or(0)
    } else {
        0
    };

    let disks = sysinfo::Disks::new_with_refreshed_list();
    let free_disk = disks.iter().map(|d| d.available_space()).max().unwrap_or(0);
    let media_bytes = compute_media_bytes(&ctx.media_root());
    let breakers = ctx.guard.breaker_statuses().await;
    let open_breakers = breakers.iter().filter(|b| b.is_open).count();

    Json(serde_json::json!({
        "aimd": aimd,
        "rss_bytes": rss,
        "disk_free_bytes": free_disk,
        "media_bytes": media_bytes,
        "open_breakers": open_breakers,
    }))
}

fn compute_media_bytes(media_root: &std::path::Path) -> u64 {
    let mut total = 0u64;
    if let Ok(entries) = std::fs::read_dir(media_root) {
        for entry in entries.flatten() {
            let p = entry.path();
            if p.is_dir() {
                total += compute_media_bytes(&p);
            } else if let Ok(meta) = p.metadata() {
                total += meta.len();
            }
        }
    }
    total
}

async fn handle_metrics(State(ctx): State<Arc<PipelineContext>>) -> impl IntoResponse {
    let queue = ctx.db.queue_counts().await.unwrap_or_else(|_| serde_json::json!({}));
    let breakers = ctx.guard.breaker_statuses().await;
    let open_breakers = breakers.iter().filter(|b| b.is_open).count();

    let queued = queue.get("queued").and_then(|v| v.as_i64()).unwrap_or(0);
    let running = queue.get("running").and_then(|v| v.as_i64()).unwrap_or(0);
    let succeeded = queue.get("succeeded").and_then(|v| v.as_i64()).unwrap_or(0);
    let failed = queue.get("failed").and_then(|v| v.as_i64()).unwrap_or(0);

    let aimd = ctx.concurrency.snapshot().await;

    let mut sys = sysinfo::System::new();
    let pid = sysinfo::get_current_pid().ok();
    let rss = if let Some(p) = pid {
        sys.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[p]), true);
        sys.process(p).map(|proc| proc.memory()).unwrap_or(0)
    } else {
        0
    };

    let disks = sysinfo::Disks::new_with_refreshed_list();
    let free_disk = disks.iter().map(|d| d.available_space()).max().unwrap_or(0);
    let media_bytes = compute_media_bytes(&ctx.media_root());

    let mut text = format!(
        "leadfinder_jobs_queued {queued}\nleadfinder_jobs_running {running}\nleadfinder_jobs_succeeded {succeeded}\nleadfinder_jobs_failed {failed}\nleadfinder_circuit_breakers_open {open_breakers}\nleadfinder_process_memory_rss_bytes {rss}\nleadfinder_disk_free_bytes {free_disk}\nleadfinder_media_bytes {media_bytes}\nleadfinder_backpressure_active {}\n",
        if aimd.backpressure_active { 1 } else { 0 }
    );

    for (st, snap) in &aimd.stages {
        text.push_str(&format!(
            "leadfinder_aimd_concurrency{{stage=\"{}\"}} {}\n",
            st, snap.current_concurrency
        ));
        text.push_str(&format!(
            "leadfinder_aimd_active_tasks{{stage=\"{}\"}} {}\n",
            st, snap.active_tasks
        ));
        text.push_str(&format!(
            "leadfinder_aimd_latency_ms{{stage=\"{}\"}} {}\n",
            st, snap.last_latency_ms
        ));
    }

    if let Ok(usages) = ctx.db.get_today_provider_usage().await {
        for u in usages {
            text.push_str(&format!(
                "leadfinder_provider_requests_today{{provider=\"{}\",model=\"{}\"}} {}\n",
                u.provider, u.model, u.requests
            ));
            text.push_str(&format!(
                "leadfinder_provider_tokens_in_today{{provider=\"{}\",model=\"{}\"}} {}\n",
                u.provider, u.model, u.tokens_in
            ));
            text.push_str(&format!(
                "leadfinder_provider_tokens_out_today{{provider=\"{}\",model=\"{}\"}} {}\n",
                u.provider, u.model, u.tokens_out
            ));
            text.push_str(&format!(
                "leadfinder_provider_errors_429_today{{provider=\"{}\",model=\"{}\"}} {}\n",
                u.provider, u.model, u.errors_429
            ));
        }
    }

    ([(header::CONTENT_TYPE, "text/plain; version=0.0.4")], text)
}

async fn handle_export(
    State(ctx): State<Arc<PipelineContext>>,
    Query(q): Query<ExportQuery>,
) -> Response {
    let fmt_str = q.format.as_deref().unwrap_or("markdown");
    let fmt = match crate::export::ExportFormat::parse_str(fmt_str) {
        Ok(f) => f,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response()
        }
    };
    match ctx.export_leads(q.lane.as_deref(), fmt).await {
        Ok(out) => match fmt {
            crate::export::ExportFormat::Markdown => (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
                out,
            )
                .into_response(),
            crate::export::ExportFormat::Csv => (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
                out,
            )
                .into_response(),
            crate::export::ExportFormat::Json => (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json")],
                out,
            )
                .into_response(),
        },
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}


#[derive(Debug, Deserialize)]
pub struct DownloadModelRequest {
    pub model_id: String,
    pub url: String,
    pub filename: String,
    pub kind: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct ScanExternalAiServersRequest {
    pub custom_url: Option<String>,
}

fn format_file_size(bytes: u64) -> String {
    if bytes >= 1_073_741_824 {
        format!("{:.2} GB", bytes as f64 / 1_073_741_824.0)
    } else if bytes >= 1_048_576 {
        format!("{:.1} MB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

async fn handle_list_models(State(ctx): State<Arc<PipelineContext>>) -> Response {
    let mut models: Vec<leadfinder_core::InstalledModelInfo> = Vec::new();

    let llm_dirs = [
        ctx.paths.models_dir.join("llm"),
        ctx.paths.static_root.join("models").join("llm"),
    ];
    for dir in &llm_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("gguf") {
                    let fname = p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
                    if !models.iter().any(|m| m.filename == fname) {
                        let meta = entry.metadata().ok();
                        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                        let modified = meta
                            .and_then(|m| m.modified().ok())
                            .and_then(|t| {
                                chrono::DateTime::<chrono::Utc>::from(t)
                                    .to_rfc3339()
                                    .into()
                            })
                            .unwrap_or_default();
                        models.push(leadfinder_core::InstalledModelInfo {
                            id: format!("local_llm_{fname}"),
                            name: fname.clone(),
                            kind: "llm_gguf".to_string(),
                            filename: fname,
                            file_path: p.to_string_lossy().to_string(),
                            size_bytes: size,
                            size_display: format_file_size(size),
                            modified_at: modified,
                            recommended_roles: vec!["claim_extraction".to_string(), "fact_verification".to_string()],
                        });
                    }
                }
            }
        }
    }

    let whisper_dirs = [
        ctx.paths.models_dir.join("whisper"),
        ctx.paths.static_root.join("models").join("whisper"),
    ];
    for dir in &whisper_dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let p = entry.path();
                if p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("bin") {
                    let fname = p.file_name().and_then(|s| s.to_str()).unwrap_or("").to_string();
                    if !models.iter().any(|m| m.filename == fname) {
                        let meta = entry.metadata().ok();
                        let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                        let modified = meta
                            .and_then(|m| m.modified().ok())
                            .and_then(|t| {
                                chrono::DateTime::<chrono::Utc>::from(t)
                                    .to_rfc3339()
                                    .into()
                            })
                            .unwrap_or_default();
                        models.push(leadfinder_core::InstalledModelInfo {
                            id: format!("whisper_{fname}"),
                            name: fname.clone(),
                            kind: "whisper_bin".to_string(),
                            filename: fname,
                            file_path: p.to_string_lossy().to_string(),
                            size_bytes: size,
                            size_display: format_file_size(size),
                            modified_at: modified,
                            recommended_roles: vec!["asr".to_string()],
                        });
                    }
                }
            }
        }
    }

    models.push(leadfinder_core::InstalledModelInfo {
        id: "cloud_gemini_flash".to_string(),
        name: "gemini-2.5-flash".to_string(),
        kind: "cloud_gemini".to_string(),
        filename: "gemini-2.5-flash".to_string(),
        file_path: "Google AI Cloud".to_string(),
        size_bytes: 0,
        size_display: "Cloud API".to_string(),
        modified_at: "Online".to_string(),
        recommended_roles: vec!["claim_extraction".to_string(), "fact_verification".to_string(), "osint_dossier".to_string()],
    });

    models.push(leadfinder_core::InstalledModelInfo {
        id: "cloud_gemini_pro".to_string(),
        name: "gemini-1.5-pro".to_string(),
        kind: "cloud_gemini".to_string(),
        filename: "gemini-1.5-pro".to_string(),
        file_path: "Google AI Cloud".to_string(),
        size_bytes: 0,
        size_display: "Cloud API".to_string(),
        modified_at: "Online".to_string(),
        recommended_roles: vec!["fact_verification".to_string(), "osint_dossier".to_string()],
    });

    models.push(leadfinder_core::InstalledModelInfo {
        id: "offline_heuristics".to_string(),
        name: "heuristics".to_string(),
        kind: "offline_engine".to_string(),
        filename: "heuristics".to_string(),
        file_path: "Embedded Rust Rules".to_string(),
        size_bytes: 0,
        size_display: "0 MB (Built-in)".to_string(),
        modified_at: "Ready".to_string(),
        recommended_roles: vec!["claim_extraction".to_string()],
    });

    Json(serde_json::json!({
        "models": models,
        "models_dir": ctx.paths.models_dir.to_string_lossy(),
    }))
    .into_response()
}

async fn handle_get_model_catalog(State(ctx): State<Arc<PipelineContext>>) -> Response {
    let mut catalog = vec![
        leadfinder_core::CuratedModelCatalogItem {
            id: "bggpt-gemma-3-4b-it".to_string(),
            name: "BgGPT-Gemma-3-4B-IT-Q4_K_M".to_string(),
            author: "INSAIT Institute".to_string(),
            kind: "llm_gguf".to_string(),
            download_url: "https://huggingface.co/INSAIT-Institute/BgGPT-Gemma-3-4B-IT-GGUF/resolve/main/BgGPT-Gemma-3-4B-IT-Q4_K_M.gguf".to_string(),
            filename: "BgGPT-Gemma-3-4B-IT-Q4_K_M.gguf".to_string(),
            size_bytes: 2_489_893_728,
            size_display: "2.49 GB".to_string(),
            vram_estimate: "~3.5 GB VRAM".to_string(),
            description_bg: "Най-добрият специализиран български AI езиков модел. Перфектен за екстракция на твърдения и сложни формулировки.".to_string(),
            description_en: "State-of-the-art Bulgarian AI language model. Ideal for claim extraction and nuanced Bulgarian syntax.".to_string(),
            recommended_for: vec!["claim_extraction".to_string(), "fact_verification".to_string()],
            is_installed: false,
        },
        leadfinder_core::CuratedModelCatalogItem {
            id: "bggpt-gemma-2-2b-it".to_string(),
            name: "BgGPT-Gemma-2-2B-IT-v1.0.Q4_K_M".to_string(),
            author: "INSAIT Institute".to_string(),
            kind: "llm_gguf".to_string(),
            download_url: "https://huggingface.co/INSAIT-Institute/BgGPT-Gemma-2-2.6B-IT-v1.0-GGUF/resolve/main/BgGPT-Gemma-2-2B-IT-v1.0.Q4_K_M.gguf".to_string(),
            filename: "BgGPT-Gemma-2-2B-IT-v1.0.Q4_K_M.gguf".to_string(),
            size_bytes: 1_708_582_400,
            size_display: "1.71 GB".to_string(),
            vram_estimate: "~2.2 GB VRAM".to_string(),
            description_bg: "Ултра-лек и бърз български модел. Отличен за слаби лаптопи и режим на батерия.".to_string(),
            description_en: "Ultra-compact and fast Bulgarian model. Optimal for low-end laptops and battery mode.".to_string(),
            recommended_for: vec!["claim_extraction".to_string()],
            is_installed: false,
        },
        leadfinder_core::CuratedModelCatalogItem {
            id: "qwen-2.5-3b-instruct".to_string(),
            name: "Qwen2.5-3B-Instruct-Q4_K_M".to_string(),
            author: "Qwen Team (Alibaba)".to_string(),
            kind: "llm_gguf".to_string(),
            download_url: "https://huggingface.co/Qwen/Qwen2.5-3B-Instruct-GGUF/resolve/main/qwen2.5-3b-instruct-q4_k_m.gguf".to_string(),
            filename: "qwen2.5-3b-instruct-q4_k_m.gguf".to_string(),
            size_bytes: 2_050_000_000,
            size_display: "2.05 GB".to_string(),
            vram_estimate: "~2.8 GB VRAM".to_string(),
            description_bg: "Глобален шампион по скорост и точност в 3B класа. Превъзходно структуриране в JSON.".to_string(),
            description_en: "Top-tier 3B model for speed and accuracy. Excellent JSON adherence.".to_string(),
            recommended_for: vec!["claim_extraction".to_string()],
            is_installed: false,
        },
        leadfinder_core::CuratedModelCatalogItem {
            id: "qwen-2.5-7b-instruct".to_string(),
            name: "Qwen2.5-7B-Instruct-Q4_K_M".to_string(),
            author: "Qwen Team (Alibaba)".to_string(),
            kind: "llm_gguf".to_string(),
            download_url: "https://huggingface.co/Qwen/Qwen2.5-7B-Instruct-GGUF/resolve/main/qwen2.5-7b-instruct-q4_k_m.gguf".to_string(),
            filename: "qwen2.5-7b-instruct-q4_k_m.gguf".to_string(),
            size_bytes: 4_480_000_000,
            size_display: "4.48 GB".to_string(),
            vram_estimate: "~5.8 GB VRAM".to_string(),
            description_bg: "Мощен аналитичен модел за дълбоко логическо разсъждение и проверка на сложни твърдения.".to_string(),
            description_en: "Powerful analytical model for deep logical reasoning and fact verification.".to_string(),
            recommended_for: vec!["fact_verification".to_string(), "osint_dossier".to_string()],
            is_installed: false,
        },
        leadfinder_core::CuratedModelCatalogItem {
            id: "llama-3.2-3b-instruct".to_string(),
            name: "Llama-3.2-3B-Instruct-Q4_K_M".to_string(),
            author: "Meta AI".to_string(),
            kind: "llm_gguf".to_string(),
            download_url: "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/resolve/main/Llama-3.2-3B-Instruct-Q4_K_M.gguf".to_string(),
            filename: "Llama-3.2-3B-Instruct-Q4_K_M.gguf".to_string(),
            size_bytes: 2_020_000_000,
            size_display: "2.02 GB".to_string(),
            vram_estimate: "~2.7 GB VRAM".to_string(),
            description_bg: "Бърз и надежден многоезичен модел от Meta за филтриране на факти.".to_string(),
            description_en: "Fast and reliable multilingual model from Meta for fact screening.".to_string(),
            recommended_for: vec!["claim_extraction".to_string()],
            is_installed: false,
        },
        leadfinder_core::CuratedModelCatalogItem {
            id: "whisper-large-v3-turbo".to_string(),
            name: "Whisper Large-v3-Turbo".to_string(),
            author: "OpenAI".to_string(),
            kind: "whisper_bin".to_string(),
            download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin".to_string(),
            filename: "ggml-large-v3-turbo-q5_0.bin".to_string(),
            size_bytes: 574_041_195,
            size_display: "574 MB".to_string(),
            vram_estimate: "~1.2 GB VRAM".to_string(),
            description_bg: "Препоръчителният флагман за разпознаване на българска реч с под 5% грешка (WER).".to_string(),
            description_en: "Recommended flagship Bulgarian speech recognition with <5% WER.".to_string(),
            recommended_for: vec!["asr".to_string()],
            is_installed: false,
        },
        leadfinder_core::CuratedModelCatalogItem {
            id: "whisper-small".to_string(),
            name: "Whisper Small".to_string(),
            author: "OpenAI".to_string(),
            kind: "whisper_bin".to_string(),
            download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin".to_string(),
            filename: "ggml-small.bin".to_string(),
            size_bytes: 487_601_967,
            size_display: "488 MB".to_string(),
            vram_estimate: "~1.0 GB VRAM".to_string(),
            description_bg: "Балансиран модел за по-слаби компютри и лаптопи на батерия.".to_string(),
            description_en: "Balanced ASR model for lower-spec machines and battery power.".to_string(),
            recommended_for: vec!["asr".to_string()],
            is_installed: false,
        },
        leadfinder_core::CuratedModelCatalogItem {
            id: "whisper-base".to_string(),
            name: "Whisper Base".to_string(),
            author: "OpenAI".to_string(),
            kind: "whisper_bin".to_string(),
            download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin".to_string(),
            filename: "ggml-base.bin".to_string(),
            size_bytes: 147_951_465,
            size_display: "148 MB".to_string(),
            vram_estimate: "~500 MB VRAM".to_string(),
            description_bg: "Ултра-лек ASR модел за максимална скорост на стари машини.".to_string(),
            description_en: "Ultra-lightweight ASR model for maximum speed on legacy systems.".to_string(),
            recommended_for: vec!["asr".to_string()],
            is_installed: false,
        },
    ];

    for item in &mut catalog {
        let sub = if item.kind == "whisper_bin" { "whisper" } else { "llm" };
        let check_paths = [
            ctx.paths.models_dir.join(sub).join(&item.filename),
            ctx.paths.static_root.join("models").join(sub).join(&item.filename),
        ];
        item.is_installed = check_paths.iter().any(|p| p.is_file());
    }

    Json(serde_json::json!({ "catalog": catalog })).into_response()
}

async fn handle_download_model(
    State(ctx): State<Arc<PipelineContext>>,
    Json(req): Json<DownloadModelRequest>,
) -> Response {
    if ctx.model_download_state.read().await.active {
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({ "error": "Another model download is already running." })),
        )
            .into_response();
    }

    let sub = if req.kind == "whisper" || req.filename.ends_with(".bin") { "whisper" } else { "llm" };
    let dest_dir = ctx.paths.models_dir.join(sub);
    let _ = std::fs::create_dir_all(&dest_dir);
    let final_dest = dest_dir.join(&req.filename);
    let part_dest = dest_dir.join(format!("{}.part", req.filename));

    ctx.model_download_cancel.store(false, Ordering::SeqCst);
    {
        let mut st = ctx.model_download_state.write().await;
        st.active = true;
        st.model_id = req.model_id.clone();
        st.filename = req.filename.clone();
        st.bytes_downloaded = 0;
        st.total_bytes = 0;
        st.progress_pct = 0;
        st.speed_bps = 0;
        st.status = "downloading".to_string();
        st.error_message = None;
    }

    let state_arc = ctx.model_download_state.clone();
    let cancel_arc = ctx.model_download_cancel.clone();
    let events_tx = ctx.events_tx.clone();

    tokio::spawn(async move {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(3600))
            .build()
            .unwrap_or_default();

        let resp_res = client.get(&req.url).send().await;
        let resp = match resp_res {
            Ok(r) => match r.error_for_status() {
                Ok(ok_r) => ok_r,
                Err(e) => {
                    let mut st = state_arc.write().await;
                    st.active = false;
                    st.status = "error".to_string();
                    st.error_message = Some(e.to_string());
                    return;
                }
            },
            Err(e) => {
                let mut st = state_arc.write().await;
                st.active = false;
                st.status = "error".to_string();
                st.error_message = Some(e.to_string());
                return;
            }
        };

        let total_size = resp.content_length().unwrap_or(0);
        {
            let mut st = state_arc.write().await;
            st.total_bytes = total_size;
        }

        let mut file = match tokio::fs::File::create(&part_dest).await {
            Ok(f) => f,
            Err(e) => {
                let mut st = state_arc.write().await;
                st.active = false;
                st.status = "error".to_string();
                st.error_message = Some(e.to_string());
                return;
            }
        };

        let mut stream = resp.bytes_stream();
        let mut downloaded: u64 = 0;
        let mut last_instant = std::time::Instant::now();
        let mut last_bytes: u64 = 0;

        while let Some(chunk_res) = stream.next().await {
            if cancel_arc.load(Ordering::SeqCst) {
                let _ = tokio::fs::remove_file(&part_dest).await;
                let mut st = state_arc.write().await;
                st.active = false;
                st.status = "cancelled".to_string();
                return;
            }

            match chunk_res {
                Ok(chunk) => {
                    if let Err(e) = file.write_all(&chunk).await {
                        let mut st = state_arc.write().await;
                        st.active = false;
                        st.status = "error".to_string();
                        st.error_message = Some(e.to_string());
                        return;
                    }
                    downloaded += chunk.len() as u64;

                    if last_instant.elapsed() >= Duration::from_millis(500) {
                        let elapsed_s = last_instant.elapsed().as_secs_f64().max(0.001);
                        let bps = ((downloaded - last_bytes) as f64 / elapsed_s) as u64;
                        last_bytes = downloaded;
                        last_instant = std::time::Instant::now();

                        let pct = if total_size > 0 {
                            ((downloaded as f64 / total_size as f64) * 100.0).min(100.0) as u8
                        } else {
                            0
                        };

                        let mut st = state_arc.write().await;
                        st.bytes_downloaded = downloaded;
                        st.progress_pct = pct;
                        st.speed_bps = bps;
                    }
                }
                Err(e) => {
                    let mut st = state_arc.write().await;
                    st.active = false;
                    st.status = "error".to_string();
                    st.error_message = Some(e.to_string());
                    return;
                }
            }
        }

        let _ = file.flush().await;
        drop(file);

        if cancel_arc.load(Ordering::SeqCst) {
            let _ = tokio::fs::remove_file(&part_dest).await;
            let mut st = state_arc.write().await;
            st.active = false;
            st.status = "cancelled".to_string();
            return;
        }

        if let Err(e) = std::fs::rename(&part_dest, &final_dest) {
            let mut st = state_arc.write().await;
            st.active = false;
            st.status = "error".to_string();
            st.error_message = Some(e.to_string());
            return;
        }

        {
            let mut st = state_arc.write().await;
            st.active = false;
            st.bytes_downloaded = total_size;
            st.progress_pct = 100;
            st.status = "completed".to_string();
        }

        let _ = events_tx.send(leadfinder_core::CoreEvent::CalibrationUpdated { capacity_per_day: None });
    });

    Json(serde_json::json!({ "status": "started", "filename": req.filename })).into_response()
}

async fn handle_download_model_status(State(ctx): State<Arc<PipelineContext>>) -> Response {
    let st = ctx.model_download_state.read().await.clone();
    Json(st).into_response()
}

async fn handle_download_model_cancel(State(ctx): State<Arc<PipelineContext>>) -> Response {
    ctx.model_download_cancel.store(true, Ordering::SeqCst);
    Json(serde_json::json!({ "status": "cancelling" })).into_response()
}

async fn handle_delete_model(
    State(ctx): State<Arc<PipelineContext>>,
    Path((kind, filename)): Path<(String, String)>,
) -> Response {
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": "Invalid filename" }))).into_response();
    }
    let sub = if kind == "whisper" { "whisper" } else { "llm" };
    let path = ctx.paths.models_dir.join(sub).join(&filename);
    if path.is_file() {
        if let Err(e) = std::fs::remove_file(&path) {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": e.to_string() })),
            )
                .into_response();
        }
    }
    Json(serde_json::json!({ "deleted": true, "filename": filename })).into_response()
}

async fn handle_scan_models_folder(State(ctx): State<Arc<PipelineContext>>) -> Response {
    let _ = std::fs::create_dir_all(ctx.paths.models_dir.join("llm"));
    let _ = std::fs::create_dir_all(ctx.paths.models_dir.join("whisper"));
    handle_list_models(State(ctx)).await
}

async fn handle_scan_external_ai_servers(
    State(_ctx): State<Arc<PipelineContext>>,
    Json(req): Json<ScanExternalAiServersRequest>,
) -> Response {
    let mut servers: Vec<leadfinder_core::ExternalAiServerStatus> = Vec::new();
    let client = reqwest::Client::builder().timeout(Duration::from_millis(2000)).build().unwrap_or_default();

    let t0 = std::time::Instant::now();
    let ollama_res = client.get("http://127.0.0.1:11434/api/tags").send().await;
    let mut ollama_status = leadfinder_core::ExternalAiServerStatus {
        server_type: "ollama".to_string(),
        endpoint_url: "http://localhost:11434".to_string(),
        is_online: false,
        latency_ms: None,
        models: Vec::new(),
    };
    if let Ok(resp) = ollama_res {
        if resp.status().is_success() {
            ollama_status.is_online = true;
            ollama_status.latency_ms = Some(t0.elapsed().as_millis() as u64);
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                if let Some(arr) = body.get("models").and_then(|v| v.as_array()) {
                    for m in arr {
                        let name = m.get("name").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        let size = m.get("size").and_then(|v| v.as_u64());
                        let param = m.get("details").and_then(|d| d.get("parameter_size")).and_then(|v| v.as_str()).map(|s| s.to_string());
                        let quant = m.get("details").and_then(|d| d.get("quantization_level")).and_then(|v| v.as_str()).map(|s| s.to_string());
                        if !name.is_empty() {
                            ollama_status.models.push(leadfinder_core::ExternalAiServerModel {
                                id: format!("ollama_{name}"),
                                name,
                                parameter_size: param,
                                quantization: quant,
                                size_bytes: size,
                            });
                        }
                    }
                }
            }
        }
    }
    servers.push(ollama_status);

    let t1 = std::time::Instant::now();
    let lm_res = client.get("http://127.0.0.1:1234/v1/models").send().await;
    let mut lm_status = leadfinder_core::ExternalAiServerStatus {
        server_type: "lmstudio".to_string(),
        endpoint_url: "http://localhost:1234".to_string(),
        is_online: false,
        latency_ms: None,
        models: Vec::new(),
    };
    if let Ok(resp) = lm_res {
        if resp.status().is_success() {
            lm_status.is_online = true;
            lm_status.latency_ms = Some(t1.elapsed().as_millis() as u64);
            if let Ok(body) = resp.json::<serde_json::Value>().await {
                if let Some(arr) = body.get("data").and_then(|v| v.as_array()) {
                    for m in arr {
                        let id = m.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                        if !id.is_empty() {
                            lm_status.models.push(leadfinder_core::ExternalAiServerModel {
                                id: format!("lmstudio_{id}"),
                                name: id,
                                parameter_size: None,
                                quantization: None,
                                size_bytes: None,
                            });
                        }
                    }
                }
            }
        }
    }
    servers.push(lm_status);

    if let Some(custom_url) = req.custom_url.filter(|s| !s.is_empty()) {
        let t2 = std::time::Instant::now();
        let target_url = if custom_url.ends_with("/models") {
            custom_url.clone()
        } else if custom_url.ends_with("/v1") {
            format!("{custom_url}/models")
        } else {
            format!("{}/v1/models", custom_url.trim_end_matches('/'))
        };

        let custom_res = client.get(&target_url).send().await;
        let mut custom_status = leadfinder_core::ExternalAiServerStatus {
            server_type: "custom".to_string(),
            endpoint_url: custom_url,
            is_online: false,
            latency_ms: None,
            models: Vec::new(),
        };
        if let Ok(resp) = custom_res {
            if resp.status().is_success() {
                custom_status.is_online = true;
                custom_status.latency_ms = Some(t2.elapsed().as_millis() as u64);
                if let Ok(body) = resp.json::<serde_json::Value>().await {
                    if let Some(arr) = body.get("data").and_then(|v| v.as_array()) {
                        for m in arr {
                            let id = m.get("id").and_then(|v| v.as_str()).unwrap_or("").to_string();
                            if !id.is_empty() {
                                custom_status.models.push(leadfinder_core::ExternalAiServerModel {
                                    id: format!("custom_{id}"),
                                    name: id,
                                    parameter_size: None,
                                    quantization: None,
                                    size_bytes: None,
                                });
                            }
                        }
                    }
                }
            }
        }
        servers.push(custom_status);
    }

    Json(serde_json::json!({ "servers": servers })).into_response()
}

async fn handle_get_model_roles(State(ctx): State<Arc<PipelineContext>>) -> Response {
    if let Ok(Some(roles_val)) = ctx.db.get_setting("model_roles").await {
        if let Ok(cfg) = serde_json::from_value::<leadfinder_core::ModelHubRolesConfig>(roles_val) {
            return Json(cfg).into_response();
        }
    }

    let prof = ctx.db.get_profile().await.ok().flatten();
    let eff_asr = prof.as_ref().and_then(|p| p.chosen_json.get("effective")).and_then(|e| e.get("asr_model")).and_then(|v| v.as_str()).unwrap_or("large-v3-turbo");
    let eff_llm_prov = prof.as_ref().and_then(|p| p.chosen_json.get("effective")).and_then(|e| e.get("llm_provider")).and_then(|v| v.as_str()).unwrap_or("local");
    let eff_llm_model = prof.as_ref().and_then(|p| p.chosen_json.get("effective")).and_then(|e| e.get("llm_model")).and_then(|v| v.as_str()).unwrap_or("BgGPT-Gemma-3-4B-IT-Q4_K_M.gguf");

    let mut roles = std::collections::HashMap::new();
    roles.insert("asr".to_string(), leadfinder_core::ModelRoleConfig {
        role: "asr".to_string(),
        provider: "whisper".to_string(),
        model_name: eff_asr.to_string(),
        custom_url: None,
        temperature: None,
    });
    roles.insert("claim_extraction".to_string(), leadfinder_core::ModelRoleConfig {
        role: "claim_extraction".to_string(),
        provider: eff_llm_prov.to_string(),
        model_name: eff_llm_model.to_string(),
        custom_url: None,
        temperature: Some(0.0),
    });
    roles.insert("fact_verification".to_string(), leadfinder_core::ModelRoleConfig {
        role: "fact_verification".to_string(),
        provider: eff_llm_prov.to_string(),
        model_name: eff_llm_model.to_string(),
        custom_url: None,
        temperature: Some(0.0),
    });
    roles.insert("osint_dossier".to_string(), leadfinder_core::ModelRoleConfig {
        role: "osint_dossier".to_string(),
        provider: "gemini".to_string(),
        model_name: "gemini-2.5-flash".to_string(),
        custom_url: None,
        temperature: Some(0.2),
    });
    roles.insert("ocr".to_string(), leadfinder_core::ModelRoleConfig {
        role: "ocr".to_string(),
        provider: "tesseract".to_string(),
        model_name: "bul+eng".to_string(),
        custom_url: None,
        temperature: None,
    });

    let defaults = leadfinder_core::ModelHubRolesConfig {
        roles,
        consensus: leadfinder_core::ConsensusSettings::default(),
    };

    Json(defaults).into_response()
}

async fn handle_update_model_roles(
    State(ctx): State<Arc<PipelineContext>>,
    Json(cfg): Json<leadfinder_core::ModelHubRolesConfig>,
) -> Response {
    let val = serde_json::to_value(&cfg).unwrap_or(serde_json::Value::Null);
    if let Err(e) = ctx.db.set_setting("model_roles", &val).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response();
    }
    Json(serde_json::json!({ "saved": true })).into_response()
}

async fn handle_test_model(
    State(ctx): State<Arc<PipelineContext>>,
    Json(req): Json<leadfinder_core::ModelTestRequest>,
) -> Response {
    let start = std::time::Instant::now();
    let app_cfg = ctx.config.read().await.clone();

    let llm_provider: Arc<dyn leadfinder_core::Llm> = match req.provider.as_str() {
        "gemini" => {
            let key = leadfinder_core::resolve_secret("GEMINI_API_KEY");
            Arc::new(leadfinder_providers::GeminiLlm::new(
                &req.model,
                key,
                true,
                Duration::from_secs(45),
            ))
        }
        "ollama" => {
            let raw_url = req.custom_url.as_deref().unwrap_or("http://localhost:11434");
            let norm_url = if raw_url.ends_with("/v1") {
                raw_url.to_string()
            } else {
                format!("{}/v1", raw_url.trim_end_matches('/'))
            };
            Arc::new(leadfinder_providers::OpenAiCompatLlm::new(
                "ollama",
                norm_url,
                &req.model,
                None,
                Duration::from_secs(45),
            ))
        }
        "lmstudio" => {
            let raw_url = req.custom_url.as_deref().unwrap_or("http://localhost:1234");
            let norm_url = if raw_url.ends_with("/v1") {
                raw_url.to_string()
            } else {
                format!("{}/v1", raw_url.trim_end_matches('/'))
            };
            Arc::new(leadfinder_providers::OpenAiCompatLlm::new(
                "lmstudio",
                norm_url,
                &req.model,
                None,
                Duration::from_secs(45),
            ))
        }
        "heuristics" => {
            Arc::new(leadfinder_providers::HeuristicLlm::new())
        }
        _ => {
            let model_rel = format!("llm/{}", req.model);
            let resolved = ctx
                .paths
                .resolve_model_path(&model_rel)
                .or_else(|| ctx.paths.resolve_model_path(&req.model))
                .unwrap_or_else(|| ctx.paths.models_dir.join("llm").join(&req.model));
            let llama_bin = ctx
                .paths
                .resolve_binary("llama-server")
                .unwrap_or_else(|| ctx.paths.tools_dir.join("llama-server.exe"));
            Arc::new(
                leadfinder_providers::OpenAiCompatLlm::new(
                    "local",
                    &app_cfg.providers.local_llm_url,
                    &req.model,
                    None,
                    Duration::from_secs(60),
                )
                .with_local_autolaunch(leadfinder_providers::LocalLlamaLaunchSpec {
                    llama_bin,
                    model_path: resolved,
                    use_gpu: true,
                    context_size: 4096,
                }),
            )
        }
    };

    let llm_req = leadfinder_core::LlmJsonRequest {
        system_prompt: req.system_prompt.unwrap_or_else(|| "You are an AI assistant. Return strictly valid JSON with an answer field.".to_string()),
        user_prompt: req.prompt,
        json_schema: None,
        temperature: 0.1,
        max_tokens: 1024,
    };

    match llm_provider.complete_json(&llm_req).await {
        Ok(resp) => {
            let elapsed_ms = start.elapsed().as_millis() as u64;
            let elapsed_s = (elapsed_ms as f64 / 1000.0).max(0.001);
            let tokens_per_second = (resp.tokens_out as f64 / elapsed_s * 10.0).round() / 10.0;
            Json(leadfinder_core::ModelTestResponse {
                success: true,
                elapsed_ms,
                tokens_in: resp.tokens_in,
                tokens_out: resp.tokens_out,
                tokens_per_second,
                raw_text: resp.raw_text,
                parsed_json: Some(resp.parsed_json),
                error: None,
            })
            .into_response()
        }
        Err(e) => {
            let elapsed_ms = start.elapsed().as_millis() as u64;
            Json(leadfinder_core::ModelTestResponse {
                success: false,
                elapsed_ms,
                tokens_in: 0,
                tokens_out: 0,
                tokens_per_second: 0.0,
                raw_text: String::new(),
                parsed_json: None,
                error: Some(e.to_string()),
            })
            .into_response()
        }
    }
}

