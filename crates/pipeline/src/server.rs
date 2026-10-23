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
use crate::engine::PipelineContext;

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
        .route("/api/rank/run", post(handle_rank_run))
        .route("/api/rank/pull-more", post(handle_pull_more))
        .route("/api/rank/tune", post(handle_tune))
        .route("/api/videos", get(handle_list_videos))
        .route("/api/videos/:id", get(handle_video_detail).delete(handle_delete_video))
        .route("/api/videos/:id/decision", post(handle_decision))
        .route("/api/videos/:id/transcript", patch(handle_edit_transcript))
        .route("/api/videos/:id/retranscribe", post(handle_retranscribe))
        .route("/api/videos/:id/ai-check", post(handle_ai_check_transcript))
        .route("/api/evidence/preview", get(handle_evidence_preview))
        .route("/api/people/:id/research", post(handle_research_person))
        .route("/api/seeds", get(handle_list_seeds).post(handle_upsert_seed))
        .route("/api/seeds/:id", delete(handle_delete_seed))
        .route("/api/creators", get(handle_list_creators))
        .route("/api/creators/watchlist", post(handle_set_watchlist))
        .route("/api/profile", get(handle_get_profile))
        .route("/api/calibrate", post(handle_calibrate))
        .route("/api/calibrate/overrides", post(handle_calibrate_overrides))
        .route("/api/settings", get(handle_list_settings).post(handle_update_settings))
        .route("/api/export", get(handle_export))
        .route("/api/scale/status", get(handle_scale_status))
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
        Ok(promoted) => Json(serde_json::json!({ "promoted": promoted })).into_response(),
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
        let user_prompt = format!("what would the fixed text look like?\n{stage1}");

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
    let user_prompt = format!("what would the fixed text look like?\n{stage1}");

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

async fn handle_research_person(
    State(ctx): State<Arc<PipelineContext>>,
    Path(id): Path<i64>,
    Json(body): Json<ResearchPersonRequest>,
) -> Response {
    let name = body.name.unwrap_or_default();
    let cfg = ctx.config.read().await.clone();
    let lex = leadfinder_providers::global_lexicon(&ctx.paths, &cfg.normalisation.lexicon_config_file);
    let query_str = format!("{} новини биография длъжност фактчекинг", name);
    let search_q = leadfinder_core::SearchQuery {
        query: query_str,
        restrict_domains: Vec::new(),
        date_from: None,
        date_to: None,
        max_results: 4,
    };
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
    if let Ok(hits) = search_engine.search(&search_q).await {
        for hit in hits.into_iter().take(3) {
            let html = ranker.fetch_html_cached(&hit.url).await.unwrap_or_default();
            let txt = ranker.extract_main_text(&html);
            if !txt.is_empty() {
                hits_text.push(format!("Източник {}: {}\n{}", hit.title, hit.url, txt.chars().take(800).collect::<String>()));
            }
        }
    }

    let summary_context = hits_text.join("\n\n");
    let runner = leadfinder_providers::LlmChainRunner::from_runtime(&ctx.paths, &cfg, &ctx.db).await;
    let schema_val = serde_json::json!({
        "type": "object",
        "properties": {
            "role": { "type": "string" },
            "summary": { "type": "string" },
            "public_figure": { "type": "boolean" }
        },
        "required": ["role", "summary", "public_figure"]
    });
    let llm_req = leadfinder_core::LlmJsonRequest {
        system_prompt: "You are a Bulgarian investigative journalist. Given the scraped news data about the named person, summarize in Bulgarian in 2 concise sentences: their public position or profession, and any notable public actions or fact-checks. Output valid JSON.".to_string(),
        user_prompt: format!("Лице: {}\nКонтекст от мрежата:\n{}", name, summary_context),
        json_schema: Some(schema_val),
        max_tokens: 512,
        temperature: 0.1,
    };

    let mut res_role = None;
    let mut res_summary = None;
    let mut res_pub = false;
    if let Ok(val) = runner.complete_validated(&llm_req, &ctx.db).await {
        res_role = val.response.parsed_json.get("role").and_then(|v| v.as_str()).map(|s| s.to_string());
        res_summary = val.response.parsed_json.get("summary").and_then(|v| v.as_str()).map(|s| s.to_string());
        res_pub = val.response.parsed_json.get("public_figure").and_then(|v| v.as_bool()).unwrap_or(false);
    }
    if res_summary.is_none() && !summary_context.is_empty() {
        res_summary = Some(summary_context.chars().take(250).collect());
    }

    let _ = ctx.db.update_person_profile(id, res_role.as_deref(), res_summary.as_deref(), res_pub).await;
    let (total_claims, contradicted, supported) = ctx.db.get_person_claims_stats(id).await.unwrap_or((0, 0, 0));

    Json(serde_json::json!({
        "id": id,
        "canonical_name": name,
        "role": res_role,
        "summary": res_summary,
        "public_figure": res_pub,
        "total_claims": total_claims,
        "contradicted_count": contradicted,
        "supported_count": supported
    })).into_response()
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
