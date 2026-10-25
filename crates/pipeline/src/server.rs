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
use leadfinder_core::{CoreEvent, Search, Stage, VideoFilter};
use leadfinder_probe::{apply_calibration_overrides, run_calibration, CalibrationOverrides};
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
        .route("/api/profile", get(handle_get_profile))
        .route("/api/calibrate", post(handle_calibrate))
        .route("/api/calibrate/overrides", post(handle_calibrate_overrides))
        .route("/api/settings", get(handle_list_settings).post(handle_update_settings))
        .route("/api/settings/gemini-status", get(handle_gemini_status))
        .route("/api/settings/gemini-key", post(handle_set_gemini_key).delete(handle_delete_gemini_key))
        .route("/api/export", get(handle_export))
        .route("/api/scale/status", get(handle_scale_status))
        .route("/api/system/shutdown", post(handle_system_shutdown))
        .route("/api/system/shutdown-when-idle", post(handle_system_shutdown_when_idle))
        .route("/api/system/exit-status", get(handle_system_exit_status))
        .route_layer(middleware::from_fn_with_state(ctx.clone(), auth_middleware));

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
    Json(body): Json<CalibrateRequest>,
) -> Response {
    let cfg = ctx.config.read().await.clone();
    match run_calibration(&ctx.paths, &cfg, &ctx.db, body.force_cpu).await {
        Ok(prof) => Json(prof).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
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
}

#[derive(Debug, Deserialize)]
pub struct SetGeminiKeyRequest {
    pub key: String,
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
    let configured = resolved_key.is_some();
    let masked_key = resolved_key.as_deref().map(mask_api_key);
    let cloud_enabled = cfg.providers.cloud_enabled;
    let model = cfg.providers.gemini_model.clone();

    Json(GeminiStatusResponse {
        configured,
        masked_key,
        cloud_enabled,
        model,
    })
    .into_response()
}

async fn handle_set_gemini_key(
    State(ctx): State<Arc<PipelineContext>>,
    Json(body): Json<SetGeminiKeyRequest>,
) -> Response {
    let key = body.key.trim().to_string();
    if key.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "API key cannot be empty" })),
        )
            .into_response();
    }

    let test_url = format!("https://generativelanguage.googleapis.com/v1beta/models?key={}", key);
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build();

    let client = match client {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "error": format!("Failed to create HTTP client: {e}") })),
            )
                .into_response();
        }
    };

    match client.get(&test_url).send().await {
        Ok(resp) => {
            if !resp.status().is_success() {
                let status = resp.status();
                let err_body = resp.text().await.unwrap_or_default();
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": format!("Google AI validation failed (status {status}): {err_body}")
                    })),
                )
                    .into_response();
            }
        }
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Connection to Google AI failed: {e}")
                })),
            )
                .into_response();
        }
    }

    std::env::set_var("GEMINI_API_KEY", &key);
    let _ = leadfinder_core::store_secret_in_os_credential_store("GEMINI_API_KEY", &key);

    let _ = ctx.db.set_setting("cloud_enabled", &serde_json::json!(true)).await;
    let _ = ctx.db.set_setting("gemini_configured", &serde_json::json!(true)).await;
    ctx.config.write().await.providers.cloud_enabled = true;

    Json(serde_json::json!({
        "ok": true,
        "masked_key": mask_api_key(&key),
        "cloud_enabled": true,
    }))
    .into_response()
}

async fn handle_delete_gemini_key(
    State(ctx): State<Arc<PipelineContext>>,
) -> Response {
    std::env::remove_var("GEMINI_API_KEY");
    let _ = leadfinder_core::store_secret_in_os_credential_store("GEMINI_API_KEY", "");
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
